//! Minimal REST API daemon for managing mesh + sync without a TUI.
//!
//! Intentionally dependency-light: a small threaded HTTP/1.1 server over
//! std::net, speaking JSON. Bound to localhost by default (see [daemon] in
//! kero.toml). Endpoints:
//!
//!   GET  /health                 -> { ok }
//!   GET  /status                 -> { net, sync }
//!   GET  /net/peers              -> { peers: <raw text> }
//!   GET  /net/info               -> { info: <raw text> }
//!   GET  /sync/folders           -> [ {id,label,path,state,completion} ]
//!   POST /sync/folders           -> add folder  {id?,path,devices?[]}
//!   DELETE /sync/folders/<id>    -> remove folder
//!   GET  /sync/devices           -> [ {deviceID,name,addresses} ]  (raw passthrough)
//!   POST /sync/devices           -> add device  {id,address,name?}
//!   GET  /sync/id                -> { deviceID }

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};

use crate::config::Config;
use crate::net;
use crate::sync::syncthing::Syncthing;
use crate::sync::{FolderSpec, SyncBackend};

pub fn serve(cfg: Config) -> Result<()> {
    let addr = cfg.daemon.api_addr.clone();
    let listener = TcpListener::bind(&addr)
        .with_context(|| format!("binding kero api on {addr}"))?;
    println!("kero api listening on http://{addr}");
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let cfg = cfg.clone();
                // One thread per connection; load is tiny (local management UI).
                std::thread::spawn(move || {
                    if let Err(e) = handle(s, &cfg) {
                        eprintln!("api: {e}");
                    }
                });
            }
            Err(e) => eprintln!("api accept: {e}"),
        }
    }
    Ok(())
}

struct Req {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn parse_request(stream: &mut TcpStream) -> Result<Req> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();

    let mut content_length = 0usize;
    loop {
        let mut h = String::new();
        reader.read_line(&mut h)?;
        if h == "\r\n" || h.is_empty() {
            break;
        }
        if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
            content_length = v.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }
    Ok(Req { method, path, body })
}

fn handle(mut stream: TcpStream, cfg: &Config) -> Result<()> {
    let req = parse_request(&mut stream)?;
    let (code, payload) = route(cfg, &req);
    let body = serde_json::to_vec(&payload).unwrap_or_default();
    let status = match code {
        200 => "200 OK",
        201 => "201 Created",
        400 => "400 Bad Request",
        404 => "404 Not Found",
        500 => "500 Internal Server Error",
        _ => "200 OK",
    };
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(resp.as_bytes())?;
    stream.write_all(&body)?;
    stream.flush()?;
    Ok(())
}

fn err(code: u16, msg: impl Into<String>) -> (u16, Value) {
    (code, json!({ "error": msg.into() }))
}

fn syncthing(cfg: &Config) -> Option<Syncthing> {
    cfg.sync.as_ref().map(Syncthing::from_config)
}

fn route(cfg: &Config, req: &Req) -> (u16, Value) {
    let m = req.method.as_str();
    let p = req.path.as_str();
    match (m, p) {
        ("GET", "/health") => (200, json!({ "ok": true })),

        ("GET", "/status") => {
            let net_running = net::is_running();
            let net_peers = if net_running {
                net::peers(cfg).unwrap_or_default()
            } else {
                String::new()
            };
            let sync = match syncthing(cfg) {
                None => json!({ "enabled": false }),
                Some(st) => {
                    let running = st.is_running();
                    let s = if running { st.status().ok() } else { None };
                    match s {
                        Some(s) => json!({
                            "enabled": true,
                            "running": running,
                            "deviceId": s.device_id,
                            "connectedDevices": s.connected_devices,
                            "folders": s.folders.iter().map(|f| json!({
                                "id": f.id, "label": f.label, "path": f.path,
                                "state": f.state, "completion": f.completion
                            })).collect::<Vec<_>>(),
                        }),
                        None => json!({ "enabled": true, "running": running }),
                    }
                }
            };
            (200, json!({
                "net": { "running": net_running, "peers": net_peers },
                "sync": sync,
            }))
        }

        ("GET", "/net/peers") => match net::peers(cfg) {
            Ok(t) => (200, json!({ "peers": t })),
            Err(e) => err(500, e.to_string()),
        },
        ("GET", "/net/info") => match net::cli(cfg, &["node"]) {
            Ok(t) => (200, json!({ "info": t })),
            Err(e) => err(500, e.to_string()),
        },

        ("GET", "/sync/id") => match syncthing(cfg) {
            None => err(400, "sync disabled on this host"),
            Some(st) => match st.device_id() {
                Ok(id) => (200, json!({ "deviceId": id })),
                Err(e) => err(500, e.to_string()),
            },
        },

        ("GET", "/sync/folders") => match syncthing(cfg) {
            None => err(400, "sync disabled on this host"),
            Some(st) => match st.list_folders() {
                Ok(fs) => (
                    200,
                    json!(fs
                        .iter()
                        .map(|f| json!({
                            "id": f.id, "label": f.label, "path": f.path,
                            "state": f.state, "completion": f.completion
                        }))
                        .collect::<Vec<_>>()),
                ),
                Err(e) => err(500, e.to_string()),
            },
        },

        ("POST", "/sync/folders") => {
            let Some(st) = syncthing(cfg) else {
                return err(400, "sync disabled on this host");
            };
            let v: Value = match serde_json::from_slice(&req.body) {
                Ok(v) => v,
                Err(e) => return err(400, format!("bad json: {e}")),
            };
            let Some(path) = v.get("path").and_then(|x| x.as_str()) else {
                return err(400, "missing 'path'");
            };
            let id = v
                .get("id")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| default_folder_id(path));
            let devices = v
                .get("devices")
                .and_then(|x| x.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|d| d.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let spec = FolderSpec {
                id: id.clone(),
                label: id.clone(),
                path: path.to_string(),
                devices,
            };
            std::fs::create_dir_all(path).ok();
            match st.add_folder(&spec) {
                Ok(_) => (201, json!({ "id": id, "path": path })),
                Err(e) => err(500, e.to_string()),
            }
        }

        ("GET", "/sync/devices") => match syncthing(cfg) {
            None => err(400, "sync disabled on this host"),
            Some(st) => match st.raw_get("/rest/config/devices") {
                Ok(v) => (200, v),
                Err(e) => err(500, e.to_string()),
            },
        },

        ("POST", "/sync/devices") => {
            let Some(st) = syncthing(cfg) else {
                return err(400, "sync disabled on this host");
            };
            let v: Value = match serde_json::from_slice(&req.body) {
                Ok(v) => v,
                Err(e) => return err(400, format!("bad json: {e}")),
            };
            let (Some(id), Some(addr)) = (
                v.get("id").and_then(|x| x.as_str()),
                v.get("address").and_then(|x| x.as_str()),
            ) else {
                return err(400, "missing 'id' or 'address'");
            };
            let name = v.get("name").and_then(|x| x.as_str()).unwrap_or(id);
            match st.add_device(id, addr, name) {
                Ok(_) => (201, json!({ "id": id, "address": addr })),
                Err(e) => err(500, e.to_string()),
            }
        }

        ("DELETE", _) if p.starts_with("/sync/folders/") => {
            let Some(st) = syncthing(cfg) else {
                return err(400, "sync disabled on this host");
            };
            let id = p.trim_start_matches("/sync/folders/");
            match st.remove_folder(id) {
                Ok(_) => (200, json!({ "removed": id })),
                Err(e) => err(500, e.to_string()),
            }
        }

        _ => err(404, "not found"),
    }
}

fn default_folder_id(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("folder")
        .to_string()
}
