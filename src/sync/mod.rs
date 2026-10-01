//! Sync-layer abstraction. The default backend is Syncthing; the trait keeps
//! the door open for a future native-Rust sync backend without touching the
//! CLI or config.

pub mod syncthing;

use anyhow::Result;

#[derive(Debug, Clone)]
pub struct FolderSpec {
    pub id: String,
    pub label: String,
    pub path: String,
    pub devices: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FolderStatus {
    pub id: String,
    pub label: String,
    pub path: String,
    pub state: String,
    pub completion: f64,
}

#[derive(Debug, Clone)]
pub struct SyncStatus {
    pub device_id: String,
    pub folders: Vec<FolderStatus>,
    pub connected_devices: usize,
}

/// A pluggable synchronization backend.
pub trait SyncBackend {
    fn start(&self) -> Result<()>;
    fn stop(&self) -> Result<bool>;
    fn is_running(&self) -> bool;
    fn device_id(&self) -> Result<String>;
    fn add_device(&self, id: &str, addr: &str, name: &str) -> Result<()>;
    fn add_folder(&self, f: &FolderSpec) -> Result<()>;
    fn remove_folder(&self, id: &str) -> Result<()>;
    fn list_folders(&self) -> Result<Vec<FolderStatus>>;
    fn status(&self) -> Result<SyncStatus>;
}
