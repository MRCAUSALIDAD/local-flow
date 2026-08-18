//! System audio capture — the sound the computer is playing back, rather than
//! the microphone. Used to transcribe calls, videos and anything else the user
//! is listening to.
//!
//! cpal 0.18 implements loopback on every desktop platform, but the device to
//! ask for differs:
//!
//! * macOS   — build an *input* stream on an *output* device. cpal creates a
//!             Core Audio process tap plus a private aggregate device behind
//!             the scenes, and tears them down when the stream drops.
//!             Requires macOS 14.6 or newer.
//! * Windows — same shape: an input stream on a render endpoint makes cpal set
//!             `AUDCLNT_STREAMFLAGS_LOOPBACK`.
//! * Linux   — different: PulseAudio and PipeWire already expose each sink's
//!             output as a `.monitor` *source*, so it is a plain input device.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::dsp::{downmix_into, StreamResampler};

/// Roughly 20 seconds of 16 kHz audio in typical callback-sized blocks. If the
/// transcriber falls this far behind, dropping is better than growing without
/// bound during a long meeting.
const QUEUE_CAPACITY: usize = 2_000;

/// A block of captured audio, already mono and resampled to 16 kHz.
pub struct AudioBlock {
    pub samples: Vec<f32>,
    /// Wall-clock instant the block was captured.
    ///
    /// Sample counting cannot be used to derive timing here: a loopback stream
    /// delivers no callbacks at all while the system is silent, so the sample
    /// clock stops rather than filling with zeroes.
    pub captured_at: Instant,
}

#[derive(Serialize, Clone, Debug)]
pub struct LoopbackSource {
    pub name: String,
    pub is_default: bool,
}

/// What a capture stream should listen to.
#[derive(Clone, Debug)]
pub enum Target {
    /// The computer's own output.
    System(Option<String>),
    /// A microphone, streamed continuously.
    ///
    /// The dictation recorder also reads a microphone, but it buffers a whole
    /// press-and-hold and hands it over at the end. A live session needs the
    /// same continuous, bounded delivery as the system stream.
    Mic(Option<String>),
}

/// Whether this platform build can capture system audio at all.
///
/// The only current restriction is the macOS version: Core Audio process taps
/// landed in 14.x and cpal requires 14.6 or newer.
pub fn availability() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let version = sysinfo::System::os_version()
            .ok_or_else(|| "Could not determine the macOS version.".to_string())?;
        if !macos_version_supported(&version) {
            return Err(format!(
                "System audio capture needs macOS 14.6 or newer (this Mac runs {version})."
            ));
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_version_supported(version: &str) -> bool {
    let mut parts = version.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);
    major > 14 || (major == 14 && minor >= 6)
}

/// Lists the devices that can actually be tapped for system audio.
///
/// On macOS and Windows the candidates are output devices that report no input
/// support; a device offering both (a virtual audio driver, say) would be
/// opened as an ordinary input and would capture the wrong thing. On Linux the
/// candidates are the `.monitor` sources.
pub fn list_sources() -> Vec<LoopbackSource> {
    let host = cpal::default_host();
    let default_name = default_source(&host).and_then(|d| device_name(&d));

    let devices: Vec<cpal::Device> = if cfg!(target_os = "linux") {
        host.input_devices()
            .map(|d| d.filter(is_monitor_source).collect())
            .unwrap_or_default()
    } else {
        host.output_devices()
            .map(|d| d.filter(|dev| !dev.supports_input()).collect())
            .unwrap_or_default()
    };

    devices
        .iter()
        .filter_map(device_name)
        .map(|name| LoopbackSource {
            is_default: Some(&name) == default_name.as_ref(),
            name,
        })
        .collect()
}

fn device_name(device: &cpal::Device) -> Option<String> {
    device
        .description()
        .ok()
        .map(|desc| desc.name().to_string())
}

fn is_monitor_source(device: &cpal::Device) -> bool {
    device_name(device)
        .map(|n| n.ends_with(".monitor"))
        .unwrap_or(false)
}

