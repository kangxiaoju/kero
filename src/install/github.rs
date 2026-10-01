//! GitHub release querying and asset download with optional proxy + sha256.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
}

fn client(proxy: Option<&str>) -> Result<reqwest::blocking::Client> {
    let mut b = reqwest::blocking::Client::builder()
        .user_agent("kero/0.1")
        .timeout(Duration::from_secs(120));
    // Explicit flag wins; otherwise honor standard env proxies.
    if let Some(p) = proxy {
        b = b.proxy(reqwest::Proxy::all(p).context("invalid proxy url")?);
    }
    b.build().context("building http client")
}

/// Fetch the latest release, or a specific tag if `version` is given.
pub fn fetch_release(repo: &str, version: Option<&str>, proxy: Option<&str>) -> Result<Release> {
    let url = match version {
        Some(v) => {
            let tag = if v.starts_with('v') {
                v.to_string()
            } else {
                format!("v{v}")
            };
            format!("https://api.github.com/repos/{repo}/releases/tags/{tag}")
        }
        None => format!("https://api.github.com/repos/{repo}/releases/latest"),
    };
    let resp = client(proxy)?
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .with_context(|| format!("GET {url}"))?;
    if !resp.status().is_success() {
        bail!("github api {} returned {}", url, resp.status());
    }
    Ok(resp.json().context("parsing release json")?)
}

/// Strip leading `v` from a tag to get a bare version string.
pub fn version_from_tag(tag: &str) -> String {
    tag.trim_start_matches('v').to_string()
}

pub fn find_asset<'a>(rel: &'a Release, name: &str) -> Result<&'a Asset> {
    rel.assets
        .iter()
        .find(|a| a.name == name)
        .with_context(|| format!("asset {name} not found in release {}", rel.tag_name))
}

pub fn download(url: &str, proxy: Option<&str>) -> Result<Vec<u8>> {
    let resp = client(proxy)?
        .get(url)
        .send()
        .with_context(|| format!("downloading {url}"))?;
    if !resp.status().is_success() {
        bail!("download {} returned {}", url, resp.status());
    }
    Ok(resp.bytes()?.to_vec())
}

pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}
