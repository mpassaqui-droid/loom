//! Live audio output through the system sound server (PipeWire/ALSA via cpal).
//!
//! The UI thread never touches the sequencer directly: it sends small
//! commands down a channel, and reads the transport back from `Status`.

use crate::seq::{Sequencer, Status};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
use loom_core::Program;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

enum Cmd {
    Load(Box<Program>),
    Run(Box<Program>),
    Play,
    Stop,
    Volume(f32),
    Mute(String, bool),
}

/// A running audio engine. Drop it to close the sound device.
pub struct Engine {
    tx: Sender<Cmd>,
    status: Arc<Status>,
    sample_rate: u32,
    device_name: String,
    _stream: cpal::Stream,
}

impl Engine {
    /// Open the default output device and start the (silent) audio stream.
    pub fn start() -> Result<Engine, String> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("aucune sortie audio trouvée")?;
        let device_name = device.description().map(|d| d.to_string()).unwrap_or_else(|_| "sortie audio".into());
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let (tx, rx) = channel();
        let status = Arc::new(Status::default());
        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, &config, rx, status.clone()),
            SampleFormat::F64 => build::<f64>(&device, &config, rx, status.clone()),
            SampleFormat::I16 => build::<i16>(&device, &config, rx, status.clone()),
            SampleFormat::I32 => build::<i32>(&device, &config, rx, status.clone()),
            SampleFormat::U16 => build::<u16>(&device, &config, rx, status.clone()),
            other => return Err(format!("format audio non géré : {other}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Engine { tx, status, sample_rate: config.sample_rate, device_name, _stream: stream })
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
}

fn build<T>(device: &cpal::Device, config: &StreamConfig, rx: Receiver<Cmd>, status: Arc<Status>) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut seq = Sequencer::new(config.sample_rate as f32);
    let mut scratch: Vec<f32> = Vec::new();
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
                    }
                }
                scratch.resize(out.len(), 0.0);
                seq.render(&mut scratch, channels);
                for (o, s) in out.iter_mut().zip(&scratch) { *o = T::from_sample(*s); }
                seq.publish(&status);
            },
            |err| eprintln!("loom audio: {err}"),
            None,
        )
        .map_err(|e| e.to_string())
}