fn default_source(host: &cpal::Host) -> Option<cpal::Device> {
    if cfg!(target_os = "linux") {
        // Prefer the monitor of the default sink, which PulseAudio names after
        // it; fall back to any monitor at all.
        let sink = host.default_output_device().and_then(|d| device_name(&d));
        let mut monitors = host.input_devices().ok()?.filter(is_monitor_source);
        if let Some(sink) = sink {
            let preferred = monitors.find(|m| {
                device_name(m)
                    .map(|n| n.starts_with(&sink) || n == format!("{sink}.monitor"))
                    .unwrap_or(false)
            });
            if preferred.is_some() {
                return preferred;
            }
        }
        host.input_devices().ok()?.find(is_monitor_source)
    } else {
        host.default_output_device().filter(|d| !d.supports_input())
    }
}

fn pick_target(host: &cpal::Host, target: &Target) -> Option<cpal::Device> {
    match target {
        Target::System(name) => {
            if let Some(name) = name {
                let mut all = host
                    .output_devices()
                    .ok()
                    .into_iter()
                    .flatten()
                    .chain(host.input_devices().ok().into_iter().flatten());
                if let Some(dev) = all.find(|d| device_name(d).as_deref() == Some(name)) {
                    return Some(dev);
                }
            }
            default_source(host)
        }
        Target::Mic(name) => {
            if let Some(name) = name {
                if let Ok(mut devs) = host.input_devices() {
                    if let Some(dev) = devs.find(|d| device_name(d).as_deref() == Some(name)) {
                        return Some(dev);
                    }
                }
            }
            host.default_input_device()
        }
    }
}

pub struct StreamCapture {
    running: Arc<AtomicBool>,
    dropped: Arc<AtomicU64>,
}

impl StreamCapture {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            dropped: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Number of blocks discarded because the consumer could not keep up.
    pub fn dropped_blocks(&self) -> u64 {
        self.dropped.load(Ordering::SeqCst)
    }

    /// Starts capturing, returning the receiving end of the audio queue.
    ///
    /// Unlike the microphone recorder, device errors surface here rather than
    /// being logged from a worker thread: opening a loopback device is the step
    /// most likely to fail (missing permission, unsupported OS), and the user
    /// needs to be told immediately.
    pub fn start(&self, target: Target) -> Result<Receiver<AudioBlock>, String> {
        if matches!(target, Target::System(_)) {
            availability()?;
        }
        if self.is_running() {
            return Err("This capture stream is already running.".into());
        }

        let (tx, rx) = sync_channel::<AudioBlock>(QUEUE_CAPACITY);
        let (ready_tx, ready_rx) = sync_channel::<Result<(), String>>(1);

        self.dropped.store(0, Ordering::SeqCst);
        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let dropped = self.dropped.clone();
        thread::spawn(move || {
            let outcome = capture_loop(&target, tx, &running, &dropped, &ready_tx);
            if let Err(e) = outcome {
                // Only reaches here for a failure after start-up was reported.
                eprintln!("[loopback] {e}");
            }
            running.store(false, Ordering::SeqCst);
        });

        // Generous: creating a Core Audio process tap and its aggregate device
        // can take many seconds when coreaudiod is still tearing down a
        // previous one. Ten seconds was tight enough to fail in practice.
        match ready_rx.recv_timeout(Duration::from_secs(30)) {
            Ok(Ok(())) => Ok(rx),
            Ok(Err(e)) => {
                self.running.store(false, Ordering::SeqCst);
                Err(e)
            }
            Err(_) => {
                self.running.store(false, Ordering::SeqCst);
                Err("Timed out opening the audio device. Try again in a moment.".into())
            }
        }
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

fn capture_loop(
    target: &Target,
    tx: SyncSender<AudioBlock>,
    running: &Arc<AtomicBool>,
    dropped: &Arc<AtomicU64>,
    ready: &SyncSender<Result<(), String>>,
) -> Result<(), String> {
    let host = cpal::default_host();

    let build = || -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
        let device = pick_target(&host, target)
            .ok_or_else(|| match target {
                Target::System(_) => "No system audio source available.".to_string(),
                Target::Mic(_) => "No microphone available.".to_string(),
            })?;
        // A monitor source is a real input; a tapped output device is not, and
        // its stream follows the output format.
        let supported = if device.supports_input() {
            device.default_input_config()
        } else {
            device.default_output_config()
        }
        .map_err(|e| format!("Could not read the device configuration: {e}"))?;
        Ok((device, supported))
    };

    let (device, supported) = match build() {
        Ok(v) => v,
        Err(e) => {
            let _ = ready.send(Err(e.clone()));
            return Err(e);
        }
    };

    let sample_format = supported.sample_format();
    let source_sr = supported.sample_rate();
    let channels = supported.channels() as usize;
    let config: cpal::StreamConfig = supported.into();

    let stream = match build_stream(
        &device, config, sample_format, channels, source_sr, tx, dropped.clone(),
    ) {
        Ok(s) => s,
        Err(e) => {
            let _ = ready.send(Err(e.clone()));
            return Err(e);
        }
    };

    if let Err(e) = stream.play() {
        let e = format!("Could not start the audio stream: {e}");
        let _ = ready.send(Err(e.clone()));
        return Err(e);
    }

    let _ = ready.send(Ok(()));

    while running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(40));
    }
    Ok(())
}

