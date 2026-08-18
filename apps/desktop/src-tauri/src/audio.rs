use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::dsp::{downmix_into, resample_to_16k, TARGET_SR};

#[derive(Clone)]
pub struct Recorder {
    recording: Arc<AtomicBool>,
    samples: Arc<Mutex<Vec<f32>>>,
    source_sr: Arc<AtomicU32>,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            recording: Arc::new(AtomicBool::new(false)),
            samples: Arc::new(Mutex::new(Vec::new())),
            source_sr: Arc::new(AtomicU32::new(TARGET_SR)),
        }
    }

    pub fn is_recording(&self) -> bool {
        self.recording.load(Ordering::SeqCst)
    }

    pub fn start(&self, device_name: Option<String>) {
        if self.is_recording() {
            return;
        }
        self.samples.lock().unwrap().clear();
        self.recording.store(true, Ordering::SeqCst);

        let recording = self.recording.clone();
        let samples = self.samples.clone();
        let source_sr = self.source_sr.clone();
        thread::spawn(move || {
            if let Err(e) = record_loop(&recording, &samples, &source_sr, device_name.as_deref()) {
                eprintln!("[audio] record error: {e}");
                recording.store(false, Ordering::SeqCst);
            }
        });
    }

    pub fn stop(&self) -> Vec<f32> {
        self.recording.store(false, Ordering::SeqCst);
        thread::sleep(Duration::from_millis(120));
        let raw = std::mem::take(&mut *self.samples.lock().unwrap());
        let sr = self.source_sr.load(Ordering::SeqCst).max(1);
        resample_to_16k(&raw, sr)
    }
}

/// cpal 0.18 replaced `Device::name()` with the richer `description()`.
fn device_name(device: &cpal::Device) -> Option<String> {
    device
        .description()
        .ok()
        .map(|desc| desc.name().to_string())
}

pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    host.input_devices()
        .map(|devs| devs.filter_map(|d| device_name(&d)).collect())
        .unwrap_or_default()
}

fn pick_device(host: &cpal::Host, name: Option<&str>) -> Option<cpal::Device> {
    if let Some(name) = name {
        if let Ok(mut devs) = host.input_devices() {
            if let Some(dev) = devs.find(|d| device_name(d).as_deref() == Some(name)) {
                return Some(dev);
            }
        }
    }
    host.default_input_device()
}

fn record_loop(
    recording: &Arc<AtomicBool>,
    samples: &Arc<Mutex<Vec<f32>>>,
    source_sr: &Arc<AtomicU32>,
    device_name: Option<&str>,
) -> anyhow::Result<()> {
    let host = cpal::default_host();
    let device = pick_device(&host, device_name)
        .ok_or_else(|| anyhow::anyhow!("no input device available"))?;
    let supported = device.default_input_config()?;
    let sample_format = supported.sample_format();
    // cpal 0.18 aliases SampleRate to u32, so it is no longer a newtype.
    let sr = supported.sample_rate();
    let channels = supported.channels() as usize;
    source_sr.store(sr, Ordering::SeqCst);

    let config: cpal::StreamConfig = supported.into();
    let err_fn = |e| eprintln!("[audio] stream error: {e}");

    let rec = recording.clone();
    let buf = samples.clone();
    let stream = match sample_format {
        cpal::SampleFormat::F32 => device.build_input_stream(
            config,
            move |data: &[f32], _: &_| push(&rec, &buf, data, channels, |s| s),
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_input_stream(
            config,
            move |data: &[i16], _: &_| push(&rec, &buf, data, channels, |s| s as f32 / 32768.0),
            err_fn,
            None,
        )?,
        cpal::SampleFormat::U16 => device.build_input_stream(
            config,
            move |data: &[u16], _: &_| {
                push(&rec, &buf, data, channels, |s| (s as f32 - 32768.0) / 32768.0)
            },
            err_fn,
            None,
        )?,
        other => return Err(anyhow::anyhow!("unsupported sample format: {other:?}")),
    };

    stream.play()?;
    while recording.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(40));
    }
    Ok(())
}

fn push<T: Copy>(
    recording: &Arc<AtomicBool>,
    buf: &Arc<Mutex<Vec<f32>>>,
    data: &[T],
    channels: usize,
    conv: impl Fn(T) -> f32,
) {
    if !recording.load(Ordering::SeqCst) {
        return;
    }
    let mut g = buf.lock().unwrap();
    downmix_into(data, channels, conv, &mut g);
}
