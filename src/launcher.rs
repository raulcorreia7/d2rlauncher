use std::env;
use std::process::Command;

use crate::config::{Config, LaunchMode};
use crate::constants;
use crate::domain::Region;
use crate::error::{Error, Result};

pub fn launch(config: &Config, region: Region) -> Result<()> {
    eprintln!("[launcher] Launch mode: {:?}", config.launch_mode);
    match config.launch_mode {
        LaunchMode::Steam => launch_steam(region),
        LaunchMode::BattleNet => launch_battle_net(region),
        LaunchMode::Direct => launch_direct(config, region),
        LaunchMode::Custom => launch_custom(config, region),
    }
}

fn launch_steam(region: Region) -> Result<()> {
    let address = region.ping_host();
    let steam_uri = format!(
        "steam://run/{}//-address%20{}",
        constants::D2R_STEAM_APP_ID,
        address
    );

    eprintln!("[launcher] Steam URI: {}", steam_uri);

    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new("cmd");
        cmd.arg("/c").arg("start").arg(&steam_uri);

        cmd.spawn()
            .map_err(|e| Error::Launch(format!("Failed to spawn Steam: {e}")))?;
    }

    #[cfg(target_os = "macos")]
    {
        let mut cmd = Command::new("open");
        cmd.arg(&steam_uri);

        cmd.spawn()
            .map_err(|e| Error::Launch(format!("Failed to spawn Steam: {e}")))?;
    }

    #[cfg(target_os = "linux")]
    {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(&steam_uri);

        for (key, value) in env::vars() {
            cmd.env(key, value);
        }

        cmd.spawn()
            .map_err(|e| Error::Launch(format!("Failed to spawn Steam: {e}")))?;
    }

    eprintln!("[launcher] Steam launch initiated");
    Ok(())
}

fn launch_battle_net(region: Region) -> Result<()> {
    let address = region.ping_host();
    let uri = format!("battlenet://d2r/?address={}", address);

    eprintln!("[launcher] Battle.net URI: {}", uri);

    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new("cmd");
        cmd.arg("/c").arg("start").arg(&uri);

        cmd.spawn()
            .map_err(|e| Error::Launch(format!("Failed to spawn Battle.net: {e}")))?;
    }

    #[cfg(target_os = "macos")]
    {
        let mut cmd = Command::new("open");
        cmd.arg(&uri);

        cmd.spawn()
            .map_err(|e| Error::Launch(format!("Failed to spawn Battle.net: {e}")))?;
    }

    #[cfg(target_os = "linux")]
    {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(&uri);

        cmd.spawn()
            .map_err(|e| Error::Launch(format!("Failed to spawn Battle.net: {e}")))?;
    }

    eprintln!("[launcher] Battle.net launch initiated");
    Ok(())
}

fn launch_direct(config: &Config, region: Region) -> Result<()> {
    let path = config
        .d2r_path
        .as_ref()
        .ok_or_else(|| Error::Launch("D2R path not configured".into()))?;

    let exe = path.join(constants::D2R_EXE);
    if !exe.exists() {
        return Err(Error::ExecutableNotFound(path.clone()));
    }

    let address = region.ping_host();
    eprintln!("[launcher] Direct: {} -address {}", exe.display(), address);

    let mut cmd = Command::new(exe);
    cmd.arg("-address").arg(address);

    for (key, value) in env::vars() {
        cmd.env(key, value);
    }

    cmd.spawn()
        .map_err(|e| Error::Launch(format!("Failed to spawn D2R: {e}")))?;

    eprintln!("[launcher] Direct launch initiated");
    Ok(())
}

fn launch_custom(config: &Config, region: Region) -> Result<()> {
    let command = config
        .custom_command
        .as_ref()
        .ok_or_else(|| Error::Launch("Custom command not configured".into()))?;

    let address = region.ping_host();
    let command = command.replace("{address}", address);

    eprintln!("[launcher] Custom: sh -c \"{}\"", command);

    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(&command);

    for (key, value) in env::vars() {
        cmd.env(key, value);
    }

    cmd.spawn()
        .map_err(|e| Error::Launch(format!("Failed to run custom command: {e}")))?;

    eprintln!("[launcher] Custom command initiated");
    Ok(())
}
