//! Command-line interface definition.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "kero",
    version,
    about = "Unified CLI for EasyTier (mesh networking) + Syncthing (decentralized sync)"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Download and install easytier + syncthing into ~/.kero/bin.
    Install {
        /// Proxy URL (e.g. http://127.0.0.1:7890). Falls back to HTTPS_PROXY.
        #[arg(long)]
        proxy: Option<String>,
        /// Pin an EasyTier version (default: latest).
        #[arg(long)]
        et_version: Option<String>,
        /// Pin a Syncthing version (default: latest).
        #[arg(long)]
        st_version: Option<String>,
        /// Install easytier only, skip syncthing (easytier-only hosts).
        #[arg(long)]
        no_sync: bool,
    },
    /// Generate a default kero.toml and a Syncthing identity.
    Init {
        /// Overwrite an existing kero.toml.
        #[arg(long)]
        force: bool,
    },
    /// Start easytier then syncthing.
    Up {
        /// Start only the sync layer (reuse an existing mesh).
        #[arg(long)]
        no_net: bool,
    },
    /// Stop syncthing then easytier.
    Down,
    /// Show combined mesh + sync status.
    Status,
    /// Network subcommands.
    Net {
        #[command(subcommand)]
        cmd: NetCmd,
    },
    /// Sync-folder subcommands.
    Sync {
        #[command(subcommand)]
        cmd: SyncCmd,
    },
    /// Peer-device subcommands.
    Device {
        #[command(subcommand)]
        cmd: DeviceCmd,
    },
    /// Tail managed process logs.
    Logs {
        /// Which process: net|sync.
        #[arg(default_value = "sync")]
        which: String,
        #[arg(long, default_value_t = 40)]
        lines: usize,
    },
    /// Check or apply binary upgrades.
    Upgrade {
        /// Only report available updates.
        #[arg(long)]
        check: bool,
        #[arg(long)]
        proxy: Option<String>,
    },
    /// Launch the interactive TUI.
    Tui,
    /// Run the REST API daemon (foreground).
    Serve,
}

#[derive(Subcommand)]
pub enum NetCmd {
    /// List mesh peers (via easytier-cli).
    Peers,
    /// Show this node's mesh info.
    Info,
}

#[derive(Subcommand)]
pub enum SyncCmd {
    /// Add a folder to sync.
    Add {
        path: String,
        #[arg(long)]
        id: Option<String>,
        /// Device IDs to share with (repeatable).
        #[arg(long = "device")]
        devices: Vec<String>,
    },
    /// List sync folders and progress.
    Ls,
    /// Remove a folder by id.
    Rm { id: String },
}

#[derive(Subcommand)]
pub enum DeviceCmd {
    /// Add a peer device.
    Add {
        id: String,
        address: String,
        #[arg(long)]
        name: Option<String>,
    },
    /// Print this node's Syncthing device id.
    Id,
}
