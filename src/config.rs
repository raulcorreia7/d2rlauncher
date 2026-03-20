use std::path::{Path, PathBuf};

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

    pub fn resolved_d2r_path(&self) -> Option<PathBuf> {
        self.d2r_path
            .as_deref()
            .map(|path| resolve_config_path(path, Self::config_dir().as_deref()))
    }

    fn config_path() -> PathBuf {
        Self::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(constants::CONFIG_FILE)
    }
}

fn resolve_config_path(path: &Path, base_dir: Option<&Path>) -> PathBuf {
    let path = expand_home_path(path);
    if path.is_absolute() {
        return path;
    }

    match base_dir {
        Some(base_dir) => base_dir.join(path),
        None => path,
    }
}

fn expand_home_path(path: &Path) -> PathBuf {
    let path_str = path.to_string_lossy();
    let Some(stripped) = path_str.strip_prefix('~') else {
        return path.to_path_buf();
    };

    let Some(home_dir) = home_dir() else {
        return path.to_path_buf();
    };

    let suffix = stripped.trim_start_matches(['/', '\\']);
    if suffix.is_empty() {
        home_dir
    } else {
        home_dir.join(suffix)
    }
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .or_else(|| {
                let drive = std::env::var_os("HOMEDRIVE")?;
                let path = std::env::var_os("HOMEPATH")?;
                let mut home = PathBuf::from(drive);
                home.push(path);
                Some(home)
            })
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_config_path, Config, LaunchMode};
    use std::path::Path;

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

    #[test]
    fn resolve_config_path_should_use_config_directory_for_relative_paths() {
        let resolved = resolve_config_path(
            Path::new("Diablo II Resurrected"),
            Some(Path::new("/launcher")),
        );
        assert_eq!(resolved, Path::new("/launcher/Diablo II Resurrected"));
    }

    #[test]
    fn resolve_config_path_should_keep_absolute_paths() {
        let resolved = resolve_config_path(
            Path::new("/games/Diablo II Resurrected"),
            Some(Path::new("/launcher")),
        );
        assert_eq!(resolved, Path::new("/games/Diablo II Resurrected"));
    }
}
