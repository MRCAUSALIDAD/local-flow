//! Continuous transcription: turns an endless audio stream into utterances and
//! feeds them to Whisper one at a time.
//!
//! Dictation can buffer everything and transcribe once on key release. A call
//! or a video cannot: the audio never ends, so the stream has to be cut into
//! pieces. Cutting on a fixed sliding window means re-transcribing overlapping
//! audio, which produces duplicated and unstable text, so instead this cuts at
//! the pauses and transcribes each piece exactly once.

use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::dsp::TARGET_SR;

/// Which stream an utterance came from.
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Track {
    /// The computer's own output: the other people on the call, a video.
    System,
    /// The local microphone: the user.
    Mic,
}

/// A stretch of speech bounded by silence, ready to transcribe.
pub struct Utterance {
    pub samples: Vec<f32>,
    pub track: Track,
    pub started_at: Instant,
    pub ended_at: Instant,
    /// True when the previous utterance was cut short by the length limit
    /// rather than by a pause, so its text should prime this one.
    pub continues_previous: bool,
}

impl Utterance {
    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64(self.samples.len() as f64 / TARGET_SR as f64)
    }
}

/// Speech still in progress, transcribed provisionally.
#[derive(Clone)]
pub struct Pending {
    pub samples: Vec<f32>,
    pub track: Track,
    pub voiced_samples: usize,
    /// When this speech began, so callers can test it against another track.
    pub started_at: Instant,
}

/// A transcribed piece of a live session.
#[derive(Serialize, Clone, Debug)]
pub struct LiveSegment {
    pub track: Track,
    pub text: String,
    /// Milliseconds since the session started.
    pub start_ms: u64,
    pub end_ms: u64,
}

// --- voice activity detection ------------------------------------------------

/// 20 ms at 16 kHz.
const FRAME: usize = 320;
/// Speech needed before an interim transcription is worth attempting.
const PARTIAL_MIN_SPEECH: Duration = Duration::from_millis(1_000);
/// Absolute level below which audio counts as silence regardless of the
/// adapted noise floor, so a silent stream never drifts into self-triggering.
const ABS_FLOOR: f32 = 0.002;
const OPEN_MULT: f32 = 4.0;
const CLOSE_MULT: f32 = 2.0;
/// Mean level a stretch of speech must reach before it is worth transcribing.
///
/// The open threshold adapts to the noise floor, so on a very quiet microphone
/// it can trip on bleed from the speakers. Whisper does not return silence for
/// such input: it invents fluent sentences that never happened, which is worse
/// than missing a line. whisper.cpp's own no_speech threshold is not
/// implemented, and whisper-rs exposes no per-segment probability, so an
/// absolute gate is what is left.
const MIN_SPEECH_RMS: f32 = 0.008;

#[derive(Clone, Copy)]
pub struct VadConfig {
    /// Silence needed to close an utterance.
    pub hang: Duration,
    /// Hard cap, keeping utterances inside the 30 s window Whisper was trained
    /// on.
    pub max_len: Duration,
    /// Utterances shorter than this are discarded as clicks or noise.
    pub min_len: Duration,
    /// Audio kept from before speech was detected, so word onsets survive.
    pub pre_roll: Duration,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            hang: Duration::from_millis(600),
            max_len: Duration::from_secs(25),
            min_len: Duration::from_millis(300),
            pre_roll: Duration::from_millis(300),
        }
    }
}

fn samples_for(d: Duration) -> usize {
    (d.as_secs_f64() * TARGET_SR as f64) as usize
}

/// A rolling record of when one track carried speech.
///
/// Used to tell the user talking apart from the speakers bleeding into their
/// microphone: if the system track was playing throughout what the microphone
/// "heard", the microphone heard the speakers.
pub struct ActivityLog {
    /// (frame end, whether it carried speech), oldest first.
    frames: Mutex<VecDeque<(Instant, bool)>>,
    window: Duration,
}

impl ActivityLog {
    pub fn new(window: Duration) -> Self {
        Self {
            frames: Mutex::new(VecDeque::new()),
            window,
        }
    }

