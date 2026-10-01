//! Installed-binary versions tracking (versions.json).

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Versions {
    #[serde(default)]
    pub easytier: Option<String>,
    #[serde(default)]
    pub syncthing: Option<String>,
}

impl Versions {
    pub fn load() -> Result<Self> {
        let p = paths::versions_file()?;
        if !p.exists() {
            return Ok(Versions::default());
        }
        let raw = std::fs::read_to_string(&p)?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn save(&self) -> Result<()> {
        let p = paths::versions_file()?;
        std::fs::write(&p, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}
