//! Unified kero.toml configuration: one file describing both the mesh
//! network (EasyTier) and the sync layer (Syncthing).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub network: Network,
    /// Optional: omit on easytier-only hosts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync: Option<Sync>,
    #[serde(default)]
    pub daemon: Daemon,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Daemon {
    /// Address for kero's own REST API.
    #[serde(default = "default_daemon_addr")]
    pub api_addr: String,
}

impl Default for Daemon {
    fn default() -> Self {
        Daemon {
            api_addr: default_daemon_addr(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {
    pub name: String,
    pub secret: String,
    #[serde(default)]
    pub ip: String,
    #[serde(default)]
    pub dhcp: bool,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub peers: Vec<String>,
    #[serde(default = "default_dev_name")]
    pub dev_name: String,
    #[serde(default = "default_rpc_portal")]
    pub rpc_portal: String,
    #[serde(default = "default_listeners")]
    pub listeners: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sync {
    #[serde(default)]
    pub listen_ip: String,
    #[serde(default = "default_sync_port")]
    pub sync_port: u16,
    #[serde(default = "default_api_addr")]
    pub api_addr: String,
    #[serde(default)]
    pub folder: Vec<Folder>,
    #[serde(default)]
    pub device: Vec<Device>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    #[serde(default)]
    pub label: String,
    pub path: String,
    #[serde(default)]
    pub devices: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub address: String,
}

fn default_dev_name() -> String {
    "ket0".into()
}
fn default_rpc_portal() -> String {
    "127.0.0.1:15899".into()
}
fn default_listeners() -> Vec<String> {
    vec!["tcp://0.0.0.0:11020".into(), "udp://0.0.0.0:11020".into()]
}
fn default_sync_port() -> u16 {
    22010
}
fn default_api_addr() -> String {
    "127.0.0.1:8390".into()
}
fn default_daemon_addr() -> String {
    "127.0.0.1:8391".into()
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading config {}", path.display()))?;
        let cfg: Config = toml::from_str(&raw)
            .with_context(|| format!("parsing config {}", path.display()))?;
        Ok(cfg)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let raw = toml::to_string_pretty(self).context("serializing config")?;
        std::fs::write(path, raw).with_context(|| format!("writing config {}", path.display()))?;
        Ok(())
    }

    /// A sensible default config for `kero init`.
    pub fn sample() -> Self {
        let host = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "kero-node".into());
        Config {
            network: Network {
                name: "kero-net".into(),
                secret: "change-me".into(),
                ip: "10.145.145.1".into(),
                dhcp: false,
                hostname: host,
                peers: vec![],
                dev_name: default_dev_name(),
                rpc_portal: default_rpc_portal(),
                listeners: default_listeners(),
            },
            sync: Some(Sync {
                listen_ip: "10.145.145.1".into(),
                sync_port: default_sync_port(),
                api_addr: default_api_addr(),
                folder: vec![],
                device: vec![],
            }),
            daemon: Daemon::default(),
        }
    }
}