    fn record(&self, at: Instant, voiced: bool) {
        let mut f = self.frames.lock().unwrap();
        f.push_back((at, voiced));
        let cutoff = at.checked_sub(self.window).unwrap_or(at);
        while f.front().is_some_and(|(t, _)| *t < cutoff) {
            f.pop_front();
        }
    }

    /// Fraction of the window that carried speech, or `None` if the window is
    /// not covered by recorded frames at all.
    pub fn coverage(&self, from: Instant, to: Instant) -> Option<f32> {
        let f = self.frames.lock().unwrap();
        let in_range: Vec<bool> = f
            .iter()
            .filter(|(t, _)| *t >= from && *t <= to)
            .map(|(_, v)| *v)
            .collect();
        if in_range.is_empty() {
            return None;
        }
        let voiced = in_range.iter().filter(|v| **v).count();
        Some(voiced as f32 / in_range.len() as f32)
    }
}

/// Splits a continuous 16 kHz mono stream into utterances.
pub struct VadChunker {
    cfg: VadConfig,
    track: Track,
    noise_floor: f32,
    in_speech: bool,
    /// Audio accumulated for the utterance being built.
    current: Vec<f32>,
    /// Rolling window of recent silence, prepended when speech starts.
    pre_roll: VecDeque<f32>,
    /// Trailing silent samples already inside `current`.
    trailing_silence: usize,
    /// Samples of actual speech in `current`, excluding pre-roll and tail.
    /// The minimum-length test measures this, not the buffer: a click wrapped
    /// in pre-roll is still a click.
    voiced_samples: usize,
    /// Running sum of frame levels over voiced frames, for the energy gate.
    voiced_rms_sum: f32,
    voiced_frames: usize,
    started_at: Option<Instant>,
    last_block_at: Option<Instant>,
    /// Leftover samples when a block does not divide into whole frames.
    partial: Vec<f32>,
    carry_continuation: bool,
    /// Where this track publishes its speech activity, if anyone is watching.
    activity: Option<Arc<ActivityLog>>,
}

impl VadChunker {
    pub fn new(track: Track, cfg: VadConfig) -> Self {
        Self {
            cfg,
            track,
            noise_floor: ABS_FLOOR,
            in_speech: false,
            current: Vec::new(),
            pre_roll: VecDeque::new(),
            trailing_silence: 0,
            voiced_samples: 0,
            voiced_rms_sum: 0.0,
            voiced_frames: 0,
            started_at: None,
            last_block_at: None,
            partial: Vec::new(),
            carry_continuation: false,
            activity: None,
        }
    }

    /// Publishes this track's speech activity so another track can check it.
    pub fn reporting_to(mut self, log: Arc<ActivityLog>) -> Self {
        self.activity = Some(log);
        self
    }

    /// Feeds one captured block, returning any utterances it completed.
    ///
    /// `captured_at` is the wall-clock instant the block ended.
    pub fn push(&mut self, samples: &[f32], captured_at: Instant) -> Vec<Utterance> {
        let mut done = Vec::new();

        // A loopback stream emits nothing at all while the system is silent, so
        // a gap between blocks is real silence that was never delivered. Left
        // unhandled, an utterance would stay open across a pause of any length.
        if let Some(prev) = self.last_block_at {
            let gap = captured_at.saturating_duration_since(prev);
            let block_len = Duration::from_secs_f64(samples.len() as f64 / TARGET_SR as f64);
            if gap > block_len + self.cfg.hang && self.in_speech {
                if let Some(u) = self.close(prev, false) {
                    done.push(u);
                }
            }
        }
        self.last_block_at = Some(captured_at);

        self.partial.extend_from_slice(samples);
        let total = self.partial.len();
        let frames = total / FRAME;

        // Copied rather than borrowed so the frame does not hold a reference
        // into `self` while `push_frame` mutates it.
        let mut frame = [0f32; FRAME];
        for i in 0..frames {
            frame.copy_from_slice(&self.partial[i * FRAME..(i + 1) * FRAME]);
            // Instant this frame ended, derived from the block's end time.
            let remaining = total - (i + 1) * FRAME;
            let at = captured_at
                .checked_sub(Duration::from_secs_f64(remaining as f64 / TARGET_SR as f64))
                .unwrap_or(captured_at);

            if let Some(u) = self.push_frame(&frame, at) {
                done.push(u);
            }
        }

        self.partial.drain(..frames * FRAME);
        done
    }

