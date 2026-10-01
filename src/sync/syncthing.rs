//! Syncthing backend: spawn headless (no user-facing web, REST bound to
//! localhost) and drive it entirely through the REST API.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

use super::{FolderSpec, FolderStatus, SyncBackend, SyncStatus};
use crate::config::Sync as SyncCfg;
use crate::paths;
use crate::process;

pub const PROC: &str = "syncthing";

pub struct Syncthing {
    api_addr: String,
    api_key: String,
    sync_port: u16,
    listen_ip: String,
}

impl Syncthing {
    pub fn from_config(sync: &SyncCfg) -> Self {
        Syncthing {
            api_addr: sync.api_addr.clone(),
            api_key: api_key(),
            sync_port: sync.sync_port,
            listen_ip: sync.listen_ip.clone(),
        }
    }

    fn base(&self) -> String {
        format!("http://{}", self.api_addr)
    }

    fn http(&self) -> Result<reqwest::blocking::Client> {
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .context("building http client")
    }

    fn get(&self, path: &str) -> Result<Value> {
        let resp = self
            .http()?
            .get(format!("{}{}", self.base(), path))
            .header("X-API-Key", &self.api_key)
            .send()
            .with_context(|| format!("GET {path}"))?;
        if !resp.status().is_success() {
            bail!("GET {} -> {}", path, resp.status());
        }
        Ok(resp.json().unwrap_or(Value::Null))
    }

    /// Public raw GET passthrough for the REST API layer.
    pub fn raw_get(&self, path: &str) -> Result<Value> {
        self.get(path)
    }

    fn put(&self, path: &str, body: &Value) -> Result<()> {
        let resp = self
            .http()?
            .put(format!("{}{}", self.base(), path))
            .header("X-API-Key", &self.api_key)
            .json(body)
            .send()
            .with_context(|| format!("PUT {path}"))?;
        if !resp.status().is_success() {
            bail!("PUT {} -> {}", path, resp.status());
        }
        Ok(())
    }

