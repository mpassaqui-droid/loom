//! Live audio output through the system sound server (PipeWire/ALSA via cpal),
//! plus MIDI in/out.
//!
//! The UI thread never touches the sequencer directly: it sends small
//! commands down a channel, and reads the transport back from `Status`.

use crate::midi::{ClockFollower, MidiIo, MidiMessageIn, MidiSender};
use crate::sample::SampleBank;
use crate::seq::{MidiEvent, Sequencer, Status};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
use loom_core::Program;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

enum Cmd {
    Load(Box<Program>),
    Run(Box<Program>),
    Play,
    Stop,
    Volume(f32),
    Mute(String, bool),
    MidiClock(bool),
    Tempo(Option<f64>),
    LiveOn(u8, u8),
    LiveOff(u8),
    Metronome(bool),
    VoiceGain(String, f32),
    VoicePan(String, f32),
    Solo(String, bool),
    Swing(f32),
    Space(f32),
    Echo(f32),
    Eq(f32, f32, f32),
    /// Wet fader of a `send:` return.
    SendLevel(String, f32),
    /// Start (Some) or stop (None) copying the output to a recorder thread.
    Record(Option<Sender<Vec<f32>>>),
}

/// A WAV recording in progress: the thread writing it, and where.
struct Recording {
    path: std::path::PathBuf,
    writer: std::thread::JoinHandle<std::io::Result<()>>,
}

/// A running audio engine. Drop it to close the sound device.
pub struct Engine {
    tx: Sender<Cmd>,
    status: Arc<Status>,
    sample_rate: u32,
    device_name: String,
    samples: usize,
    midi: Result<MidiIo, String>,
    follow: Arc<AtomicBool>,
    recording: Mutex<Option<Recording>>,
    _stream: cpal::Stream,
}

