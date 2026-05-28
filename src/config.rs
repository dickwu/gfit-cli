//! Persistent CLI config stored at `~/.config/gfit.json`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_API_URL: &str = "https://api.gfitwellness.ca";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,
}

/// Resolve the config file path. `GFIT_CONFIG` overrides; otherwise `~/.config/gfit.json`.
pub fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("GFIT_CONFIG") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config").join("gfit.json")
}

pub fn load() -> Config {
    match std::fs::read_to_string(config_path()) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

pub fn save(cfg: &Config) -> std::io::Result<()> {
    let path = config_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(&path, json + "\n")
}

/// Effective API base URL: `GFIT_API_URL` env > config file > built-in default.
pub fn api_url(cfg: &Config) -> String {
    if let Ok(u) = std::env::var("GFIT_API_URL") {
        return u;
    }
    cfg.api_url
        .clone()
        .unwrap_or_else(|| DEFAULT_API_URL.to_string())
}
