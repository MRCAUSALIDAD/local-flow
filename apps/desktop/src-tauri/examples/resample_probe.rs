//! Dev tool: isolates whether the capture-path resampler degrades transcription.
//!
//!     cargo run --release --example resample_probe -- s48.wav s16.wav
//!
//! s48 is the source at its native rate, s16 the same audio resampled by a
//! known-good tool. Path A pushes s48 through StreamResampler in cpal-sized
//! blocks; path B uses s16 directly. Same model, same options, so any quality
//! gap is the resampler's.

use local_flow_lib::dsp::StreamResampler;
use local_flow_lib::whisper::{self, Options, Transcriber};

fn read_wav_mono_f32(path: &str) -> (Vec<f32>, u32) {
    let bytes = std::fs::read(path).expect("read wav");
    // Minimal RIFF parse: locate fmt and data chunks.
    let mut pos = 12;
    let mut rate = 0u32;
    let mut channels = 1u16;
    let mut bits = 16u16;
    let mut data: &[u8] = &[];
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let body = &bytes[pos + 8..(pos + 8 + size).min(bytes.len())];
        if id == b"fmt " {
            channels = u16::from_le_bytes(body[2..4].try_into().unwrap());
            rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
            bits = u16::from_le_bytes(body[14..16].try_into().unwrap());
        } else if id == b"data" {
            data = body;
        }
        pos += 8 + size + (size & 1);
    }
    assert_eq!(bits, 16, "expected 16-bit pcm");
    let samples: Vec<f32> = data
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
        .collect();
    let mono: Vec<f32> = if channels > 1 {
        samples
            .chunks(channels as usize)
            .map(|f| f.iter().sum::<f32>() / f.len() as f32)
            .collect()
    } else {
        samples
    };
    (mono, rate)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (native, native_sr) = read_wav_mono_f32(&args[0]);
    let (reference, ref_sr) = read_wav_mono_f32(&args[1]);
    println!("source:    {} samples at {native_sr} Hz", native.len());
    println!("reference: {} samples at {ref_sr} Hz\n", reference.len());

    // Path A: our streaming resampler, fed in the block size cpal delivers.
    let mut r = StreamResampler::new(native_sr);
    let mut ours = Vec::new();
    for block in native.chunks(512) {
        ours.extend(r.push(block));
    }
    println!("ours:      {} samples", ours.len());

    let dir = dirs::data_dir().unwrap().join("com.gabriel.local-flow");
    let cfg: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("config.json")).unwrap()).unwrap();
    let model = cfg["model_path"].as_str().unwrap();
    let t = Transcriber::new(model).unwrap();

    let opts = Options {
        language: &args.get(2).cloned().unwrap_or_else(|| "es".into()),
        initial_prompt: None,
        n_threads: Some(whisper::live_threads()),
        no_context: true,
    };

    println!("\n=== A: StreamResampler (capture path) ===");
    println!("{}", whisper::transcribe_with(t.ctx.clone(), &ours, &opts).unwrap());

    println!("\n=== B: afconvert reference ===");
    println!("{}", whisper::transcribe_with(t.ctx.clone(), &reference, &opts).unwrap());
}
