use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::{Config, LaunchMode};
use crate::constants;
use crate::domain::Region;
use crate::error::{Error, Result};
use crate::logln;

pub fn launch(config: &Config, region: Region) -> Result<()> {
    logln!("[launcher] Launch mode: {:?}", config.launch_mode);
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

    logln!("[launcher] Steam URI: {}", steam_uri);
    open_uri(&steam_uri, "Steam")?;

    logln!("[launcher] Steam launch initiated");
    Ok(())
}

fn launch_battle_net(region: Region) -> Result<()> {
    let address = region.ping_host();
    let uri = format!("battlenet://d2r/?address={}", address);

    logln!("[launcher] Battle.net URI: {}", uri);
    open_uri(&uri, "Battle.net")?;

    logln!("[launcher] Battle.net launch initiated");
    Ok(())
}

fn launch_direct(config: &Config, region: Region) -> Result<()> {
    let path = config
        .d2r_path
        .as_ref()
        .ok_or_else(|| Error::Launch("D2R path not configured".into()))?;

    let exe = resolve_direct_executable(path);
    if !exe.exists() {
        return Err(Error::ExecutableNotFound(exe));
    }

    let address = region.ping_host();
    logln!("[launcher] Direct: {} -address {}", exe.display(), address);

    let mut cmd = Command::new(exe);
    cmd.arg("-address").arg(address);

    for (key, value) in env::vars() {
        cmd.env(key, value);
    }

    cmd.spawn()
        .map_err(|e| Error::Launch(format!("Failed to spawn D2R: {e}")))?;

    logln!("[launcher] Direct launch initiated");
    Ok(())
}

fn launch_custom(config: &Config, region: Region) -> Result<()> {
    let command = config
        .custom_command
        .as_ref()
        .ok_or_else(|| Error::Launch("Custom command not configured".into()))?;

    let command = expand_custom_command(command, region);

    let shell = custom_shell_invocation(&command);
    logln!(
        "[launcher] Custom: {} {}",
        shell.program,
        shell.args.join(" ")
    );

    let mut cmd = Command::new(shell.program);
    cmd.args(&shell.args);

    for (key, value) in env::vars() {
        cmd.env(key, value);
    }

    cmd.spawn()
        .map_err(|e| Error::Launch(format!("Failed to run custom command: {e}")))?;

    logln!("[launcher] Custom command initiated");
    Ok(())
}

fn open_uri(uri: &str, launcher_name: &str) -> Result<()> {
    let invocation = open_uri_invocation(uri);
    let mut cmd = Command::new(invocation.program);
    cmd.args(&invocation.args);

    for (key, value) in env::vars() {
        cmd.env(key, value);
    }

    cmd.spawn().map_err(|e| {
        Error::Launch(format!(
            "Failed to spawn {} via {}: {e}",
            launcher_name, invocation.program
        ))
    })?;

    Ok(())
}

fn resolve_direct_executable(path: &Path) -> PathBuf {
    if path
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case(constants::D2R_EXE))
    {
        path.to_path_buf()
    } else {
        path.join(constants::D2R_EXE)
    }
}

fn expand_custom_command(command: &str, region: Region) -> String {
    command.replace("{address}", region.ping_host())
}

#[derive(Debug, PartialEq, Eq)]
struct CommandInvocation {
    program: &'static str,
    args: Vec<String>,
}

fn open_uri_invocation(uri: &str) -> CommandInvocation {
    #[cfg(target_os = "windows")]
    {
        CommandInvocation {
            program: "cmd",
            args: vec!["/c".into(), "start".into(), uri.to_string()],
        }
    }

    #[cfg(target_os = "macos")]
    {
        CommandInvocation {
            program: "open",
            args: vec![uri.to_string()],
        }
    }

    #[cfg(target_os = "linux")]
    {
        CommandInvocation {
            program: "xdg-open",
            args: vec![uri.to_string()],
        }
    }
}

fn custom_shell_invocation(command: &str) -> CommandInvocation {
    #[cfg(target_os = "windows")]
    {
        CommandInvocation {
            program: "cmd",
            args: vec!["/C".into(), command.to_string()],
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        CommandInvocation {
            program: "sh",
            args: vec!["-c".into(), command.to_string()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{custom_shell_invocation, expand_custom_command, resolve_direct_executable};
    use crate::domain::Region;
    use std::path::Path;

    #[test]
    fn resolve_direct_executable_accepts_install_directory() {
        let path = Path::new("/games/d2r");
        assert_eq!(resolve_direct_executable(path), path.join("D2R.exe"));
    }

    #[test]
    fn resolve_direct_executable_accepts_executable_path() {
        let path = Path::new("/games/d2r/D2R.exe");
        assert_eq!(resolve_direct_executable(path), path);
    }

    #[test]
    fn expand_custom_command_replaces_region_address_placeholder() {
        let command = expand_custom_command("launch --address {address}", Region::Europe);
        assert_eq!(command, "launch --address eu.actual.battle.net");
    }

    #[test]
    fn custom_shell_invocation_uses_platform_shell() {
        let invocation = custom_shell_invocation("echo test");

        #[cfg(target_os = "windows")]
        {
            assert_eq!(invocation.program, "cmd");
            assert_eq!(invocation.args, vec!["/C", "echo test"]);
        }

        #[cfg(not(target_os = "windows"))]
        {
            assert_eq!(invocation.program, "sh");
            assert_eq!(invocation.args, vec!["-c", "echo test"]);
        }
    }
}
