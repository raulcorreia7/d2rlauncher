use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::constants;
use crate::domain::Region;
use crate::error::Result;
use crate::logln;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub launch_mode: LaunchMode,
    #[serde(default)]
    pub d2r_path: Option<PathBuf>,
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
    #[serde(alias = "battle_net", alias = "custom")]
    Direct,
}

fn default_quick_launch() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            launch_mode: LaunchMode::Steam,
            d2r_path: None,
            quick_launch: true,
            default_region: None,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        logln!("[config] Loading from: {}", path.display());

        if !path.exists() {
            logln!("[config] File not found, creating default");
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        let content = std::fs::read_to_string(&path)?;
        let config: Self = serde_json::from_str(&content)?;
        logln!("[config] Loaded successfully");
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        logln!("[config] Saving to: {}", path.display());

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        logln!("[config] Saved successfully");
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

#[cfg(test)]
mod tests {
    use super::{Config, LaunchMode};

    #[test]
    fn launch_mode_should_map_legacy_battle_net_value_to_direct() {
        let config: Config = serde_json::from_str(r#"{"launch_mode":"battle_net"}"#).unwrap();
        assert_eq!(config.launch_mode, LaunchMode::Direct);
    }

    #[test]
    fn launch_mode_should_map_legacy_custom_value_to_direct() {
        let config: Config = serde_json::from_str(r#"{"launch_mode":"custom"}"#).unwrap();
        assert_eq!(config.launch_mode, LaunchMode::Direct);
    }
}
