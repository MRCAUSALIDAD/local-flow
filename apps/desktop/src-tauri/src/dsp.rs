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

/// A single biquad section, used to build the anti-aliasing filter.
#[derive(Clone, Copy)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    /// Low-pass section at `cutoff` Hz for the given sample rate and Q.
    fn low_pass(cutoff: f32, sample_rate: f32, q: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * cutoff / sample_rate;
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: ((1.0 - cos_w0) / 2.0) / a0,
            b1: (1.0 - cos_w0) / a0,
            b2: ((1.0 - cos_w0) / 2.0) / a0,
            a1: (-2.0 * cos_w0) / a0,
            a2: (1.0 - alpha) / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// Cutoff of the anti-aliasing filter, comfortably under the 8 kHz Nyquist
/// limit of 16 kHz audio while leaving the speech band intact.
const ANTI_ALIAS_HZ: f32 = 7_000.0;
/// Q values for a maximally flat eighth-order Butterworth response.
///
/// Eighth order rather than fourth because the transition band is narrow: the
/// cutoff sits at 7 kHz and content only half an octave above it already folds
/// back into the speech band.
const BUTTERWORTH_Q: [f32; 4] = [0.509_796, 0.601_345, 0.899_976, 2.562_915];

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
    /// Anti-aliasing filter, present only when downsampling.
    ///
    /// Dropping samples without removing what sits above the new Nyquist limit
    /// folds that content back into the audible band at full amplitude. Speech
    /// alone barely notices, but system audio carries music and effects with
    /// real high-frequency energy, which would land straight on top of the
    /// voices being transcribed.
    lpf: Option<[Biquad; 4]>,
}

impl StreamResampler {
    pub fn new(source_sr: u32) -> Self {
        let sr = source_sr.max(1);
        let lpf = (sr > TARGET_SR).then(|| {
            BUTTERWORTH_Q.map(|q| Biquad::low_pass(ANTI_ALIAS_HZ, sr as f32, q))
        });
        Self {
            step: sr as f64 / TARGET_SR as f64,
            phase: 0.0,
            carry: Vec::new(),
            lpf,
        }
    }

    /// Feeds mono samples at the source rate, returning whatever 16 kHz samples
    /// are now complete. May return an empty slice for small inputs.
    pub fn push(&mut self, mono: &[f32]) -> Vec<f32> {
        match &mut self.lpf {
            Some(sections) => self.carry.extend(mono.iter().map(|s| {
                sections.iter_mut().fold(*s, |acc, b| b.process(acc))
            })),
            None => self.carry.extend_from_slice(mono),
        }
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

#[cfg(test)]
mod alias_tests {
    use super::*;

    /// Energy at a frequency, via a Goertzel-style correlation.
    fn energy_at(x: &[f32], freq: f32, sr: f32) -> f32 {
        let (mut re, mut im) = (0f64, 0f64);
        for (n, s) in x.iter().enumerate() {
            let w = 2.0 * std::f64::consts::PI * freq as f64 * n as f64 / sr as f64;
            re += *s as f64 * w.cos();
            im += *s as f64 * w.sin();
        }
        ((re * re + im * im).sqrt() / x.len() as f64) as f32
    }

    #[test]
    fn measure_aliasing_of_a_10khz_tone() {
        // 10 kHz sits above the 8 kHz Nyquist limit of 16 kHz audio. Without a
        // low-pass before decimation it folds down to |16000 - 10000| = 6 kHz.
        let sr = 48_000.0;
        let tone: Vec<f32> = (0..48_000)
            .map(|i| (2.0 * std::f32::consts::PI * 10_000.0 * i as f32 / sr).sin())
            .collect();

        let mut r = StreamResampler::new(48_000);
        let mut out = Vec::new();
        for b in tone.chunks(512) {
            out.extend(r.push(b));
        }

        let ghost = energy_at(&out, 6_000.0, 16_000.0);
        let orig = energy_at(&tone, 10_000.0, 48_000.0);
        let ratio = ghost / orig;
        println!("original 10kHz energy: {orig:.4}");
        println!("aliased  6kHz energy:  {ghost:.4}");
        println!("ratio: {:.1}%  ({:.1} dB)", 100.0 * ratio, 20.0 * ratio.log10());
        assert!(
            ratio < 0.05,
            "anti-alias filter too weak: {:.1}% of the tone folded back",
            100.0 * ratio
        );
    }
}
