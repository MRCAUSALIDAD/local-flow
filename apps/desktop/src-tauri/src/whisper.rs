use anyhow::Result;
use std::sync::{Arc, Mutex};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Serialises every inference in the process.
///
/// Live transcription and push-to-talk dictation share the same cores. Letting
/// them run at once makes both slower, and dictation is the one the user is
/// actively waiting on.
static INFERENCE: Mutex<()> = Mutex::new(());

pub struct Transcriber {
    pub ctx: Arc<WhisperContext>,
}

impl Transcriber {
    pub fn new(model_path: &str) -> Result<Self> {
        let ctx = WhisperContext::new_with_params(
            model_path,
            WhisperContextParameters::default(),
        )?;
        Ok(Self { ctx: Arc::new(ctx) })
    }
}

#[derive(Clone, Debug)]
pub struct Options<'a> {
    pub language: &'a str,
    /// Text from the previous utterance, used to keep a cut sentence coherent.
    pub initial_prompt: Option<&'a str>,
    /// Leave `None` to use whisper.cpp's own default.
    pub n_threads: Option<i32>,
    /// Transcribe this audio in isolation.
    ///
    /// Wanted for live chunks, which are independent utterances, so one bad
    /// chunk cannot poison the ones after it. Dictation leaves this off, where
    /// carrying context across a long recording's internal windows helps.
    pub no_context: bool,
}

impl<'a> Options<'a> {
    pub fn new(language: &'a str) -> Self {
        Self {
            language,
            initial_prompt: None,
            n_threads: None,
            no_context: false,
        }
    }
}

/// Threads to give a live transcription.
///
/// Half the cores, so capture, the UI and any concurrent dictation still have
/// somewhere to run.
pub fn live_threads() -> i32 {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    (cores / 2).clamp(1, 8) as i32
}

pub fn transcribe(ctx: Arc<WhisperContext>, audio: &[f32], language: &str) -> Result<String> {
    transcribe_with(ctx, audio, &Options::new(language))
}

pub fn transcribe_with(
    ctx: Arc<WhisperContext>,
    audio: &[f32],
    opts: &Options,
) -> Result<String> {
    let _guard = INFERENCE.lock().unwrap_or_else(|e| e.into_inner());

    let mut state = ctx.create_state()?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    if opts.language != "auto" {
        params.set_language(Some(opts.language));
    }
    if let Some(n) = opts.n_threads {
        params.set_n_threads(n);
    }
    if let Some(prompt) = opts.initial_prompt {
        if !prompt.is_empty() {
            params.set_initial_prompt(prompt);
        }
    }
    params.set_no_context(opts.no_context);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    state.full(params, audio)?;

    let segments = state.full_n_segments()?;
    let mut text = String::new();
    for i in 0..segments {
        text.push_str(&state.full_get_segment_text(i)?);
    }
    Ok(text.trim().to_string())
}
