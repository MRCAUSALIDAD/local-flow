use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub model_path: Option<String>,
    #[serde(default)]
    pub input_device: Option<String>,
    pub language: String,
    #[serde(default = "default_true", alias = "auto_paste")]
    pub auto_type: bool,
    #[serde(default = "default_true")]
    pub copy_to_clipboard: bool,
    #[serde(default)]
    pub show_metrics: bool,
    #[serde(default = "default_corner")]
    pub metrics_corner: String,
}

fn default_true() -> bool {
    true
}

fn default_corner() -> String {
    "top-right".into()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model_path: None,
            input_device: None,
            language: "auto".into(),
            auto_type: true,
            copy_to_clipboard: true,
            show_metrics: false,
            metrics_corner: default_corner(),
        }
    }
}

fn config_file(dir: &Path) -> PathBuf {
    dir.join("config.json")
}

pub fn load(dir: &Path) -> Config {
    fs::read_to_string(config_file(dir))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(dir: &Path, cfg: &Config) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let json = serde_json::to_string_pretty(cfg).unwrap_or_default();
    fs::write(config_file(dir), json)
}
