//! High-level install / upgrade orchestration for the managed binaries.

mod extract;
mod github;

use anyhow::{Context, Result};

use crate::paths;
use crate::platform::{Os, Platform};
use crate::state::Versions;

pub const EASYTIER_REPO: &str = "EasyTier/EasyTier";
pub const SYNCTHING_REPO: &str = "syncthing/syncthing";

/// Resolve proxy: explicit flag first, then standard env vars.
fn resolve_proxy(flag: Option<&str>) -> Option<String> {
    if let Some(p) = flag {
        return Some(p.to_string());
    }
    for k in ["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy"] {
        if let Ok(v) = std::env::var(k) {
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

pub struct InstallOptions<'a> {
    pub proxy: Option<&'a str>,
    pub et_version: Option<&'a str>,
    pub st_version: Option<&'a str>,
    /// Skip syncthing (easytier-only hosts).
    pub no_sync: bool,
}

/// Install both binaries. Returns the resolved versions.
pub fn install_all(opts: &InstallOptions) -> Result<Versions> {
    paths::ensure_dirs()?;
    let plat = Platform::detect()?;
    let proxy = resolve_proxy(opts.proxy);

    let mut versions = Versions::load().unwrap_or_default();

    let et = install_easytier(&plat, opts.et_version, proxy.as_deref())?;
    println!("  installed easytier {et}");
    versions.easytier = Some(et);

    if opts.no_sync {
        println!("  skipping syncthing (--no-sync)");
    } else {
        let st = install_syncthing(&plat, opts.st_version, proxy.as_deref())?;
        println!("  installed syncthing {st}");
        versions.syncthing = Some(st);
    }

    versions.save()?;
    verify_binaries()?;
    Ok(versions)
}

fn install_easytier(plat: &Platform, version: Option<&str>, proxy: Option<&str>) -> Result<String> {
    let rel = github::fetch_release(EASYTIER_REPO, version, proxy)?;
    let ver = github::version_from_tag(&rel.tag_name);
    let asset_name = plat.easytier_asset(&ver);
    let asset = github::find_asset(&rel, &asset_name)?;
    println!("  downloading {}", asset.name);
    let data = github::download(&asset.browser_download_url, proxy)?;
    extract::from_zip(&data, &["easytier-core", "easytier-cli"], &paths::bin_dir()?)
        .context("extracting easytier")?;
    Ok(ver)
}

fn install_syncthing(plat: &Platform, version: Option<&str>, proxy: Option<&str>) -> Result<String> {
    let rel = github::fetch_release(SYNCTHING_REPO, version, proxy)?;
    let ver = github::version_from_tag(&rel.tag_name);
    let asset_name = plat.syncthing_asset(&ver);
    let asset = github::find_asset(&rel, &asset_name)?;
    println!("  downloading {}", asset.name);
    let data = github::download(&asset.browser_download_url, proxy)?;

    // Optional integrity check against the published sha256sum list.
    if let Ok(sums) = github::find_asset(&rel, "sha256sum.txt.asc") {
        if let Ok(list) = github::download(&sums.browser_download_url, proxy) {
            verify_sha256(&data, &asset.name, &list);
        }
    }

    match plat.os {
        Os::Linux => extract::from_tar_gz(&data, &["syncthing"], &paths::bin_dir()?)?,
        Os::Macos => extract::from_zip(&data, &["syncthing"], &paths::bin_dir()?)?,
    };
    Ok(ver)
}

fn verify_sha256(data: &[u8], asset_name: &str, sumfile: &[u8]) {
    let want = String::from_utf8_lossy(sumfile);
    let actual = github::sha256_hex(data);
    for line in want.lines() {
        let line = line.trim();
        if line.ends_with(asset_name) {
            if let Some((hash, _)) = line.split_once(char::is_whitespace) {
                if hash.eq_ignore_ascii_case(&actual) {
                    println!("  sha256 ok: {asset_name}");
                } else {
                    eprintln!("  WARNING sha256 mismatch for {asset_name}");
                }
            }
            return;
        }
    }
}

/// Run `--version` on each installed binary to confirm it is runnable
/// (important on NixOS where dynamic binaries need nix-ld).
pub fn verify_binaries() -> Result<()> {
    for name in ["easytier-core", "syncthing"] {
        let bin = paths::binary(name)?;
        if !bin.exists() {
            continue;
        }
        let out = std::process::Command::new(&bin)
            .arg("--version")
            .output();
        match out {
            Ok(o) if o.status.success() => {}
            Ok(o) => eprintln!(
                "  WARNING {name} --version exited with {}: {}",
                o.status,
                String::from_utf8_lossy(&o.stderr).trim()
            ),
            Err(e) => eprintln!("  WARNING cannot run {name}: {e}"),
        }
    }
    Ok(())
}

/// Check latest available versions against installed ones.
pub fn check_updates(proxy: Option<&str>) -> Result<(Versions, Versions)> {
    let proxy = resolve_proxy(proxy);
    let installed = Versions::load().unwrap_or_default();
    let et = github::fetch_release(EASYTIER_REPO, None, proxy.as_deref())
        .map(|r| github::version_from_tag(&r.tag_name))
        .ok();
    let st = github::fetch_release(SYNCTHING_REPO, None, proxy.as_deref())
        .map(|r| github::version_from_tag(&r.tag_name))
        .ok();
    let latest = Versions {
        easytier: et,
        syncthing: st,
    };
    Ok((installed, latest))
}
