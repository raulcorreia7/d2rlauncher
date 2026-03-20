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
        LaunchMode::Direct => launch_direct(config, region),
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

fn launch_direct(config: &Config, region: Region) -> Result<()> {
    let exe = find_direct_executable(config)?;
    let address = region.ping_host();
    logln!("[launcher] Direct: {} -address {}", exe.display(), address);

    let mut cmd = Command::new(&exe);
    cmd.current_dir(exe.parent().unwrap_or_else(|| Path::new(".")));
    cmd.arg("-address").arg(address);

    cmd.spawn()
        .map_err(|e| Error::Launch(format!("Failed to spawn D2R: {e}")))?;

    logln!("[launcher] Direct launch initiated");
    Ok(())
}

fn open_uri(uri: &str, launcher_name: &str) -> Result<()> {
    let invocation = open_uri_invocation(uri);
    let mut cmd = Command::new(invocation.program);
    cmd.args(&invocation.args);

    cmd.spawn().map_err(|e| {
        Error::Launch(format!(
            "Failed to spawn {} via {}: {e}",
            launcher_name, invocation.program
        ))
    })?;

    Ok(())
}

fn find_direct_executable(config: &Config) -> Result<PathBuf> {
    let launcher_exe = std::env::current_exe().ok();
    let candidates = direct_executable_candidates(
        config.resolved_d2r_path().as_deref(),
        launcher_exe.as_deref(),
    );

    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    Err(Error::ExecutableNotFound(
        candidates
            .into_iter()
            .next()
            .unwrap_or_else(|| PathBuf::from(constants::D2R_EXE)),
    ))
}

fn direct_executable_candidates(
    config_path: Option<&Path>,
    launcher_exe: Option<&Path>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(path) = config_path {
        push_unique_path(&mut candidates, resolve_direct_executable(path));
    }

    if let Some(path) = launcher_exe {
        push_unique_path(&mut candidates, resolve_sibling_direct_executable(path));
    }

    candidates
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

fn resolve_sibling_direct_executable(launcher_exe: &Path) -> PathBuf {
    launcher_exe
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(constants::D2R_EXE)
}

fn push_unique_path(paths: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !paths.iter().any(|path| path == &candidate) {
        paths.push(candidate);
    }
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

#[cfg(test)]
mod tests {
    use super::{
        direct_executable_candidates, open_uri_invocation, resolve_direct_executable,
        resolve_sibling_direct_executable,
    };
    use crate::constants;
    use std::path::Path;

    #[test]
    fn resolve_direct_executable_accepts_install_directory() {
        let path = Path::new("/games/d2r");
        assert_eq!(
            resolve_direct_executable(path),
            path.join(constants::D2R_EXE)
        );
    }

    #[test]
    fn resolve_direct_executable_accepts_executable_path() {
        let path = Path::new("/games/d2r/D2R.exe");
        assert_eq!(resolve_direct_executable(path), path);
    }

    #[test]
    fn resolve_sibling_direct_executable_should_use_launcher_directory() {
        let launcher = Path::new("/games/d2rlauncher/d2rlauncher");
        assert_eq!(
            resolve_sibling_direct_executable(launcher),
            Path::new("/games/d2rlauncher").join(constants::D2R_EXE)
        );
    }

    #[test]
    fn direct_executable_candidates_should_prefer_configured_path_then_sibling_path() {
        let candidates = direct_executable_candidates(
            Some(Path::new("/games/d2r")),
            Some(Path::new("/tools/d2rlauncher")),
        );

        assert_eq!(
            candidates,
            vec![
                Path::new("/games/d2r").join(constants::D2R_EXE),
                Path::new("/tools").join(constants::D2R_EXE),
            ]
        );
    }

    #[test]
    fn open_uri_invocation_should_use_platform_launcher() {
        let invocation = open_uri_invocation("steam://run/2536520");

        #[cfg(target_os = "windows")]
        {
            assert_eq!(invocation.program, "cmd");
            assert_eq!(invocation.args, vec!["/c", "start", "steam://run/2536520"]);
        }

        #[cfg(target_os = "macos")]
        {
            assert_eq!(invocation.program, "open");
            assert_eq!(invocation.args, vec!["steam://run/2536520"]);
        }

        #[cfg(target_os = "linux")]
        {
            assert_eq!(invocation.program, "xdg-open");
            assert_eq!(invocation.args, vec!["steam://run/2536520"]);
        }
    }
}