impl Engine {
    /// Open the default output device, the MIDI ports, and start the (silent) audio stream.
    pub fn start() -> Result<Engine, String> {
        let host = cpal::default_host();
        // `LOOM_AUDIO_DEVICE=BlackHole` (part of a name) picks another output, e.g. to feed Live
        let wanted = std::env::var("LOOM_AUDIO_DEVICE").ok().filter(|w| !w.is_empty());
        let named = wanted.as_ref().and_then(|w| {
            host.output_devices().ok()?.find(|d| d.description().map(|n| n.to_string().contains(w.as_str())).unwrap_or(false))
        });
        let device = match named {
            Some(d) => d,
            None => host.default_output_device().ok_or("aucune sortie audio trouvée")?,
        };
        let device_name = device.description().map(|d| d.to_string()).unwrap_or_else(|_| "sortie audio".into());
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let (tx, rx) = channel();
        let status = Arc::new(Status::default());
        // samples are decoded here, never on the audio thread
        let bank = Arc::new(SampleBank::load_default());
        let samples = bank.names().len();

        // MIDI: keyboard notes play live; an external clock can drive the transport
        let follow = Arc::new(AtomicBool::new(false));
        let midi = {
            let tx = tx.clone();
            let status = status.clone();
            let follow = follow.clone();
            let clock = Arc::new(Mutex::new(ClockFollower::default()));
            MidiIo::start(move |m| {
                let following = follow.load(Ordering::Relaxed);
                let _ = match m {
                    MidiMessageIn::NoteOn { note, velocity } => tx.send(Cmd::LiveOn(note, velocity)),
                    MidiMessageIn::NoteOff { note } => tx.send(Cmd::LiveOff(note)),
                    MidiMessageIn::Control { cc, value } => { status.push_control(cc, value); Ok(()) }
                    MidiMessageIn::Clock if following => match clock.lock().ok().and_then(|mut c| c.pulse(Instant::now())) {
                        Some(bpm) => tx.send(Cmd::Tempo(Some(bpm))),
                        None => Ok(()),
                    },
                    MidiMessageIn::Start if following => {
                        if let Ok(mut c) = clock.lock() { c.reset(); }
                        tx.send(Cmd::Play)
                    }
                    MidiMessageIn::Stop if following => tx.send(Cmd::Stop),
                    _ => Ok(()),
                };
            })
        };
        let midi_tx = midi.as_ref().ok().map(|m| m.sender());

        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, &config, rx, status.clone(), bank, midi_tx),
            SampleFormat::F64 => build::<f64>(&device, &config, rx, status.clone(), bank, midi_tx),
            SampleFormat::I16 => build::<i16>(&device, &config, rx, status.clone(), bank, midi_tx),
            SampleFormat::I32 => build::<i32>(&device, &config, rx, status.clone(), bank, midi_tx),
            SampleFormat::U16 => build::<u16>(&device, &config, rx, status.clone(), bank, midi_tx),
            other => return Err(format!("format audio non géré : {other}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Engine { tx, status, sample_rate: config.sample_rate, device_name, samples, midi, follow, recording: Mutex::new(None), _stream: stream })
    }

    /// Parse and load LOOM source. Hot-swaps at the next bar while playing.
    pub fn load(&self, src: &str) { let _ = self.tx.send(Cmd::Load(Box::new(Program::parse(src)))); }
    /// RUN: load the code, and start playing only if stopped. The audio thread
    /// decides, so pressing RUN twice quickly never restarts the song at bar 1.
    pub fn run(&self, src: &str) { let _ = self.tx.send(Cmd::Run(Box::new(Program::parse(src)))); }
    pub fn play(&self) { let _ = self.tx.send(Cmd::Play); }
    pub fn stop(&self) { let _ = self.tx.send(Cmd::Stop); }
    pub fn set_volume(&self, v: f32) { let _ = self.tx.send(Cmd::Volume(v)); }
    pub fn set_muted(&self, voice: &str, muted: bool) { let _ = self.tx.send(Cmd::Mute(voice.to_string(), muted)); }
    pub fn status(&self) -> &Status { &self.status }
    pub fn sample_rate(&self) -> u32 { self.sample_rate }
    pub fn device_name(&self) -> &str { &self.device_name }
    /// How many samples were found (`bd`, `amen`…).
    pub fn sample_count(&self) -> usize { self.samples }

    /// Send MIDI clock + start/stop so a DAW or drum machine follows LOOM.
    pub fn set_midi_clock(&self, on: bool) { let _ = self.tx.send(Cmd::MidiClock(on)); }
    /// Let an external MIDI clock drive LOOM's tempo, start and stop.
    pub fn set_midi_follow(&self, on: bool) {
        self.follow.store(on, Ordering::Relaxed);
        if !on { let _ = self.tx.send(Cmd::Tempo(None)); }
    }
    /// Play a note live with the `lead` voice (on-screen or computer keyboard).
    pub fn live_note_on(&self, note: u8, velocity: u8) { let _ = self.tx.send(Cmd::LiveOn(note, velocity)); }
    pub fn live_note_off(&self, note: u8) { let _ = self.tx.send(Cmd::LiveOff(note)); }
    /// Mixer fader for a voice, 0..2 (1 = as written).
    pub fn set_voice_gain(&self, voice: &str, gain: f32) { let _ = self.tx.send(Cmd::VoiceGain(voice.into(), gain)); }
    /// Mixer pan for a voice, -1..1.
    pub fn set_voice_pan(&self, voice: &str, pan: f32) { let _ = self.tx.send(Cmd::VoicePan(voice.into(), pan)); }
    /// Mixer solo for a voice.
    pub fn set_solo(&self, voice: &str, solo: bool) { let _ = self.tx.send(Cmd::Solo(voice.into(), solo)); }
    /// The SWING knob, 0..0.9, for voices without their own `| swing`.
    pub fn set_swing(&self, swing: f32) { let _ = self.tx.send(Cmd::Swing(swing)); }

    /// SPACE knob 0..100 (browser default 20): reverb on the whole mix.
    pub fn set_space(&self, knob: f32) { let _ = self.tx.send(Cmd::Space(knob)); }
    /// ECHO knob 0..100 (browser default 15): ping-pong delay on the whole mix.
    pub fn set_echo(&self, knob: f32) { let _ = self.tx.send(Cmd::Echo(knob)); }
    /// Master EQ in dB (-18..18): low shelf, mid peak, high shelf (browser defaults 0, 0, 2).
    pub fn set_eq(&self, low: f32, mid: f32, high: f32) { let _ = self.tx.send(Cmd::Eq(low, mid, high)); }
    /// Wet fader of a `send:` return, 0..1.
    pub fn set_send_level(&self, name: &str, level: f32) { let _ = self.tx.send(Cmd::SendLevel(name.to_string(), level)); }

    /// Tick on every beat.
    pub fn set_metronome(&self, on: bool) { let _ = self.tx.send(Cmd::Metronome(on)); }

    /// REC: record everything LOOM plays (live edits included) into a WAV file.
    pub fn start_recording(&self, path: &std::path::Path) -> Result<(), String> {
        let mut rec = self.recording.lock().map_err(|e| e.to_string())?;
        if rec.is_some() { return Err("un enregistrement est déjà en cours".into()); }
        if let Some(dir) = path.parent() { std::fs::create_dir_all(dir).map_err(|e| e.to_string())?; }
        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let (tx, rx) = channel::<Vec<f32>>();
        let sr = self.sample_rate;
        let writer = std::thread::Builder::new().name("loom-rec".into())
            .spawn(move || write_wav_stream(file, sr, rx))
            .map_err(|e| e.to_string())?;
        let _ = self.tx.send(Cmd::Record(Some(tx)));
        *rec = Some(Recording { path: path.to_path_buf(), writer });
        Ok(())
    }

    /// Stop recording; returns the finished file.
    pub fn stop_recording(&self) -> Result<std::path::PathBuf, String> {
        let rec = self.recording.lock().map_err(|e| e.to_string())?.take().ok_or("aucun enregistrement en cours")?;
        // dropping the audio thread's sender ends the writer loop
        let _ = self.tx.send(Cmd::Record(None));
        rec.writer.join().map_err(|_| "l'enregistreur a planté".to_string())?.map_err(|e| e.to_string())?;
        Ok(rec.path)
    }

    pub fn is_recording(&self) -> bool { self.recording.lock().map(|r| r.is_some()).unwrap_or(false) }

    /// What MIDI ports are connected (or why not), in French for the status bar.
    pub fn midi_description(&self) -> String {
        match &self.midi { Ok(m) => m.description.clone(), Err(e) => format!("MIDI indisponible : {e}") }
    }
}

fn build<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    rx: Receiver<Cmd>,
    status: Arc<Status>,
    bank: Arc<SampleBank>,
    midi: Option<MidiSender>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut seq = Sequencer::new(config.sample_rate as f32, bank);
    let mut scratch: Vec<f32> = Vec::new();
    let mut recorder: Option<Sender<Vec<f32>>> = None;
    device
        .build_output_stream::<T, _, _>(
            *config,
            move |out: &mut [T], _| {
                while let Ok(cmd) = rx.try_recv() {
                    match cmd {
                        Cmd::Load(p) => seq.set_program(*p),
                        Cmd::Run(p) => {
                            seq.set_program(*p);
                            if !seq.is_playing() { seq.play(); }
                        }
                        Cmd::Play => seq.play(),
                        Cmd::Stop => seq.stop(),
                        Cmd::Volume(v) => seq.set_volume(v),
                        Cmd::Mute(name, m) => seq.set_muted(&name, m),
                        Cmd::MidiClock(on) => seq.set_midi_clock(on),
                        Cmd::Tempo(bpm) => seq.set_bpm_override(bpm),
                        Cmd::LiveOn(n, v) => seq.live_note_on(n, v),
                        Cmd::LiveOff(n) => seq.live_note_off(n),
                        Cmd::Metronome(on) => seq.set_metronome(on),
                        Cmd::VoiceGain(v, g) => seq.set_voice_gain(&v, g),
                        Cmd::VoicePan(v, p) => seq.set_voice_pan(&v, p),
                        Cmd::Solo(v, on) => seq.set_solo(&v, on),
                        Cmd::Swing(x) => seq.set_swing(x),
                        Cmd::Space(x) => seq.set_space(x),
                        Cmd::Echo(x) => seq.set_echo(x),
                        Cmd::Eq(l, m, h) => seq.set_eq(l, m, h),
                        Cmd::SendLevel(n, l) => seq.set_send_level(&n, l),
                        Cmd::Record(r) => recorder = r,
                    }
                }
                let start = seq.transport_seconds();
                let now = Instant::now();
                scratch.resize(out.len(), 0.0);
                seq.render(&mut scratch, channels);
                for (o, s) in out.iter_mut().zip(&scratch) { *o = T::from_sample(*s); }
                if let Some(rec) = &recorder {
                    // stereo copy of this block for the WAV writer thread
                    let stereo: Vec<f32> = scratch.chunks(channels.max(1))
                        .flat_map(|f| [f[0], *f.get(1).unwrap_or(&f[0])])
                        .collect();
                    if rec.send(stereo).is_err() { recorder = None; }
                }
                // hand MIDI to its own thread, converting transport time to wall-clock time
                for ev in seq.midi_out.drain(..) {
                    let Some(tx) = &midi else { continue };
                    let at = match ev {
                        MidiEvent::Note(n) => n.at,
                        MidiEvent::Clock { at } | MidiEvent::Start { at } => at,
                        MidiEvent::Stop => start,
                    };
                    let ahead = (at - start).clamp(0.0, 60.0);
                    let _ = tx.send((now + Duration::from_secs_f64(ahead), ev));
                }
                status.feed_visuals(&mut seq.hits, &seq.levels, &scratch, channels);
                seq.publish(&status);
            },
            |err| eprintln!("loom audio: {err}"),
            None,
        )
        .map_err(|e| e.to_string())
}

