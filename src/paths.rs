//! Resolution of the `~/.kero` runtime directory layout.

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Root of all kero runtime state. Overridable via `KERO_HOME`.
pub fn home() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("KERO_HOME") {
        return Ok(PathBuf::from(p));
    }
    let base = dirs::home_dir().context("cannot determine home directory")?;
    Ok(base.join(".kero"))
}

pub fn config_file() -> Result<PathBuf> {
    Ok(home()?.join("kero.toml"))
}

pub fn bin_dir() -> Result<PathBuf> {
    Ok(home()?.join("bin"))
}

pub fn versions_file() -> Result<PathBuf> {
    Ok(home()?.join("versions.json"))
}

pub fn easytier_dir() -> Result<PathBuf> {
    Ok(home()?.join("easytier"))
}

pub fn easytier_config() -> Result<PathBuf> {
    Ok(easytier_dir()?.join("config.toml"))
}

pub fn syncthing_home() -> Result<PathBuf> {
    Ok(home()?.join("syncthing"))
}

pub fn run_dir() -> Result<PathBuf> {
    Ok(home()?.join("run"))
}

/// Path to a managed binary under `~/.kero/bin`.
pub fn binary(name: &str) -> Result<PathBuf> {
    let name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    Ok(bin_dir()?.join(name))
}

/// Create the full directory skeleton if missing.
pub fn ensure_dirs() -> Result<()> {
    for d in [
        home()?,
        bin_dir()?,
        easytier_dir()?,
        syncthing_home()?,
        run_dir()?,
    ] {
        std::fs::create_dir_all(&d)
            .with_context(|| format!("creating directory {}", d.display()))?;
    }
    Ok(())
}
