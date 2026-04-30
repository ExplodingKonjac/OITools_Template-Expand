use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize, Default)]
pub struct TexpandConfig {
    #[serde(default)]
    pub include_paths: Vec<String>,
    #[serde(default)]
    pub default_compress: bool,
}

impl TexpandConfig {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let config_path = match path {
            Some(p) => p.to_path_buf(),
            None => default_config_path(),
        };

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read config '{}'", config_path.display()))?;

        toml::from_str(&content)
            .with_context(|| format!("failed to parse config '{}'", config_path.display()))
    }
}

fn default_config_path() -> PathBuf {
    if let Ok(xdg_home) = std::env::var("XDG_CONFIG_HOME") {
        std::path::PathBuf::from(xdg_home).join("texpand.toml")
    } else if let Ok(home) = std::env::var("HOME") {
        std::path::PathBuf::from(home)
            .join(".config")
            .join("texpand.toml")
    } else {
        "texpand.toml".into()
    }
}
