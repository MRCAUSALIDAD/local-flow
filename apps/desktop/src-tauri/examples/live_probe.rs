//! Dev tool: end-to-end live transcription of system audio.
//!
//!     cargo run --release --example live_probe -- [seconds]
//!
//! Plays back nothing itself: start a video or a call first.

use local_flow_lib::loopback::{StreamCapture, Target};
use local_flow_lib::stream::{Track, VadChunker, VadConfig};
use local_flow_lib::whisper::{self, Options, Transcriber};
use std::time::{Duration, Instant};

fn main() {
    let secs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);

    let dir = dirs::data_dir()
        .unwrap()
        .join("com.gabriel.local-flow");
    let cfg: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("config.json")).unwrap()).unwrap();
    let model = cfg["model_path"].as_str().expect("no model configured");
    let language = cfg["language"].as_str().unwrap_or("auto").to_string();

    println!("model:    {model}");
    println!("language: {language}");
    println!("threads:  {}", whisper::live_threads());

    let t0 = Instant::now();
    let transcriber = Transcriber::new(model).expect("failed to load model");
    println!("model loaded in {:.1}s\n", t0.elapsed().as_secs_f64());

    let recorder = StreamCapture::new();
    let rx = match recorder.start(Target::System(None)) {
        Ok(rx) => rx,
        Err(e) => {
            println!("FAILED to start capture: {e}");
            return;
        }
    };

    let mut chunker = VadChunker::new(Track::System, VadConfig::default());
    let session_start = Instant::now();
    let deadline = session_start + Duration::from_secs(secs);

    println!("--- listening for {secs}s, play something now ---\n");

    let mut audio_total = 0.0f64;
    let mut infer_total = 0.0f64;
    let mut count = 0;

    let mut handle = |u: &local_flow_lib::stream::Utterance, transcriber: &Transcriber| {
        let audio_secs = u.duration().as_secs_f64();
        let t = Instant::now();
        let opts = Options {
            language: &language,
            initial_prompt: None,
            n_threads: Some(whisper::live_threads()),
            no_context: true,
        };
        let text = whisper::transcribe_with(transcriber.ctx.clone(), &u.samples, &opts);
        let infer_secs = t.elapsed().as_secs_f64();
        audio_total += audio_secs;
        infer_total += infer_secs;
        count += 1;
        let at = u.started_at.saturating_duration_since(session_start).as_secs_f64();
        match text {
            Ok(t) if t.is_empty() => println!("[{at:5.1}s] ({audio_secs:.1}s audio, rtf {:.2}) <empty>", infer_secs / audio_secs.max(0.001)),
            Ok(t) => println!("[{at:5.1}s] ({audio_secs:.1}s audio, rtf {:.2}) {t}", infer_secs / audio_secs.max(0.001)),
            Err(e) => println!("[{at:5.1}s] ERROR: {e}"),
        }
    };

    while Instant::now() < deadline {
        if let Ok(block) = rx.recv_timeout(Duration::from_millis(500)) {
            for u in chunker.push(&block.samples, block.captured_at) {
                handle(&u, &transcriber);
            }
        }
    }
    if let Some(u) = chunker.flush() {
        handle(&u, &transcriber);
    }
    recorder.stop();

    println!("\n--- summary ---");
    println!("  utterances:     {count}");
    println!("  audio:          {audio_total:.1}s");
    println!("  inference:      {infer_total:.1}s");
    if audio_total > 0.0 {
        let rtf = infer_total / audio_total;
        println!("  overall rtf:    {rtf:.2}  -> {}", if rtf < 1.0 { "keeps up with real time" } else { "TOO SLOW" });
    }
    println!("  dropped blocks: {}", recorder.dropped_blocks());
}
