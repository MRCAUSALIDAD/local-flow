//! Dev tool: exercises a full live session, both tracks, without the UI.
//!
//!     cargo run --release --example session_probe -- [seconds]

use local_flow_lib::config;
use local_flow_lib::live::LiveSession;
use local_flow_lib::session::{Format, Session};
use local_flow_lib::whisper::Transcriber;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn main() {
    let secs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(20);
    let dir = dirs::config_dir().unwrap().join("com.gabriel.local-flow");
    let data = dirs::data_dir().unwrap().join("com.gabriel.local-flow");
    let cfg = config::load(&dir);

    println!("language: {}  capture_mic: {}", cfg.language, cfg.capture_mic);
    println!("vad_silence_ms: {}  max_chunk: {}s", cfg.vad_silence_ms, cfg.live_max_chunk_secs);

    let model = cfg.model_path.clone().expect("no model configured");
    let t = Transcriber::new(&model).expect("model load");

    let session = Arc::new(Session::new());
    session.begin();
    let live = LiveSession::new();

    let seg_session = session.clone();
    let started = Instant::now();
    let started_partial = started;

    live.start(
        t.ctx.clone(),
        &cfg,
        move |segment| {
            let e = seg_session.push(segment);
            let who = match e.track {
                local_flow_lib::stream::Track::Mic => "Me  ",
                local_flow_lib::stream::Track::System => "Them",
            };
            println!("  [{:6.1}s] {who} | {}", e.start_ms as f64 / 1000.0, e.text);
        },
        move |st| {
            if st.lagging || st.dropped > 0 {
                println!(
                    "  !! backlog={} dropped={} elapsed={:.0}s",
                    st.backlog, st.dropped, st.elapsed_ms as f64 / 1000.0
                );
            }
            if !st.listening {
                println!("  (session ended, {:.1}s)", st.elapsed_ms as f64 / 1000.0);
            }
        },
        move |_track, text| {
            if !text.is_empty() {
                println!(
                    "  [{:6.1}s] ~~~~ | {}",
                    started_partial.elapsed().as_secs_f64(),
                    text
                );
            }
        },
    )
    .expect("start");

    println!("\n--- listening {secs}s ---");
    while started.elapsed() < Duration::from_secs(secs) {
        std::thread::sleep(Duration::from_millis(200));
    }
    live.stop();
    // Let the tail drain.
    std::thread::sleep(Duration::from_secs(3));

    println!("\n--- transcript ---");
    println!("{}", session.to_text());
    match session.write(&data, Format::Markdown) {
        Ok(p) => println!("\nwrote {}", p.display()),
        Err(e) => println!("\nexport failed: {e}"),
    }
}
