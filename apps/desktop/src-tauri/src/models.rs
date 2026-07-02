use serde::Serialize;
use std::io::Read;
use std::path::PathBuf;

pub struct ModelSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub filename: &'static str,
    pub url: &'static str,
    pub size_mb: u32,
    pub note: &'static str,
}

pub fn catalog() -> Vec<ModelSpec> {
    vec![
        ModelSpec {
            id: "tiny",
            label: "Tiny",
            filename: "ggml-tiny.bin",
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
            size_mb: 75,
            note: "Fastest · lowest accuracy",
        },
        ModelSpec {
            id: "base",
            label: "Base",
            filename: "ggml-base.bin",
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
            size_mb: 142,
            note: "Recommended · balanced",
        },
        ModelSpec {
            id: "small",
            label: "Small",
            filename: "ggml-small.bin",
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
            size_mb: 466,
            note: "Slower · best accuracy",
        },
    ]
}

pub fn spec(id: &str) -> Option<ModelSpec> {
    catalog().into_iter().find(|m| m.id == id)
}

pub fn models_dir(app_data: &PathBuf) -> PathBuf {
    app_data.join("models")
}

#[derive(Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
    pub note: String,
    pub size_mb: u32,
    pub downloaded: bool,
    pub active: bool,
}

pub fn list(app_data: &PathBuf, active_path: Option<&str>) -> Vec<ModelInfo> {
    let dir = models_dir(app_data);
    catalog()
        .into_iter()
        .map(|m| {
            let path = dir.join(m.filename);
            let downloaded = path.exists();
            let active = active_path
                .map(|p| p == path.to_string_lossy())
                .unwrap_or(false);
            ModelInfo {
                id: m.id.into(),
                label: m.label.into(),
                note: m.note.into(),
                size_mb: m.size_mb,
                downloaded,
                active,
            }
        })
        .collect()
}

pub fn download(
    app_data: &PathBuf,
    id: &str,
    mut on_progress: impl FnMut(u64, u64),
) -> anyhow::Result<PathBuf> {
    let spec = spec(id).ok_or_else(|| anyhow::anyhow!("unknown model: {id}"))?;
    let dir = models_dir(app_data);
    std::fs::create_dir_all(&dir)?;
    let final_path = dir.join(spec.filename);
    if final_path.exists() {
        return Ok(final_path);
    }
    let tmp_path = dir.join(format!("{}.part", spec.filename));

    let mut resp = reqwest::blocking::Client::builder()
        .build()?
        .get(spec.url)
        .send()?
        .error_for_status()?;

    let total = resp.content_length().unwrap_or(0);
    let mut file = std::fs::File::create(&tmp_path)?;
    let mut buf = [0u8; 64 * 1024];
    let mut received: u64 = 0;
    loop {
        let n = resp.read(&mut buf)?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n])?;
        received += n as u64;
        on_progress(received, total);
    }
    drop(file);
    std::fs::rename(&tmp_path, &final_path)?;
    Ok(final_path)
}
