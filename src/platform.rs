//! OS/arch detection and GitHub release asset-name matching.

use anyhow::{bail, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    Macos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86_64,
    Aarch64,
}

#[derive(Debug, Clone, Copy)]
pub struct Platform {
    pub os: Os,
    pub arch: Arch,
}

impl Platform {
    pub fn detect() -> Result<Self> {
        let os = match std::env::consts::OS {
            "linux" => Os::Linux,
            "macos" => Os::Macos,
            other => bail!("unsupported OS: {other} (kero supports linux and macos)"),
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => Arch::X86_64,
            "aarch64" => Arch::Aarch64,
            other => bail!("unsupported arch: {other} (kero supports x86_64 and aarch64)"),
        };
        Ok(Self { os, arch })
    }

    /// EasyTier release asset name, e.g. `easytier-linux-x86_64-v2.6.4.zip`.
    pub fn easytier_asset(&self, version: &str) -> String {
        let tag = match (self.os, self.arch) {
            (Os::Linux, Arch::X86_64) => "linux-x86_64",
            (Os::Linux, Arch::Aarch64) => "linux-aarch64",
            (Os::Macos, Arch::X86_64) => "macos-x86_64",
            (Os::Macos, Arch::Aarch64) => "macos-aarch64",
        };
        format!("easytier-{tag}-v{version}.zip")
    }

    /// Syncthing release asset name, e.g. `syncthing-linux-amd64-v2.1.5.tar.gz`.
    pub fn syncthing_asset(&self, version: &str) -> String {
        match (self.os, self.arch) {
            (Os::Linux, Arch::X86_64) => format!("syncthing-linux-amd64-v{version}.tar.gz"),
            (Os::Linux, Arch::Aarch64) => format!("syncthing-linux-arm64-v{version}.tar.gz"),
            (Os::Macos, Arch::X86_64) => format!("syncthing-macos-amd64-v{version}.zip"),
            (Os::Macos, Arch::Aarch64) => format!("syncthing-macos-arm64-v{version}.zip"),
        }
    }
}
