//! Temporary probe: verifies the cpal 0.18 migration against real hardware and
//! shows which devices are usable as system-audio (loopback) sources.

use cpal::traits::{DeviceTrait, HostTrait};

fn main() {
    let host = cpal::default_host();
    println!("host: {}\n", host.id());

    println!("--- input devices (microphones) ---");
    match host.input_devices() {
        Ok(devs) => {
            for d in devs {
                let name = d
                    .description()
                    .map(|x| x.name().to_string())
                    .unwrap_or_else(|_| "<no description>".into());
                let cfg = d
                    .default_input_config()
                    .map(|c| format!("{} ch, {} Hz, {:?}", c.channels(), c.sample_rate(), c.sample_format()))
                    .unwrap_or_else(|e| format!("<{e}>"));
                println!("  {name}  [{cfg}]");
            }
        }
        Err(e) => println!("  error: {e}"),
    }

    println!("\n--- output devices (loopback candidates on macOS/Windows) ---");
    match host.output_devices() {
        Ok(devs) => {
            for d in devs {
                let name = d
                    .description()
                    .map(|x| x.name().to_string())
                    .unwrap_or_else(|_| "<no description>".into());
                println!(
                    "  {name}  [supports_input={}, supports_output={}]",
                    d.supports_input(),
                    d.supports_output()
                );
            }
        }
        Err(e) => println!("  error: {e}"),
    }

    println!("\n--- default devices ---");
    println!(
        "  input:  {:?}",
        host.default_input_device()
            .and_then(|d| d.description().ok().map(|x| x.name().to_string()))
    );
    println!(
        "  output: {:?}",
        host.default_output_device()
            .and_then(|d| d.description().ok().map(|x| x.name().to_string()))
    );

    println!("\n--- 2s capture from default INPUT (microphone) ---");
    if let Some(d) = host.default_input_device() {
        report(capture(&d, "microphone"));
    }

    println!("\n--- 2s capture from default OUTPUT (system audio loopback) ---");
    println!("(play something audible now to get a meaningful level)");
    if let Some(d) = host.default_output_device() {
        report(capture(&d, "loopback"));
    }
}

fn report(result: Result<(usize, f32), String>) {
    match result {
        Ok((frames, rms)) => {
            let verdict = if rms > 1e-5 { "AUDIO" } else { "silence" };
            println!("  {frames} frames captured, rms={rms:.6}  -> {verdict}");
        }
        Err(e) => println!("  FAILED: {e}"),
    }
}

/// Captures ~2s and returns (frame count, RMS). Works for both a normal input
/// device and an output device, which cpal 0.18 transparently turns into a
/// loopback capture.
fn capture(device: &cpal::Device, label: &str) -> Result<(usize, f32), String> {
    use cpal::traits::StreamTrait;
    use std::sync::{Arc, Mutex};

    let supported = if device.supports_input() {
        device.default_input_config()
    } else {
        // Loopback: the tap follows the device's *output* format.
        device.default_output_config()
    }
    .map_err(|e| format!("no config for {label}: {e}"))?;

    println!(
        "  config: {} ch, {} Hz, {:?}",
        supported.channels(),
        supported.sample_rate(),
        supported.sample_format()
    );

    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();

    let acc: Arc<Mutex<(usize, f64)>> = Arc::new(Mutex::new((0, 0.0)));
    let sink = acc.clone();

    if sample_format != cpal::SampleFormat::F32 {
        return Err(format!("unexpected sample format {sample_format:?}"));
    }

    let stream = device
        .build_input_stream(
            config,
            move |data: &[f32], _: &_| {
                let mut g = sink.lock().unwrap();
                g.0 += data.len();
                g.1 += data.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>();
            },
            |e| eprintln!("  stream error: {e}"),
            None,
        )
        .map_err(|e| format!("build_input_stream: {e}"))?;

    stream.play().map_err(|e| format!("play: {e}"))?;
    std::thread::sleep(std::time::Duration::from_secs(2));
    drop(stream);

    let (frames, sq) = *acc.lock().unwrap();
    if frames == 0 {
        return Err("no frames delivered".into());
    }
    Ok((frames, (sq / frames as f64).sqrt() as f32))
}
