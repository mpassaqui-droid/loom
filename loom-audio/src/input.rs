//! Microphone input (cpal), for features that listen: HUM (sing or whistle a
//! melody and LOOM writes it), tap-by-voice, tuners.
//!
//! The capture thread only copies mono samples into a bounded buffer; the UI
//! drains it with [`MicInput::take`].

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// Keep at most this many seconds of unread audio.
const MAX_SECONDS: usize = 30;

/// A running microphone capture. Drop it to close the device.
pub struct MicInput {
    buffer: Arc<Mutex<VecDeque<f32>>>,
    level: Arc<AtomicU32>,
    sample_rate: u32,
    device_name: String,
    _stream: cpal::Stream,
}

impl MicInput {
    /// Open the default input device.
    pub fn start() -> Result<MicInput, String> {
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or("aucun micro trouvé")?;
        let device_name = device.description().map(|d| d.to_string()).unwrap_or_else(|_| "micro".into());
        let supported = device.default_input_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let buffer = Arc::new(Mutex::new(VecDeque::new()));
        let level = Arc::new(AtomicU32::new(0));
        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, &config, buffer.clone(), level.clone()),
            SampleFormat::I16 => build::<i16>(&device, &config, buffer.clone(), level.clone()),
            SampleFormat::I32 => build::<i32>(&device, &config, buffer.clone(), level.clone()),
            SampleFormat::U16 => build::<u16>(&device, &config, buffer.clone(), level.clone()),
            other => return Err(format!("format micro non géré : {other}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(MicInput { buffer, level, sample_rate: config.sample_rate, device_name, _stream: stream })
    }

    /// All samples captured since the last call (mono, -1..1).
    pub fn take(&self) -> Vec<f32> {
        self.buffer.lock().map(|mut b| b.drain(..).collect()).unwrap_or_default()
    }

    /// Input peak of the latest block, 0..1 (for a level meter).
    pub fn level(&self) -> f32 { f32::from_bits(self.level.load(Ordering::Relaxed)) }
    pub fn sample_rate(&self) -> u32 { self.sample_rate }
    pub fn device_name(&self) -> &str { &self.device_name }
}

fn build<T>(device: &cpal::Device, config: &StreamConfig, buffer: Arc<Mutex<VecDeque<f32>>>, level: Arc<AtomicU32>) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels.max(1) as usize;
    let cap = config.sample_rate as usize * MAX_SECONDS;
    device
        .build_input_stream::<T, _, _>(
            *config,
            move |data: &[T], _| {
                let mut peak = 0.0f32;
                let Ok(mut buf) = buffer.try_lock() else { return };
                for frame in data.chunks(channels) {
                    let m = frame.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / channels as f32;
                    peak = peak.max(m.abs());
                    if buf.len() >= cap { buf.pop_front(); }
                    buf.push_back(m);
                }
                level.store(peak.to_bits(), Ordering::Relaxed);
            },
            |err| eprintln!("loom micro: {err}"),
            None,
        )
        .map_err(|e| e.to_string())
}