    /// Ensure a Syncthing identity/config exists (one-time `generate`).
    pub fn ensure_generated(&self) -> Result<()> {
        let home = paths::syncthing_home()?;
        let marker = home.join("config.xml");
        if marker.exists() {
            return Ok(());
        }
        std::fs::create_dir_all(&home)?;
        let bin = paths::binary(PROC)?;
        let out = std::process::Command::new(&bin)
            .arg("generate")
            .arg("--home")
            .arg(&home)
            .output()
            .context("running syncthing generate")?;
        if !out.status.success() {
            bail!(
                "syncthing generate failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(())
    }

    /// Block until the REST API answers (after start).
    fn wait_ready(&self) -> Result<()> {
        for _ in 0..50 {
            if self.get("/rest/system/ping").is_ok() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        bail!("syncthing REST API did not become ready at {}", self.api_addr)
    }

    /// Disable discovery/relay/NAT and bind BEP to the mesh IP, per design.
    fn apply_headless_options(&self) -> Result<()> {
        let listen = format!("tcp://{}:{}", self.listen_ip, self.sync_port);
        let body = json!({
            "globalAnnounceEnabled": false,
            "localAnnounceEnabled": false,
            "relaysEnabled": false,
            "natEnabled": false,
            "urAccepted": -1,
            "startBrowser": false,
            "listenAddresses": [listen],
        });
        self.put("/rest/config/options", &body)
    }
}

/// Fixed API key stored under the syncthing home so kero can reconnect.
fn api_key() -> String {
    if let Ok(k) = std::env::var("KERO_ST_APIKEY") {
        return k;
    }
    // Stable per-install key derived from the home path.
    let home = paths::syncthing_home()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let digest = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(b"kero-syncthing-apikey:");
        h.update(home.as_bytes());
        hex::encode(h.finalize())
    };
    digest[..32].to_string()
}

impl SyncBackend for Syncthing {
    fn start(&self) -> Result<()> {
        self.ensure_generated()?;
        let home = paths::syncthing_home()?;
        let args = vec![
            "serve".to_string(),
            "--home".to_string(),
            home.to_string_lossy().to_string(),
            "--gui-address".to_string(),
            self.api_addr.clone(),
            "--gui-apikey".to_string(),
            self.api_key.clone(),
            "--no-browser".to_string(),
            "--no-restart".to_string(),
        ];
        process::spawn(PROC, &paths::binary(PROC)?, &args, &[])?;
        self.wait_ready()?;
        self.apply_headless_options()?;
        Ok(())
    }

    fn stop(&self) -> Result<bool> {
        process::stop(PROC)
    }

    fn is_running(&self) -> bool {
        process::is_running(PROC)
    }

    fn device_id(&self) -> Result<String> {
        let v = self.get("/rest/system/status")?;
        v.get("myID")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .context("myID missing from status")
    }

    fn add_device(&self, id: &str, addr: &str, name: &str) -> Result<()> {
        let body = json!({
            "deviceID": id,
            "name": name,
            "addresses": [addr],
            "autoAcceptFolders": false,
        });
        self.put(&format!("/rest/config/devices/{id}"), &body)
    }

    fn add_folder(&self, f: &FolderSpec) -> Result<()> {
        let mut devices: Vec<Value> = Vec::new();
        // Always include self.
        if let Ok(me) = self.device_id() {
            devices.push(json!({ "deviceID": me }));
        }
        for d in &f.devices {
            devices.push(json!({ "deviceID": d }));
        }
        let label = if f.label.is_empty() { &f.id } else { &f.label };
        let body = json!({
            "id": f.id,
            "label": label,
            "path": f.path,
            "type": "sendreceive",
            "devices": devices,
            "fsWatcherEnabled": true,
            "rescanIntervalS": 3600,
        });
        self.put(&format!("/rest/config/folders/{}", f.id), &body)
    }

    fn remove_folder(&self, id: &str) -> Result<()> {
        let resp = self
            .http()?
            .delete(format!("{}/rest/config/folders/{}", self.base(), id))
            .header("X-API-Key", &self.api_key)
            .send()?;
        if !resp.status().is_success() {
            bail!("DELETE folder {} -> {}", id, resp.status());
        }
        Ok(())
    }

    fn list_folders(&self) -> Result<Vec<FolderStatus>> {
        let cfg = self.get("/rest/config/folders")?;
        let mut out = Vec::new();
        if let Some(arr) = cfg.as_array() {
            for f in arr {
                let id = f.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let label = f
                    .get("label")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let path = f.get("path").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let (state, completion) = self.folder_progress(&id);
                out.push(FolderStatus {
                    id,
                    label,
                    path,
                    state,
                    completion,
                });
            }
        }
        Ok(out)
    }

    fn status(&self) -> Result<SyncStatus> {
        let device_id = self.device_id().unwrap_or_default();
        let folders = self.list_folders().unwrap_or_default();
        let conns = self.get("/rest/system/connections").unwrap_or(Value::Null);
        let connected_devices = conns
            .get("connections")
            .and_then(|c| c.as_object())
            .map(|m| m.values().filter(|v| v.get("connected").and_then(|b| b.as_bool()).unwrap_or(false)).count())
            .unwrap_or(0);
        Ok(SyncStatus {
            device_id,
            folders,
            connected_devices,
        })
    }
}

impl Syncthing {
    fn folder_progress(&self, id: &str) -> (String, f64) {
        let st = self
            .get(&format!("/rest/db/status?folder={id}"))
            .unwrap_or(Value::Null);
        let state = st
            .get("state")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_string();
        let global = st.get("globalBytes").and_then(|x| x.as_f64()).unwrap_or(0.0);
        let need = st.get("needBytes").and_then(|x| x.as_f64()).unwrap_or(0.0);
        let completion = if global <= 0.0 {
            100.0
        } else {
            ((global - need) / global * 100.0).clamp(0.0, 100.0)
        };
        (state, completion)
    }
}