fn build_stream(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    sample_format: cpal::SampleFormat,
    channels: usize,
    source_sr: u32,
    tx: SyncSender<AudioBlock>,
    dropped: Arc<AtomicU64>,
) -> Result<cpal::Stream, String> {
    let err_fn = |e| eprintln!("[loopback] stream error: {e}");

    macro_rules! stream {
        ($sample:ty, $conv:expr) => {{
            let mut pump = Pump::new(source_sr, channels, tx, dropped);
            device.build_input_stream(
                config,
                move |data: &[$sample], _: &_| pump.feed(data, $conv),
                err_fn,
                None,
            )
        }};
    }

    let result = match sample_format {
        cpal::SampleFormat::F32 => stream!(f32, |s| s),
        cpal::SampleFormat::I16 => stream!(i16, |s| s as f32 / 32768.0),
        cpal::SampleFormat::U16 => stream!(u16, |s| (s as f32 - 32768.0) / 32768.0),
        other => return Err(format!("Unsupported sample format: {other:?}")),
    };

    result.map_err(|e| format!("Could not open the system audio device: {e}"))
}

/// Per-stream state: downmix to mono, resample to 16 kHz, hand blocks off.
struct Pump {
    resampler: StreamResampler,
    channels: usize,
    mono: Vec<f32>,
    tx: SyncSender<AudioBlock>,
    dropped: Arc<AtomicU64>,
}

impl Pump {
    fn new(
        source_sr: u32,
        channels: usize,
        tx: SyncSender<AudioBlock>,
        dropped: Arc<AtomicU64>,
    ) -> Self {
        Self {
            resampler: StreamResampler::new(source_sr),
            channels,
            mono: Vec::new(),
            tx,
            dropped,
        }
    }

    fn feed<T: Copy>(&mut self, data: &[T], conv: impl Fn(T) -> f32) {
        self.mono.clear();
        downmix_into(data, self.channels, conv, &mut self.mono);
        let samples = self.resampler.push(&self.mono);
        if samples.is_empty() {
            return;
        }
        let block = AudioBlock {
            samples,
            captured_at: Instant::now(),
        };
        // Never block here: this runs on the real-time audio thread.
        match self.tx.try_send(block) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
            Err(TrySendError::Disconnected(_)) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(target_os = "macos")]
    fn macos_version_gate() {
        use super::macos_version_supported;
        assert!(!macos_version_supported("13.7"));
        assert!(!macos_version_supported("14.5"));
        assert!(macos_version_supported("14.6"));
        assert!(macos_version_supported("15.0"));
        assert!(macos_version_supported("26.5.1"));
    }
}