/// Stream stereo f32 blocks into a 16-bit WAV, fixing the header sizes at the end.
fn write_wav_stream(file: std::fs::File, sample_rate: u32, rx: Receiver<Vec<f32>>) -> std::io::Result<()> {
    use std::io::{BufWriter, Seek, SeekFrom, Write};
    let mut w = BufWriter::new(file);
    let header = |data_len: u32| {
        let mut h = Vec::with_capacity(44);
        h.extend_from_slice(b"RIFF");
        h.extend_from_slice(&(36 + data_len).to_le_bytes());
        h.extend_from_slice(b"WAVEfmt ");
        h.extend_from_slice(&16u32.to_le_bytes());
        h.extend_from_slice(&1u16.to_le_bytes());
        h.extend_from_slice(&2u16.to_le_bytes());
        h.extend_from_slice(&sample_rate.to_le_bytes());
        h.extend_from_slice(&(sample_rate * 4).to_le_bytes());
        h.extend_from_slice(&4u16.to_le_bytes());
        h.extend_from_slice(&16u16.to_le_bytes());
        h.extend_from_slice(b"data");
        h.extend_from_slice(&data_len.to_le_bytes());
        h
    };
    w.write_all(&header(0))?;
    let mut data_len: u32 = 0;
    for block in rx {
        for s in &block {
            w.write_all(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
        }
        data_len = data_len.saturating_add(block.len() as u32 * 2);
    }
    w.seek(SeekFrom::Start(0))?;
    w.write_all(&header(data_len))?;
    w.flush()
}