    fn push_frame(&mut self, frame: &[f32], at: Instant) -> Option<Utterance> {
        let rms = rms(frame);
        let open = (self.noise_floor * OPEN_MULT).max(ABS_FLOOR * 2.0);
        let close = (self.noise_floor * CLOSE_MULT).max(ABS_FLOOR);

        let voiced = if self.in_speech { rms > close } else { rms > open };

        if let Some(log) = &self.activity {
            log.record(at, voiced);
        }

        if !voiced {
            // Adapt only on quiet frames, so loud speech never raises the floor
            // to the point of masking itself.
            self.noise_floor = self.noise_floor * 0.995 + rms * 0.005;
        }

        if voiced {
            if !self.in_speech {
                self.in_speech = true;
                self.current.clear();
                self.current.extend(self.pre_roll.iter().copied());
                let lead =
                    Duration::from_secs_f64(self.current.len() as f64 / TARGET_SR as f64);
                self.started_at = Some(
                    at.checked_sub(lead + frame_duration()).unwrap_or(at),
                );
                self.pre_roll.clear();
            }
            self.current.extend_from_slice(frame);
            self.trailing_silence = 0;
            self.voiced_samples += frame.len();
            self.voiced_rms_sum += rms;
            self.voiced_frames += 1;

            if self.current.len() >= samples_for(self.cfg.max_len) {
                // Cut mid-speech. The next utterance is marked as a
                // continuation so its transcription can be primed with this
                // one's text.
                return self.close(at, true);
            }
            return None;
        }

        if self.in_speech {
            self.current.extend_from_slice(frame);
            self.trailing_silence += frame.len();
            if self.trailing_silence >= samples_for(self.cfg.hang) {
                return self.close(at, false);
            }
            return None;
        }

        self.remember_silence(frame);
        None
    }

    fn remember_silence(&mut self, frame: &[f32]) {
        let cap = samples_for(self.cfg.pre_roll);
        self.pre_roll.extend(frame.iter().copied());
        while self.pre_roll.len() > cap {
            self.pre_roll.pop_front();
        }
    }

    /// Ends the current utterance. `truncated` marks a cut made because of the
    /// length cap rather than a pause.
    fn close(&mut self, at: Instant, truncated: bool) -> Option<Utterance> {
        self.in_speech = false;
        let mut samples = std::mem::take(&mut self.current);
        let started_at = self.started_at.take().unwrap_or(at);
        let continues_previous = self.carry_continuation;
        self.carry_continuation = truncated;
        let voiced = std::mem::take(&mut self.voiced_samples);
        let rms_sum = std::mem::take(&mut self.voiced_rms_sum);
        let frames = std::mem::take(&mut self.voiced_frames);
        let mean_level = if frames > 0 {
            rms_sum / frames as f32
        } else {
            0.0
        };

        // Drop the trailing silence, keeping a little as a natural tail.
        let keep = samples_for(Duration::from_millis(120));
        if self.trailing_silence > keep {
            let cut = samples.len() - (self.trailing_silence - keep);
            samples.truncate(cut);
        }
        self.trailing_silence = 0;

        if truncated {
            // Speech is still going, so reopen immediately.
            self.in_speech = true;
            self.started_at = Some(at);
        }

        if voiced < samples_for(self.cfg.min_len) || mean_level < MIN_SPEECH_RMS {
            return None;
        }

        Some(Utterance {
            samples,
            track: self.track,
            started_at,
            ended_at: at,
            continues_previous,
        })
    }

    /// Whether this track is mid-utterance right now.
    pub fn is_speaking(&self) -> bool {
        self.in_speech
    }

