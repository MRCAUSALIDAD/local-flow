//! Sample-rate and channel conversion shared by the microphone and system
//! audio capture paths.

pub const TARGET_SR: u32 = 16_000;

/// Downmixes interleaved frames to mono and appends them to `out`.
pub fn downmix_into<T: Copy>(
    data: &[T],
    channels: usize,
    conv: impl Fn(T) -> f32,
    out: &mut Vec<f32>,
) {
    let chans = channels.max(1);
    for frame in data.chunks(chans) {
        let sum: f32 = frame.iter().map(|&s| conv(s)).sum();
        out.push(sum / frame.len() as f32);
    }
}

/// One-shot linear resample of a complete recording to 16 kHz.
///
/// Used by push-to-talk dictation, where the whole buffer is available at once.
/// For continuous capture use [`StreamResampler`] instead.
pub fn resample_to_16k(input: &[f32], sr: u32) -> Vec<f32> {
    if input.is_empty() {
        return Vec::new();
    }
    if sr == TARGET_SR {
        return input.to_vec();
    }
    let ratio = TARGET_SR as f64 / sr as f64;
    let out_len = (input.len() as f64 * ratio) as usize;
    let last = input.len() - 1;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let idx = src as usize;
        let frac = src - idx as f64;
        let a = input[idx.min(last)] as f64;
        let b = input[(idx + 1).min(last)] as f64;
        out.push((a * (1.0 - frac) + b * frac) as f32);
    }
    out
}

/// Continuous linear resampler to 16 kHz.
///
/// Unlike [`resample_to_16k`], this carries the fractional read position and the
/// unconsumed input tail across calls. Resampling each callback buffer
/// independently would restart the phase every block and leave an audible seam
/// at every boundary.
pub struct StreamResampler {
    step: f64,
    phase: f64,
    carry: Vec<f32>,
}

impl StreamResampler {
    pub fn new(source_sr: u32) -> Self {
        let sr = source_sr.max(1);
        Self {
            step: sr as f64 / TARGET_SR as f64,
            phase: 0.0,
            carry: Vec::new(),
        }
    }

    /// Feeds mono samples at the source rate, returning whatever 16 kHz samples
    /// are now complete. May return an empty slice for small inputs.
    pub fn push(&mut self, mono: &[f32]) -> Vec<f32> {
        self.carry.extend_from_slice(mono);
        if self.carry.len() < 2 {
            return Vec::new();
        }

        let mut out = Vec::with_capacity((mono.len() as f64 / self.step) as usize + 1);
        // Stop one sample short: the last sample has no successor to interpolate
        // towards yet, so it stays in the carry for the next call.
        let limit = (self.carry.len() - 1) as f64;
        while self.phase < limit {
            let idx = self.phase as usize;
            let frac = self.phase - idx as f64;
            let a = self.carry[idx] as f64;
            let b = self.carry[idx + 1] as f64;
            out.push((a * (1.0 - frac) + b * frac) as f32);
            self.phase += self.step;
        }

        // The phase can land past the end of the carry when the block length is
        // not a multiple of the step, so clamp before draining. Any overshoot
        // stays in `phase` and correctly skips that many samples of the next
        // block.
        let consumed = (self.phase as usize).min(self.carry.len());
        if consumed > 0 {
            self.carry.drain(..consumed);
            self.phase -= consumed as f64;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_resampler_matches_expected_length() {
        let mut r = StreamResampler::new(48_000);
        let block = vec![0.0f32; 4_800]; // 100 ms at 48 kHz
        let mut total = 0;
        for _ in 0..10 {
            total += r.push(&block).len();
        }
        // 1 second in, expect ~16000 samples out, allowing for the tail held back.
        assert!(
            (15_990..=16_000).contains(&total),
            "got {total} samples, expected ~16000"
        );
    }

    #[test]
    fn stream_resampler_is_continuous_across_blocks() {
        // A ramp fed in two halves must not jump at the block boundary.
        let ramp: Vec<f32> = (0..960).map(|i| i as f32 / 960.0).collect();
        let mut r = StreamResampler::new(48_000);
        let mut out = r.push(&ramp[..480]);
        out.extend(r.push(&ramp[480..]));
        for w in out.windows(2) {
            let d = (w[1] - w[0]).abs();
            assert!(d < 0.02, "discontinuity of {d} in resampled ramp");
        }
    }

    #[test]
    fn survives_block_sizes_that_do_not_divide_the_step() {
        // 48 kHz -> 16 kHz is a step of 3.0, and cpal hands over 512-sample
        // blocks, which is not a multiple of it. This panicked before the
        // carry drain was clamped.
        let mut r = StreamResampler::new(48_000);
        let block = vec![0.1f32; 512];
        let mut total = 0;
        for _ in 0..100 {
            total += r.push(&block).len();
        }
        let expected = 512 * 100 / 3;
        assert!(
            (total as i64 - expected as i64).abs() <= 2,
            "got {total}, expected about {expected}"
        );
    }

    #[test]
    fn odd_rate_ratio_does_not_panic() {
        let mut r = StreamResampler::new(44_100);
        for n in [128usize, 512, 480, 1024, 333] {
            r.push(&vec![0.2f32; n]);
        }
    }

    #[test]
    fn passthrough_when_already_at_target_rate() {
        let mut r = StreamResampler::new(TARGET_SR);
        let out = r.push(&[0.0, 0.25, 0.5, 0.75]);
        assert_eq!(out.len(), 3, "expected all but the held-back tail");
    }

    #[test]
    fn downmix_averages_channels() {
        let mut out = Vec::new();
        downmix_into(&[1.0f32, 0.0, 0.5, 0.5], 2, |s| s, &mut out);
        assert_eq!(out, vec![0.5, 0.5]);
    }
}
