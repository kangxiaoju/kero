//! Child-process lifecycle management (方案 A): kero spawns and supervises
//! easytier-core and syncthing directly, tracking them by PID file.

use anyhow::{bail, Context, Result};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::paths;

fn pid_file(name: &str) -> Result<PathBuf> {
    Ok(paths::run_dir()?.join(format!("{name}.pid")))
}

fn log_file(name: &str) -> Result<PathBuf> {
    Ok(paths::run_dir()?.join(format!("{name}.log")))
}

/// Read a tracked PID if the process is still alive.
pub fn running_pid(name: &str) -> Result<Option<i32>> {
    let pf = pid_file(name)?;
    if !pf.exists() {
        return Ok(None);
    }
    let pid: i32 = std::fs::read_to_string(&pf)?.trim().parse().unwrap_or(0);
    if pid > 0 && is_alive(pid) {
        Ok(Some(pid))
    } else {
        let _ = std::fs::remove_file(&pf);
        Ok(None)
    }
}

pub fn is_running(name: &str) -> bool {
    matches!(running_pid(name), Ok(Some(_)))
}

/// Spawn a managed daemon, redirecting stdout/stderr to its log file.
pub fn spawn(name: &str, program: &Path, args: &[String], envs: &[(String, String)]) -> Result<i32> {
    if let Some(pid) = running_pid(name)? {
        bail!("{name} already running (pid {pid})");
    }
    paths::ensure_dirs()?;
    let log = log_file(name)?;
    let out = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
        .with_context(|| format!("opening log {}", log.display()))?;
    let err = out.try_clone()?;

    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err));
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let child = cmd
        .spawn()
        .with_context(|| format!("spawning {}", program.display()))?;
    let pid = child.id() as i32;
    std::fs::write(pid_file(name)?, pid.to_string())?;
    Ok(pid)
}

/// Stop a managed daemon (SIGTERM, then SIGKILL fallback).
pub fn stop(name: &str) -> Result<bool> {
    let Some(pid) = running_pid(name)? else {
        return Ok(false);
    };
    term(pid);
    for _ in 0..30 {
        if !is_alive(pid) {
            let _ = std::fs::remove_file(pid_file(name)?);
            return Ok(true);
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    kill(pid);
    let _ = std::fs::remove_file(pid_file(name)?);
    Ok(true)
}

pub fn tail_log(name: &str, lines: usize) -> Result<String> {
    let lf = log_file(name)?;
    if !lf.exists() {
        return Ok(String::new());
    }
    let content = std::fs::read_to_string(&lf)?;
    let all: Vec<&str> = content.lines().collect();
    let start = all.len().saturating_sub(lines);
    Ok(all[start..].join("\n"))
}

#[cfg(unix)]
fn is_alive(pid: i32) -> bool {
    // signal 0 probes existence without affecting the process.
    unsafe { libc_kill(pid, 0) == 0 }
}
#[cfg(unix)]
fn term(pid: i32) {
    unsafe {
        libc_kill(pid, 15);
    }
}
#[cfg(unix)]
fn kill(pid: i32) {
    unsafe {
        libc_kill(pid, 9);
    }
}

// Minimal libc kill binding to avoid pulling the whole libc crate.
#[cfg(unix)]
extern "C" {
    #[link_name = "kill"]
    fn libc_kill(pid: i32, sig: i32) -> i32;
}

#[cfg(not(unix))]
fn is_alive(_pid: i32) -> bool {
    false
}
#[cfg(not(unix))]
fn term(_pid: i32) {}
#[cfg(not(unix))]
fn kill(_pid: i32) {}

/// Ensure an executable bit is set on a freshly extracted binary.
#[cfg(unix)]
pub fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perm = std::fs::metadata(path)?.permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(path, perm)?;
    Ok(())
}
#[cfg(not(unix))]
pub fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

/// Suppress unused warning for File in non-unix builds.
#[allow(dead_code)]
fn _unused(_f: File) {}
