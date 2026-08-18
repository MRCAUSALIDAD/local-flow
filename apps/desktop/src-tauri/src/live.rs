//! Runs a live listening session: two capture streams in, transcript out.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::loopback::{StreamCapture, Target};
use crate::stream::{
    ActivityLog, PartialWorker, Track, TranscriptionWorker, Utterance, VadChunker, VadConfig,
};
use crate::whisper::{self, Options};
use whisper_rs::WhisperContext;

/// How often the session reports progress to the UI.
const STATE_INTERVAL: Duration = Duration::from_millis(750);

/// Backlog above which the UI is told transcription is lagging. Utterances are
/// seconds of audio each, so even a couple waiting means falling behind.
const LAG_THRESHOLD: usize = 3;

/// Share of a microphone utterance that the system track must also have been
/// speaking through before it is treated as bleed rather than the user.
///
/// Set high on purpose: dropping something the user actually said is worse
/// than letting one echoed line through.
const ECHO_COVERAGE: f32 = 0.8;

/// Trimmed off each end before measuring overlap. An utterance carries pre-roll
/// at the front and the silence that closed it at the back; counting those
/// dilutes the overlap and lets echo through.
const ECHO_EDGE_TRIM: Duration = Duration::from_millis(400);

/// Shortest gap between interim transcriptions of the same ongoing speech.
///
/// Refreshed faster than this the text mostly rewrites itself, and every pass
/// re-transcribes the whole utterance so far, which grows as the speaker
/// continues.
const PARTIAL_INTERVAL: Duration = Duration::from_millis(900);


#[derive(Clone, serde::Serialize)]
pub struct LiveState {
    pub listening: bool,
    pub elapsed_ms: u64,
    pub backlog: usize,
    pub lagging: bool,
    pub dropped: u64,
    /// Whether any audio has arrived yet.
    ///
    /// A loopback stream is silent until something plays, so "no audio yet" is
    /// an ordinary state and must not be reported as a failure.
    pub receiving: bool,
}

pub struct LiveSession {
    running: Arc<AtomicBool>,
    dropped: Arc<AtomicU64>,
}