    /// A copy of the speech currently being collected, if it is worth
    /// transcribing provisionally.
    ///
    /// Final segments only appear once the speaker pauses, which on a long
    /// sentence is many seconds of nothing on screen. This feeds the interim
    /// text shown in the meantime.
    pub fn pending(&self) -> Option<Pending> {
        if !self.in_speech || self.voiced_samples < samples_for(PARTIAL_MIN_SPEECH) {
            return None;
        }
        // The same energy gate a finished utterance faces. Without it, room
        // noise loud enough to open the adaptive threshold gets sent to
        // Whisper, which answers with invented sentences.
        let mean_level = if self.voiced_frames > 0 {
            self.voiced_rms_sum / self.voiced_frames as f32
        } else {
            0.0
        };
        if mean_level < MIN_SPEECH_RMS {
            return None;
        }
        Some(Pending {
            samples: self.current.clone(),
            track: self.track,
            voiced_samples: self.voiced_samples,
            started_at: self.started_at?,
        })
    }

    /// Emits whatever is still buffered. Call when capture stops.
    pub fn flush(&mut self) -> Option<Utterance> {
        if !self.in_speech {
            return None;
        }
        let at = self.last_block_at.unwrap_or_else(Instant::now);
        self.close(at, false)
    }
}

fn frame_duration() -> Duration {
    Duration::from_secs_f64(FRAME as f64 / TARGET_SR as f64)
}

fn rms(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    let sum: f64 = frame.iter().map(|s| (*s as f64) * (*s as f64)).sum();
    (sum / frame.len() as f64).sqrt() as f32
}

// --- transcription worker ----------------------------------------------------

/// Bounded so a backlog is visible and capped rather than unbounded. Utterances
/// are seconds of audio each, so this is already a long queue.
const QUEUE_CAPACITY: usize = 32;

/// Serialises Whisper inference for a live session.
///
/// One worker, one utterance at a time. Running several inferences at once
/// makes them contend for the same cores and would also slow down push-to-talk
/// dictation, which the user is actively waiting on.
pub struct TranscriptionWorker {
    tx: SyncSender<Utterance>,
    depth: Arc<AtomicUsize>,
    running: Arc<AtomicBool>,
}

impl TranscriptionWorker {
    /// `emit` is called on the worker thread for every transcribed segment.
    pub fn spawn<F>(
        transcribe: Box<dyn Fn(&Utterance, Option<&str>) -> Result<String, String> + Send>,
        session_start: Instant,
        emit: F,
    ) -> Self
    where
        F: Fn(LiveSegment) + Send + 'static,
    {
        let (tx, rx): (SyncSender<Utterance>, Receiver<Utterance>) =
            sync_channel(QUEUE_CAPACITY);
        let depth = Arc::new(AtomicUsize::new(0));
        let running = Arc::new(AtomicBool::new(true));

        let worker_depth = depth.clone();
        let worker_running = running.clone();
        thread::spawn(move || {
            // Tail of the previous text, used to prime a continuation.
            let mut carry: Option<String> = None;

            while let Ok(utterance) = rx.recv() {
                worker_depth.fetch_sub(1, Ordering::SeqCst);
                if !worker_running.load(Ordering::SeqCst) {
                    continue;
                }

                let prompt = if utterance.continues_previous {
                    carry.as_deref()
                } else {
                    None
                };

                match transcribe(&utterance, prompt) {
                    Ok(text) if text.trim().is_empty() => {}
                    Ok(text) if is_non_speech(&text) => {}
                    Ok(text) => {
                        let text = strip_annotations(&text);
                        if text.is_empty() {
                            continue;
                        }
                        carry = Some(tail_words(&text, 32));
                        emit(LiveSegment {
                            track: utterance.track,
                            text: text.trim().to_string(),
                            start_ms: millis_since(session_start, utterance.started_at),
                            end_ms: millis_since(session_start, utterance.ended_at),
                        });
                    }
                    Err(e) => eprintln!("[stream] transcription failed: {e}"),
                }
            }
        });

        Self { tx, depth, running }
    }

