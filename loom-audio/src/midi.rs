//! MIDI in and out through the ALSA sequencer (midir) — the port of the
//! browser's Web MIDI features:
//!
//! - OUT: `| midi N` notes and MIDI clock go to a virtual port named "LOOM"
//!   (connect it to a synth or DAW) and to the first hardware output found.
//! - IN: every input port is listened to — keys play the `lead` voice live, and
//!   an external MIDI clock (start/stop/tempo) can drive LOOM.
//!
//! The audio thread only computes WHEN each message is due; a dedicated thread
//! sleeps until then and sends it, so MIDI never blocks the sound.

use crate::seq::MidiEvent;
use midir::os::unix::{VirtualInput, VirtualOutput};
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

/// What arrived from a MIDI keyboard or controller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MidiMessageIn {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8 },
    /// A knob or fader moved: controller number and value 0..127.
    Control { cc: u8, value: u8 },
    /// One clock pulse (24 per quarter note).
    Clock,
    Start,
    Stop,
}

/// Decode one incoming MIDI message.
pub fn decode(bytes: &[u8]) -> Option<MidiMessageIn> {
    let status = *bytes.first()?;
    match status {
        0xF8 => return Some(MidiMessageIn::Clock),
        0xFA | 0xFB => return Some(MidiMessageIn::Start),
        0xFC => return Some(MidiMessageIn::Stop),
        _ => {}
    }
    match (status & 0xF0, bytes.get(1), bytes.get(2)) {
        (0x90, Some(&note), Some(&velocity)) if velocity > 0 => Some(MidiMessageIn::NoteOn { note, velocity }),
        (0x90, Some(&note), Some(_)) | (0x80, Some(&note), _) => Some(MidiMessageIn::NoteOff { note }),
        (0xB0, Some(&cc), Some(&value)) => Some(MidiMessageIn::Control { cc, value }),
        _ => None,
    }
}

/// Estimates the tempo of an incoming MIDI clock, like the browser does
/// (average of the last ~24 pulse intervals, once at least 9 have arrived).
#[derive(Default)]
pub struct ClockFollower { times: Vec<Instant> }

impl ClockFollower {
    pub fn reset(&mut self) { self.times.clear(); }
    /// Record a pulse; returns the tempo when it can be estimated.
    pub fn pulse(&mut self, now: Instant) -> Option<f64> {
        self.times.push(now);
        if self.times.len() > 25 { self.times.remove(0); }
        if self.times.len() < 9 { return None; }
        let span = now.duration_since(self.times[0]).as_secs_f64();
        let per_pulse = span / (self.times.len() - 1) as f64;
        let bpm = 60.0 / (per_pulse * 24.0);
        (30.0..300.0).contains(&bpm).then_some(bpm)
    }
}

/// How long before a message is due the MIDI thread stops sleeping and spins.
const SPIN: Duration = Duration::from_millis(3);

/// Where the audio thread hands timed MIDI events to the MIDI thread.
pub type MidiSender = Sender<(Instant, MidiEvent)>;

/// One timed outgoing message.
struct Timed { at: Instant, bytes: [u8; 3], len: usize }

/// The MIDI connections. Dropping it closes the ports.
pub struct MidiIo {
    tx: Sender<(Instant, MidiEvent)>,
    _inputs: Vec<MidiInputConnection<()>>,
    /// Human-readable summary of what's connected, for the status bar.
    pub description: String,
}

