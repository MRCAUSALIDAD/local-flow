use anyhow::Result;
use std::sync::Arc;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

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

pub fn transcribe(ctx: Arc<WhisperContext>, audio: &[f32], language: &str) -> Result<String> {
    let mut state = ctx.create_state()?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    if language != "auto" {
        params.set_language(Some(language));
    }
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