    /// Queues an utterance. Returns false if the backlog is full, in which case
    /// the audio is dropped: falling further behind on a live stream helps
    /// nobody, and the UI surfaces the condition instead.
    pub fn submit(&self, utterance: Utterance) -> bool {
        match self.tx.try_send(utterance) {
            Ok(()) => {
                self.depth.fetch_add(1, Ordering::SeqCst);
                true
            }
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => false,
        }
    }

    /// How many utterances are waiting. Anything sustained above a couple of
    /// items means transcription is not keeping up with real time.
    pub fn backlog(&self) -> usize {
        self.depth.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

/// Transcribes interim text for speech that has not finished yet.
///
/// Kept apart from the main worker so provisional work can never delay a final
/// segment. It holds at most one job per track and always discards the older
/// one: interim text that has been superseded is worthless.
pub struct PartialWorker {
    slot: Arc<(Mutex<Vec<Pending>>, Condvar)>,
    busy: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
}

impl PartialWorker {
    pub fn spawn<F>(
        transcribe: Box<dyn Fn(&Pending) -> Result<String, String> + Send>,
        emit: F,
    ) -> Self
    where
        F: Fn(Track, String) + Send + 'static,
    {
        let slot = Arc::new((Mutex::new(Vec::<Pending>::new()), Condvar::new()));
        let busy = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(true));

        let worker_slot = slot.clone();
        let worker_busy = busy.clone();
        let worker_running = running.clone();
        thread::spawn(move || {
            let (lock, cv) = &*worker_slot;
            let mut last: HashMap<Track, String> = HashMap::new();
            loop {
                let job = {
                    let mut queue = lock.lock().unwrap();
                    while queue.is_empty() && worker_running.load(Ordering::SeqCst) {
                        queue = cv.wait(queue).unwrap();
                    }
                    if !worker_running.load(Ordering::SeqCst) {
                        return;
                    }
                    queue.remove(0)
                };

                worker_busy.store(true, Ordering::SeqCst);
                if let Ok(text) = transcribe(&job) {
                    let text = strip_annotations(&text);
                    // Re-emitting identical text only churns the UI, and it is
                    // common: a speaker pausing mid-sentence yields the same
                    // transcription pass after pass.
                    let unchanged = last.get(&job.track).is_some_and(|p| *p == text);
                    if !text.is_empty() && !is_non_speech(&text) && !unchanged {
                        last.insert(job.track, text.clone());
                        emit(job.track, text);
                    }
                }
                worker_busy.store(false, Ordering::SeqCst);
            }
        });

        Self { slot, busy, running }
    }

    /// Queues interim work, replacing anything still waiting for that track.
    pub fn offer(&self, pending: Pending) {
        let (lock, cv) = &*self.slot;
        let mut queue = lock.lock().unwrap();
        queue.retain(|p| p.track != pending.track);
        queue.push(pending);
        cv.notify_one();
    }

    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        self.slot.1.notify_all();
    }
}

fn millis_since(base: Instant, at: Instant) -> u64 {
    at.saturating_duration_since(base).as_millis() as u64
}

/// Whether a transcription is one of Whisper's non-speech artefacts rather
/// than actual words.
///
/// Fed near-silence, Whisper reliably invents sound annotations like
/// "[Música]" or credits scraped from its subtitle training data. A live
/// session hits this constantly: the microphone track is mostly quiet, and the
/// far side pauses between sentences.
fn is_non_speech(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return true;
    }

    // Wholly enclosed in brackets or music notes: an annotation, not speech.
    let enclosed = (t.starts_with('[') && t.ends_with(']'))
        || (t.starts_with('(') && t.ends_with(')'))
        || (t.starts_with('*') && t.ends_with('*'));
    if enclosed && !t[1..t.len() - 1].contains(['[', '(']) {
        return true;
    }
    if t.chars().all(|c| "♪♫*-. ".contains(c)) {
        return true;
    }

    let lower = t.to_lowercase();
    let lower = lower.trim_matches(|c: char| !c.is_alphanumeric());
    const ARTEFACTS: [&str; 8] = [
        "subtítulos realizados por la comunidad de amara.org",
        "subtitulado por la comunidad de amara.org",
        "más información en www.amara.org",
        "thanks for watching",
        "thank you for watching",
        "gracias por ver el video",
        "subscribe to my channel",
        "www.amara.org",
    ];
    ARTEFACTS.iter().any(|a| lower == *a)
}

