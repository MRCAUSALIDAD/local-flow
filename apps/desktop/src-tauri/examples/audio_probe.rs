//! Dev tool: verifies system audio capture against real hardware.
//!
//! Run it with something audible playing:
//!     cargo run --example audio_probe

use local_flow_lib::loopback::{self, LoopbackRecorder};
use std::time::{Duration, Instant};

fn main() {
    println!("availability: {:?}\n", loopback::availability());

    let sources = loopback::list_sources();
    println!("--- system audio sources ---");
    if sources.is_empty() {
        println!("  (none found)");
    }
    for s in &sources {
        println!("  {}{}", s.name, if s.is_default { "  [default]" } else { "" });
    }

    println!("\n--- capturing 5s of system audio ---");
    println!("(play something audible now)");

    let recorder = LoopbackRecorder::new();
    let rx = match recorder.start(None) {
        Ok(rx) => rx,
        Err(e) => {
            println!("  FAILED to start: {e}");
            return;
        }
    };

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut samples = 0usize;
    let mut sq = 0f64;
    let mut blocks = 0usize;
    let mut first_at: Option<Instant> = None;
    let mut last_at: Option<Instant> = None;

    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(block) => {
                blocks += 1;
                samples += block.samples.len();
                sq += block.samples.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>();
                first_at.get_or_insert(block.captured_at);
                last_at = Some(block.captured_at);
            }
            Err(_) => println!("  ...no audio (system is silent)"),
        }
    }

    recorder.stop();

    println!("\n--- result ---");
    println!("  blocks:  {blocks}");
    println!("  samples: {samples}  ({:.2}s at 16 kHz)", samples as f64 / 16_000.0);
    println!("  dropped: {}", recorder.dropped_blocks());
    if let (Some(a), Some(b)) = (first_at, last_at) {
        println!("  wall clock span: {:.2}s", (b - a).as_secs_f64());
    }
    if samples > 0 {
        let rms = (sq / samples as f64).sqrt();
        println!("  rms: {rms:.6}  -> {}", if rms > 1e-5 { "AUDIO" } else { "silence" });
    } else {
        println!("  no samples captured");
    }
}
