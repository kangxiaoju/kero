//! EasyTier integration: generate an isolated config.toml, supervise
//! easytier-core (方案 A), and query status via easytier-cli.

use anyhow::{Context, Result};

use crate::config::Config;
use crate::paths;
use crate::process;

pub const PROC: &str = "easytier";

/// Render an EasyTier TOML config from the unified kero config.
/// Deliberately omits any web/config-server so no web page is exposed.
pub fn render_config(cfg: &Config) -> String {
    let n = &cfg.network;
    let mut s = String::new();
    // Rebuild cleanly using the array form EasyTier accepts.
    s.push_str(&format!("instance_name = \"{}\"\n", n.name));
    s.push_str(&format!("hostname = \"{}\"\n", n.hostname));
    s.push_str(&format!("dhcp = {}\n", n.dhcp));
    if !n.dhcp && !n.ip.is_empty() {
        s.push_str(&format!("ipv4 = \"{}\"\n", n.ip));
    }
    // Note: `dev_name` lives under [flags]; `rpc_portal` is a CLI-only flag
    // (not a TOML field), both applied in `start()` / below.
    let listeners = n
        .listeners
        .iter()
        .map(|l| format!("\"{l}\""))
        .collect::<Vec<_>>()
        .join(", ");
    s.push_str(&format!("listeners = [{listeners}]\n"));
    s.push('\n');

    s.push_str("[network_identity]\n");
    s.push_str(&format!("network_name = \"{}\"\n", n.name));
    s.push_str(&format!("network_secret = \"{}\"\n", n.secret));
    s.push('\n');

    for p in &n.peers {
        s.push_str("[[peer]]\n");
        s.push_str(&format!("uri = \"{p}\"\n"));
    }
    s.push('\n');
    s.push_str("[flags]\n");
    s.push_str(&format!("dev_name = \"{}\"\n", n.dev_name));
    s.push_str("enable_kcp_proxy = true\n");
    s
}

/// Write the generated EasyTier config to ~/.kero/easytier/config.toml.
pub fn write_config(cfg: &Config) -> Result<()> {
    paths::ensure_dirs()?;
    let out = paths::easytier_config()?;
    std::fs::write(&out, render_config(cfg))
        .with_context(|| format!("writing {}", out.display()))?;
    Ok(())
}

pub fn start(cfg: &Config) -> Result<i32> {
    write_config(cfg)?;
    let bin = paths::binary("easytier-core")?;
    let config = paths::easytier_config()?;
    // `rpc_portal` is not a TOML field in EasyTier; pass it as a CLI flag so
    // `kero net`/`easytier-cli` can reach the managed instance on a fixed port.
    let args = vec![
        "-c".to_string(),
        config.to_string_lossy().to_string(),
        "--rpc-portal".to_string(),
        cfg.network.rpc_portal.clone(),
    ];
    process::spawn(PROC, &bin, &args, &[])
}

pub fn stop() -> Result<bool> {
    process::stop(PROC)
}

pub fn is_running() -> bool {
    process::is_running(PROC)
}

/// Run easytier-cli against the managed rpc portal and return raw text.
pub fn cli(cfg: &Config, sub: &[&str]) -> Result<String> {
    let bin = paths::binary("easytier-cli")?;
    let out = std::process::Command::new(&bin)
        .arg("-p")
        .arg(&cfg.network.rpc_portal)
        .args(sub)
        .output()
        .with_context(|| format!("running easytier-cli {sub:?}"))?;
    if !out.status.success() {
        return Ok(format!(
            "easytier-cli error: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn peers(cfg: &Config) -> Result<String> {
    cli(cfg, &["peer"])
}