/// Removes sound annotations Whisper attaches to the ends of real speech,
/// such as a leading "(Música)".
///
/// Only the ends are touched. Brackets in the middle of a sentence are far
/// more likely to be something the speaker actually said.
fn strip_annotations(text: &str) -> String {
    let mut t = text.trim();
    loop {
        let trimmed = match t.chars().next() {
            Some('[') => t.find(']').map(|i| t[i + 1..].trim_start()),
            Some('(') => t.find(')').map(|i| t[i + 1..].trim_start()),
            _ => None,
        };
        match trimmed {
            Some(rest) if rest != t => t = rest,
            _ => break,
        }
    }
    loop {
        let trimmed = match t.chars().last() {
            Some(']') => t.rfind('[').map(|i| t[..i].trim_end()),
            Some(')') => t.rfind('(').map(|i| t[..i].trim_end()),
            _ => None,
        };
        match trimmed {
            Some(rest) if rest != t => t = rest,
            _ => break,
        }
    }
    t.to_string()
}

/// Last `n` words, used to prime a continuation without feeding Whisper a
/// prompt longer than it will read.
fn tail_words(text: &str, n: usize) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let start = words.len().saturating_sub(n);
    words[start..].join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize, amp: f32) -> Vec<f32> {
        (0..n)
            .map(|i| amp * (i as f32 * 0.15).sin())
            .collect()
    }

    fn silence(n: usize) -> Vec<f32> {
        vec![0.0; n]
    }

    fn feed(chunker: &mut VadChunker, blocks: &[Vec<f32>]) -> Vec<Utterance> {
        let mut out = Vec::new();
        let mut at = Instant::now();
        for b in blocks {
            at += Duration::from_secs_f64(b.len() as f64 / TARGET_SR as f64);
            out.extend(chunker.push(b, at));
        }
        out
    }

    #[test]
    fn silence_produces_nothing() {
        let mut c = VadChunker::new(Track::System, VadConfig::default());
        let out = feed(&mut c, &[silence(16_000), silence(16_000)]);
        assert!(out.is_empty());
        assert!(c.flush().is_none());
    }

    #[test]
    fn one_burst_becomes_one_utterance() {
        let mut c = VadChunker::new(Track::System, VadConfig::default());
        let out = feed(
            &mut c,
            &[
                silence(8_000),
                tone(16_000, 0.3), // 1 s of speech
                silence(16_000),   // 1 s of silence, past the 600 ms hang
            ],
        );
        assert_eq!(out.len(), 1, "expected exactly one utterance");
        assert_eq!(out[0].track, Track::System);
        assert!(
            out[0].duration() >= Duration::from_millis(900),
            "utterance too short: {:?}",
            out[0].duration()
        );
    }

    #[test]
    fn two_bursts_separated_by_a_pause_become_two_utterances() {
        let mut c = VadChunker::new(Track::System, VadConfig::default());
        let out = feed(
            &mut c,
            &[
                tone(16_000, 0.3),
                silence(16_000),
                tone(16_000, 0.3),
                silence(16_000),
            ],
        );
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn short_blips_are_discarded() {
        let mut c = VadChunker::new(Track::System, VadConfig::default());
        let out = feed(&mut c, &[silence(8_000), tone(640, 0.3), silence(16_000)]);
        assert!(out.is_empty(), "a 40 ms blip should not become an utterance");
    }

    #[test]
    fn long_speech_is_cut_at_the_limit_and_marked_as_continuing() {
        let cfg = VadConfig {
            max_len: Duration::from_secs(2),
            ..VadConfig::default()
        };
        let mut c = VadChunker::new(Track::Mic, cfg);
        // 5 s of unbroken speech against a 2 s cap.
        let out = feed(&mut c, &[tone(80_000, 0.3)]);
        assert!(out.len() >= 2, "expected several cuts, got {}", out.len());
        assert!(!out[0].continues_previous);
        assert!(
            out[1].continues_previous,
            "a cut made by the length cap must mark the next piece as continuing"
        );
        for u in &out {
            assert!(u.duration() <= Duration::from_millis(2_100));
        }
    }

    #[test]
    fn a_gap_in_delivery_closes_the_utterance() {
        // Loopback delivers no callbacks during silence, so a jump in the
        // wall clock has to be read as a pause.
        let mut c = VadChunker::new(Track::System, VadConfig::default());
        let mut at = Instant::now();
        at += Duration::from_secs(1);
        c.push(&tone(16_000, 0.3), at);
        // Next block arrives 5 s later with no silence in between.
        at += Duration::from_secs(5);
        let out = c.push(&tone(16_000, 0.3), at);
        assert_eq!(out.len(), 1, "the gap should have closed the first burst");
    }

    #[test]
    fn flush_emits_speech_still_in_progress() {
        let mut c = VadChunker::new(Track::System, VadConfig::default());
        let out = feed(&mut c, &[tone(16_000, 0.3)]);
        assert!(out.is_empty(), "no pause yet, so nothing is complete");
        let tail = c.flush().expect("flush should emit the open utterance");
        assert!(tail.duration() >= Duration::from_millis(900));
    }

    #[test]
    fn very_quiet_audio_is_not_transcribed() {
        // Speaker bleed into the microphone trips the adaptive threshold but
        // stays far below real speech. Whisper answers such input with
        // invented sentences, so it never gets that far.
        let mut c = VadChunker::new(Track::Mic, VadConfig::default());
        let out = feed(
            &mut c,
            &[silence(16_000), tone(16_000, 0.004), silence(16_000)],
        );
        assert!(out.is_empty(), "bleed-level audio should not be transcribed");
    }

    #[test]
    fn quiet_speech_over_a_quiet_floor_is_still_detected() {
        let mut c = VadChunker::new(Track::System, VadConfig::default());
        let out = feed(
            &mut c,
            &[silence(16_000), tone(16_000, 0.02), silence(16_000)],
        );
        assert_eq!(out.len(), 1, "a quiet talker should not be missed");
    }

    #[test]
    fn non_speech_annotations_are_rejected() {
        assert!(is_non_speech("[Música]"));
        assert!(is_non_speech("  (applause) "));
        assert!(is_non_speech("[BLANK_AUDIO]"));
        assert!(is_non_speech("♪♪♪"));
        assert!(is_non_speech(""));
        assert!(is_non_speech(
            "Subtítulos realizados por la comunidad de Amara.org"
        ));
    }

    #[test]
    fn real_speech_survives_the_filter() {
        assert!(!is_non_speech("Hola, soy la otra persona en la videollamada."));
        assert!(!is_non_speech("Thanks for watching the demo, it worked."));
        assert!(!is_non_speech("(laughs) but seriously, the deadline is Friday"));
        assert!(!is_non_speech("El segundo fragmento."));
    }

    #[test]
    fn annotations_are_stripped_from_the_ends_only() {
        assert_eq!(strip_annotations("(Música) Esta es una frase."), "Esta es una frase.");
        assert_eq!(strip_annotations("Esta es una frase. [Música]"), "Esta es una frase.");
        assert_eq!(strip_annotations("[Música] Hola [Música]"), "Hola");
        // Mid-sentence brackets are likely real speech, so they survive.
        assert_eq!(
            strip_annotations("dijo (y esto es clave) que no"),
            "dijo (y esto es clave) que no"
        );
    }

    #[test]
    fn tail_words_keeps_the_end() {
        assert_eq!(tail_words("a b c d e", 2), "d e");
        assert_eq!(tail_words("short", 10), "short");
    }
}
