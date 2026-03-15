use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::constants;
use crate::domain::Region;
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub launch_mode: LaunchMode,
    #[serde(default)]
    pub d2r_path: Option<PathBuf>,
    #[serde(default)]
    pub custom_command: Option<String>,
    #[serde(default = "default_quick_launch")]
    pub quick_launch: bool,
    #[serde(default)]
    pub default_region: Option<Region>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchMode {
    #[default]
    Steam,
    BattleNet,
    Direct,
    Custom,
}

fn default_quick_launch() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            launch_mode: LaunchMode::Steam,
            d2r_path: None,
            custom_command: None,
            quick_launch: true,
            default_region: None,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        eprintln!("[config] Loading from: {}", path.display());

        if !path.exists() {
            eprintln!("[config] File not found, creating default");
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        let content = std::fs::read_to_string(&path)?;
        let config: Self = serde_json::from_str(&content)?;
        eprintln!("[config] Loaded successfully");
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        eprintln!("[config] Saving to: {}", path.display());

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        eprintln!("[config] Saved successfully");
        Ok(())
    }

    pub fn config_dir() -> Option<PathBuf> {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
    }

    fn config_path() -> PathBuf {
        Self::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(constants::CONFIG_FILE)
    }
}