impl MidiIo {
    /// Open the ports. `on_input` is called (on a MIDI thread) for every message received.
    pub fn start(on_input: impl Fn(MidiMessageIn) + Send + Sync + Clone + 'static) -> Result<MidiIo, String> {
        let mut outputs: Vec<MidiOutputConnection> = Vec::new();
        let mut parts: Vec<String> = Vec::new();

        let virt = MidiOutput::new("LOOM").map_err(|e| e.to_string())?;
        if let Ok(conn) = virt.create_virtual("LOOM") {
            outputs.push(conn);
            parts.push("port virtuel « LOOM »".into());
        }
        let hw = MidiOutput::new("LOOM").map_err(|e| e.to_string())?;
        let hw_port = hw.ports().into_iter().find(|p| {
            hw.port_name(p).map(|n| !n.contains("Through") && !n.contains("LOOM")).unwrap_or(false)
        });
        // the port we write to must never be read back: a bus (IAC) would loop our own notes in
        let mut out_name = String::new();
        if let Some(port) = hw_port {
            let name = hw.port_name(&port).unwrap_or_default();
            out_name = name.clone();
            if let Ok(conn) = hw.connect(&port, "LOOM out") {
                outputs.push(conn);
                parts.push(format!("sortie {name}"));
            }
        }

        let mut inputs = Vec::new();
        let probe = MidiInput::new("LOOM").map_err(|e| e.to_string())?;
        for port in probe.ports() {
            let Ok(name) = probe.port_name(&port) else { continue };
            if name.contains("Through") || name.contains("LOOM") || name == out_name { continue; }
            let Ok(mut input) = MidiInput::new("LOOM") else { continue };
            input.ignore(midir::Ignore::Sysex | midir::Ignore::ActiveSense);
            let cb = on_input.clone();
            if let Ok(conn) = input.connect(&port, "LOOM in", move |_, bytes, _| { if let Some(m) = decode(bytes) { cb(m) } }, ()) {
                inputs.push(conn);
                parts.push(format!("entrée {name}"));
            }
        }
        if let Ok(mut input) = MidiInput::new("LOOM") {
            input.ignore(midir::Ignore::Sysex | midir::Ignore::ActiveSense);
            let cb = on_input.clone();
            if let Ok(conn) = input.create_virtual("LOOM in", move |_, bytes, _| { if let Some(m) = decode(bytes) { cb(m) } }, ()) {
                inputs.push(conn);
            }
        }

        let (tx, rx) = channel::<(Instant, MidiEvent)>();
        std::thread::Builder::new().name("loom-midi".into()).spawn(move || {
            let mut pending: Vec<Timed> = Vec::new();
            loop {
                // wait for new events, or until the next message is due
                // the OS wakes a sleeping thread up to ~2 ms late: sleep until SPIN
                // before the next message, then spin on the clock for the last stretch
                let next = pending.iter().map(|m| m.at).min();
                let wait = next
                    .map(|t| t.saturating_duration_since(Instant::now()).saturating_sub(SPIN))
                    .unwrap_or(Duration::from_secs(1));
                match rx.recv_timeout(wait) {
                    Ok((at, ev)) => {
                        schedule(&mut pending, at, ev);
                        while let Ok((at, ev)) = rx.try_recv() { schedule(&mut pending, at, ev); }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
                if let Some(t) = pending.iter().map(|m| m.at).min() {
                    if t.saturating_duration_since(Instant::now()) <= SPIN {
                        while Instant::now() < t { std::hint::spin_loop(); }
                    }
                }
                let now = Instant::now();
                pending.sort_by_key(|m| m.at);
                while pending.first().is_some_and(|m| m.at <= now) {
                    let m = pending.remove(0);
                    for out in &mut outputs { let _ = out.send(&m.bytes[..m.len]); }
                }
            }
            // closing: silence anything still hanging
            for out in &mut outputs {
                for ch in 0..16u8 { let _ = out.send(&[0xB0 | ch, 123, 0]); }
            }
        }).map_err(|e| e.to_string())?;

        let description = if parts.is_empty() { "MIDI : aucun port".into() } else { format!("MIDI : {}", parts.join(", ")) };
        Ok(MidiIo { tx, _inputs: inputs, description })
    }

    /// Queue an event due at wall-clock time `at`.
    pub fn send(&self, at: Instant, event: MidiEvent) { let _ = self.tx.send((at, event)); }

    /// A handle the audio thread can use to queue events.
    pub fn sender(&self) -> MidiSender { self.tx.clone() }
}

fn schedule(pending: &mut Vec<Timed>, at: Instant, ev: MidiEvent) {
    match ev {
        MidiEvent::Note(n) => {
            let ch = n.channel.clamp(1, 16) - 1;
            pending.push(Timed { at, bytes: [0x90 | ch, n.note, n.velocity], len: 3 });
            let off = at + Duration::from_secs_f32(n.duration.max(0.04));
            pending.push(Timed { at: off, bytes: [0x80 | ch, n.note, 0], len: 3 });
        }
        MidiEvent::Clock { .. } => pending.push(Timed { at, bytes: [0xF8, 0, 0], len: 1 }),
        MidiEvent::Start { .. } => pending.push(Timed { at, bytes: [0xFA, 0, 0], len: 1 }),
        MidiEvent::Stop => pending.push(Timed { at, bytes: [0xFC, 0, 0], len: 1 }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_messages() {
        assert_eq!(decode(&[0x92, 60, 100]), Some(MidiMessageIn::NoteOn { note: 60, velocity: 100 }));
        assert_eq!(decode(&[0x92, 60, 0]), Some(MidiMessageIn::NoteOff { note: 60 }));
        assert_eq!(decode(&[0x80, 61, 40]), Some(MidiMessageIn::NoteOff { note: 61 }));
        assert_eq!(decode(&[0xF8]), Some(MidiMessageIn::Clock));
        assert_eq!(decode(&[0xB0, 7, 100]), Some(MidiMessageIn::Control { cc: 7, value: 100 }));
        assert_eq!(decode(&[0xE0, 0, 64]), None);
    }

    #[test]
    fn follows_clock_tempo() {
        let mut f = ClockFollower::default();
        let t0 = Instant::now();
        let pulse = Duration::from_secs_f64(60.0 / 128.0 / 24.0);
        let mut bpm = None;
        for i in 0..12 { bpm = f.pulse(t0 + pulse * i); }
        assert!((bpm.unwrap() - 128.0).abs() < 0.01);
    }

    #[test]
    fn notes_get_an_off_message() {
        let mut p = Vec::new();
        let t = Instant::now();
        schedule(&mut p, t, MidiEvent::Note(crate::seq::MidiNote { at: 0.0, channel: 3, note: 64, velocity: 90, duration: 0.5 }));
        assert_eq!(p[0].bytes, [0x92, 64, 90]);
        assert_eq!(p[1].bytes, [0x82, 64, 0]);
        assert_eq!(p[1].at.duration_since(t), Duration::from_millis(500));
    }
}