impl LiveSession {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            dropped: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Starts capture and transcription.
    ///
    /// `on_segment` receives each transcribed piece, `on_state` periodic
    /// progress. Both are called from worker threads.
    pub fn start<S, T, P>(
        &self,
        ctx: Arc<WhisperContext>,
        cfg: &Config,
        on_segment: S,
        on_state: T,
        on_partial: P,
    ) -> Result<(), String>
    where
        S: Fn(crate::stream::LiveSegment) + Send + Sync + 'static,
        T: Fn(LiveState) + Send + 'static,
        P: Fn(Track, String) + Send + Sync + 'static,
    {
        if self.is_running() {
            return Err("Already listening.".into());
        }

        let vad = VadConfig {
            hang: Duration::from_millis(cfg.vad_silence_ms.clamp(200, 3_000)),
            max_len: Duration::from_secs(cfg.live_max_chunk_secs.clamp(5, 28)),
            ..VadConfig::default()
        };

        let system = StreamCapture::new();
        let system_rx = system.start(Target::System(cfg.loopback_source.clone()))?;

        // The microphone is optional and must not sink the session: capturing
        // the far side of a call is still useful on its own.
        let mic = StreamCapture::new();
        let mic_rx = if cfg.capture_mic {
            match mic.start(Target::Mic(cfg.input_device.clone())) {
                Ok(rx) => Some(rx),
                Err(e) => {
                    eprintln!("[live] microphone unavailable, continuing without it: {e}");
                    None
                }
            }
        } else {
            None
        };

        self.dropped.store(0, Ordering::SeqCst);
        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let dropped = self.dropped.clone();
        let language = cfg.language.clone();
        let suppress_echo = cfg.suppress_mic_echo && cfg.capture_mic;
        let want_partials = cfg.live_partials;
        let session_start = Instant::now();

        thread::spawn(move || {
            let threads = whisper::live_threads();
            let ctx_for_partials = ctx.clone();
            let language_for_partials = language.clone();
            let transcribe = Box::new(move |u: &Utterance, prompt: Option<&str>| {
                let opts = Options {
                    language: &language,
                    initial_prompt: prompt,
                    n_threads: Some(threads),
                    no_context: true,
                };
                whisper::transcribe_with(ctx.clone(), &u.samples, &opts)
                    .map_err(|e| e.to_string())
            });

            let worker = TranscriptionWorker::spawn(transcribe, session_start, on_segment);

            let on_partial = Arc::new(on_partial);
            let partial_emit = on_partial.clone();
            let partial_ctx = ctx_for_partials;
            let partial_lang = language_for_partials;
            let partials = PartialWorker::spawn(
                Box::new(move |p| {
                    let opts = Options {
                        language: &partial_lang,
                        initial_prompt: None,
                        n_threads: Some(threads),
                        no_context: true,
                    };
                    whisper::transcribe_with(partial_ctx.clone(), &p.samples, &opts)
                        .map_err(|e| e.to_string())
                }),
                move |track, text| partial_emit(track, text),
            );
            let mut last_partial = Instant::now();

            // The system track publishes when it was speaking so microphone
            // utterances can be checked against it.
            let system_activity = Arc::new(ActivityLog::new(Duration::from_secs(60)));
            let mut system_vad =
                VadChunker::new(Track::System, vad).reporting_to(system_activity.clone());
            let mut mic_vad = VadChunker::new(Track::Mic, vad);
            let mut last_state = Instant::now() - STATE_INTERVAL;
            let mut receiving = false;

            while running.load(Ordering::SeqCst) {
                let mut idle = true;

                // Poll both streams. A short timeout on the first keeps the
                // loop responsive without spinning when everything is silent.
                if let Ok(block) = system_rx.recv_timeout(Duration::from_millis(20)) {
                    idle = false;
                    receiving = true;
                    for u in system_vad.push(&block.samples, block.captured_at) {
                        // The interim line for this track is now stale whatever
                        // happens to the utterance next.
                        on_partial(Track::System, String::new());
                        if !worker.submit(u) {
                            dropped.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                if let Some(rx) = &mic_rx {
                    while let Ok(block) = rx.try_recv() {
                        idle = false;
                        for u in mic_vad.push(&block.samples, block.captured_at) {
                            on_partial(Track::Mic, String::new());
                            if suppress_echo && is_echo(&system_activity, &u) {
                                continue;
                            }
                            if !worker.submit(u) {
                                dropped.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                }

                // Interim text, but never at the cost of a final segment: skip
                // while anything is queued or the previous pass is still
                // running, which also throttles naturally as an utterance grows.
                if want_partials
                    && worker.backlog() == 0
                    && !partials.is_busy()
                    && last_partial.elapsed() >= PARTIAL_INTERVAL
                {
                    last_partial = Instant::now();
                    // The echo test for finished utterances cannot run yet, so
                    // apply its premise directly. Looking back over a window
                    // rather than at this instant also covers the moment after
                    // the system stops, when the microphone is still trailing
                    // the sound it just heard.
                    let now = Instant::now();
                    let mic_pending = mic_vad.pending().filter(|p| {
                        if !suppress_echo {
                            return true;
                        }
                        // Same test the finished utterance will face, applied to
                        // the buffer so far.
                        let (from, to) = core_span(p.started_at, now);
                        !system_activity
                            .coverage(from, to)
                            .is_some_and(|c| c >= ECHO_COVERAGE)
                    });
                    for p in [system_vad.pending(), mic_pending].into_iter().flatten() {
                        partials.offer(p);
                    }
                }

                if last_state.elapsed() >= STATE_INTERVAL {
                    last_state = Instant::now();
                    let backlog = worker.backlog();
                    on_state(LiveState {
                        listening: true,
                        elapsed_ms: session_start.elapsed().as_millis() as u64,
                        backlog,
                        lagging: backlog >= LAG_THRESHOLD,
                        dropped: dropped.load(Ordering::Relaxed),
                        receiving,
                    });
                }

                if idle {
                    thread::sleep(Duration::from_millis(10));
                }
            }

            // Emit whatever speech was still open when the user stopped.
            for u in [system_vad.flush(), mic_vad.flush()].into_iter().flatten() {
                if suppress_echo && u.track == Track::Mic && is_echo(&system_activity, &u) {
                    continue;
                }
                worker.submit(u);
            }

            system.stop();
            mic.stop();
            // Give the queue a moment to drain before the worker is dropped.
            let drain_deadline = Instant::now() + Duration::from_secs(20);
            while worker.backlog() > 0 && Instant::now() < drain_deadline {
                thread::sleep(Duration::from_millis(50));
            }
            worker.stop();
            partials.stop();
            on_partial(Track::System, String::new());
            on_partial(Track::Mic, String::new());

            on_state(LiveState {
                listening: false,
                elapsed_ms: session_start.elapsed().as_millis() as u64,
                backlog: 0,
                lagging: false,
                dropped: dropped.load(Ordering::Relaxed),
                receiving,
            });
        });

        Ok(())
    }
}

/// Whether a microphone utterance is the speakers bleeding in rather than the
/// user speaking.
///
/// Without headphones the microphone hears everything the speakers play, and
/// Whisper turns that muffled copy into text that never gets said. The test is
/// deliberately blunt: only when the system track was speaking through nearly
/// all of the utterance.
fn is_echo(system: &ActivityLog, utterance: &Utterance) -> bool {
    let (from, to) = core_span(utterance.started_at, utterance.ended_at);
    match system.coverage(from, to) {
        Some(c) => c >= ECHO_COVERAGE,
        // No system audio recorded over that span, so nothing to echo.
        None => false,
    }
}

/// The middle of a span, with the padding at each end removed.
fn core_span(from: Instant, to: Instant) -> (Instant, Instant) {
    if to.saturating_duration_since(from) <= ECHO_EDGE_TRIM * 3 {
        return (from, to);
    }
    (from + ECHO_EDGE_TRIM, to - ECHO_EDGE_TRIM)
}

/// Shared handle so Tauri commands can reach the running session.
pub type SharedSession = Arc<Mutex<LiveSession>>;
