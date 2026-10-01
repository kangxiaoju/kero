//! Extract target binaries from downloaded zip / tar.gz archives.

use anyhow::{bail, Context, Result};
use std::io::{Cursor, Read};
use std::path::Path;

use crate::process;

/// Extract named members from a zip archive into `dest_dir`, matching by the
/// final path component. Returns the basenames actually written.
pub fn from_zip(data: &[u8], wanted: &[&str], dest_dir: &Path) -> Result<Vec<String>> {
    let mut zip = zip::ZipArchive::new(Cursor::new(data)).context("opening zip")?;
    let mut written = Vec::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i)?;
        if f.is_dir() {
            continue;
        }
        let full = f.name().to_string();
        let base = full.rsplit('/').next().unwrap_or(&full).to_string();
        if wanted.iter().any(|w| &base == w) {
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            let out = dest_dir.join(&base);
            std::fs::write(&out, &buf)
                .with_context(|| format!("writing {}", out.display()))?;
            process::make_executable(&out)?;
            written.push(base);
        }
    }
    if written.is_empty() {
        bail!("none of {:?} found in zip", wanted);
    }
    Ok(written)
}

/// Extract named members from a tar.gz archive into `dest_dir`.
pub fn from_tar_gz(data: &[u8], wanted: &[&str], dest_dir: &Path) -> Result<Vec<String>> {
    let gz = flate2::read::GzDecoder::new(Cursor::new(data));
    let mut tar = tar::Archive::new(gz);
    let mut written = Vec::new();
    for entry in tar.entries()? {
        let mut e = entry?;
        let path = e.path()?.to_path_buf();
        let base = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        if wanted.iter().any(|w| &base == w) {
            let mut buf = Vec::new();
            e.read_to_end(&mut buf)?;
            let out = dest_dir.join(&base);
            std::fs::write(&out, &buf)
                .with_context(|| format!("writing {}", out.display()))?;
            process::make_executable(&out)?;
            written.push(base);
        }
    }
    if written.is_empty() {
        bail!("none of {:?} found in tar.gz", wanted);
    }
    Ok(written)
}
