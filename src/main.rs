mod api;
mod cli;
mod config;
mod install;
mod net;
mod paths;
mod platform;
mod process;
mod state;
mod sync;
mod tui;

use anyhow::{Context, Result};
use clap::Parser;

use cli::{Cli, Command, DeviceCmd, NetCmd, SyncCmd};
use config::Config;
use sync::syncthing::Syncthing;
use sync::{FolderSpec, SyncBackend};

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = Cli::parse();
    match args.command {
        Command::Install {
            proxy,
            et_version,
            st_version,
            no_sync,
        } => cmd_install(proxy, et_version, st_version, no_sync),
        Command::Init { force } => cmd_init(force),
        Command::Up { no_net } => cmd_up(no_net),
        Command::Down => cmd_down(),
        Command::Status => cmd_status(),
        Command::Net { cmd } => cmd_net(cmd),
        Command::Sync { cmd } => cmd_sync(cmd),
        Command::Device { cmd } => cmd_device(cmd),
        Command::Logs { which, lines } => cmd_logs(&which, lines),
        Command::Upgrade { check, proxy } => cmd_upgrade(check, proxy),
        Command::Tui => tui::run(),
        Command::Serve => api::serve(load_config()?),
    }
}

fn load_config() -> Result<Config> {
    let p = paths::config_file()?;
    if !p.exists() {
        anyhow::bail!("no config found at {}. run `kero init` first", p.display());
    }
    Config::load(&p)
}

/// Build a Syncthing backend if sync is configured, else None.
fn syncthing(cfg: &Config) -> Option<Syncthing> {
    cfg.sync.as_ref().map(Syncthing::from_config)
}

/// Require a configured Syncthing backend, erroring on easytier-only hosts.
fn require_sync(cfg: &Config) -> Result<Syncthing> {
    syncthing(cfg).ok_or_else(|| anyhow::anyhow!("sync is disabled on this host (no [sync] section)"))
}

fn cmd_install(
    proxy: Option<String>,
    et_version: Option<String>,
    st_version: Option<String>,
    no_sync: bool,
) -> Result<()> {
    println!("installing managed binaries into {}", paths::bin_dir()?.display());
    let opts = install::InstallOptions {
        proxy: proxy.as_deref(),
        et_version: et_version.as_deref(),
        st_version: st_version.as_deref(),
        no_sync,
    };
    install::install_all(&opts)?;
    println!("done.");
    Ok(())
}

fn cmd_init(force: bool) -> Result<()> {
    paths::ensure_dirs()?;
    let p = paths::config_file()?;
    if p.exists() && !force {
        anyhow::bail!("{} already exists (use --force to overwrite)", p.display());
    }
    Config::sample().save(&p)?;
    println!("wrote sample config to {}", p.display());

    // Pre-generate the Syncthing identity if sync is enabled and binary present.
    let cfg = Config::load(&p)?;
    if cfg.sync.is_some() && paths::binary("syncthing")?.exists() {
        if let Some(st) = syncthing(&cfg) {
            st.ensure_generated()?;
            println!("generated syncthing identity under {}", paths::syncthing_home()?.display());
        }
    }
    println!("edit {} then run `kero up`", p.display());
    Ok(())
}

fn cmd_up(no_net: bool) -> Result<()> {
    let cfg = load_config()?;
    if no_net {
        println!("skipping easytier (--no-net)");
    } else if !net::is_running() {
        let pid = net::start(&cfg).context("starting easytier")?;
        println!("easytier started (pid {pid})");
    } else {
        println!("easytier already running");
    }
    match syncthing(&cfg) {
        None => println!("sync disabled (easytier-only host)"),
        Some(st) => {
            if !st.is_running() {
                st.start().context("starting syncthing")?;
                println!("syncthing started");
            } else {
                println!("syncthing already running");
            }
            let sync = cfg.sync.as_ref().unwrap();
            // Reconcile declared devices and folders from config.
            for d in &sync.device {
                let name = if d.name.is_empty() { &d.id } else { &d.name };
                if let Err(e) = st.add_device(&d.id, &d.address, name) {
                    eprintln!("  warn: add_device {}: {e}", d.id);
                }
            }
            for f in &sync.folder {
                let spec = FolderSpec {
                    id: f.id.clone(),
                    label: f.label.clone(),
                    path: f.path.clone(),
                    devices: f.devices.clone(),
                };
                if let Err(e) = st.add_folder(&spec) {
                    eprintln!("  warn: add_folder {}: {e}", f.id);
                }
            }
        }
    }
    println!("up.");
    Ok(())
}

fn cmd_down() -> Result<()> {
    let cfg = load_config().ok();
    if let Some(cfg) = &cfg {
        if let Some(st) = syncthing(cfg) {
            if st.stop()? {
                println!("syncthing stopped");
            }
        }
    } else if process::is_running(sync::syncthing::PROC) {
        process::stop(sync::syncthing::PROC)?;
        println!("syncthing stopped");
    }
    if net::stop()? {
        println!("easytier stopped");
    }
    println!("down.");
    Ok(())
}

fn cmd_status() -> Result<()> {
    let cfg = load_config()?;
    println!("== mesh (easytier) ==");
    println!("  running: {}", net::is_running());
    if net::is_running() {
        match net::peers(&cfg) {
            Ok(txt) => println!("{txt}"),
            Err(e) => println!("  (peer query failed: {e})"),
        }
    }
    println!("== sync (syncthing) ==");
    match syncthing(&cfg) {
        None => println!("  disabled (easytier-only host)"),
        Some(st) => {
            println!("  running: {}", st.is_running());
            if st.is_running() {
                match st.status() {
                    Ok(s) => {
                        println!("  device id: {}", s.device_id);
                        println!("  connected devices: {}", s.connected_devices);
                        for f in s.folders {
                            println!(
                                "  folder {:<16} {:<12} {:>6.1}%  {}",
                                f.id, f.state, f.completion, f.path
                            );
                        }
                    }
                    Err(e) => println!("  (status query failed: {e})"),
                }
            }
        }
    }
    Ok(())
}

fn cmd_net(cmd: NetCmd) -> Result<()> {
    let cfg = load_config()?;
    match cmd {
        NetCmd::Peers => println!("{}", net::peers(&cfg)?),
        NetCmd::Info => println!("{}", net::cli(&cfg, &["node"])?),
    }
    Ok(())
}

fn cmd_sync(cmd: SyncCmd) -> Result<()> {
    let cfg = load_config()?;
    let st = require_sync(&cfg)?;
    match cmd {
        SyncCmd::Add { path, id, devices } => {
            let id = id.unwrap_or_else(|| default_folder_id(&path));
            let spec = FolderSpec {
                id: id.clone(),
                label: id.clone(),
                path: path.clone(),
                devices,
            };
            std::fs::create_dir_all(&path).ok();
            st.add_folder(&spec)?;
            println!("added folder {id} -> {path}");
        }
        SyncCmd::Ls => {
            for f in st.list_folders()? {
                println!(
                    "{:<16} {:<12} {:>6.1}%  {}",
                    f.id, f.state, f.completion, f.path
                );
            }
        }
        SyncCmd::Rm { id } => {
            st.remove_folder(&id)?;
            println!("removed folder {id}");
        }
    }
    Ok(())
}

fn cmd_device(cmd: DeviceCmd) -> Result<()> {
    let cfg = load_config()?;
    let st = require_sync(&cfg)?;
    match cmd {
        DeviceCmd::Add { id, address, name } => {
            let name = name.unwrap_or_else(|| id.clone());
            st.add_device(&id, &address, &name)?;
            println!("added device {id} ({address})");
        }
        DeviceCmd::Id => println!("{}", st.device_id()?),
    }
    Ok(())
}

fn cmd_logs(which: &str, lines: usize) -> Result<()> {
    let name = match which {
        "net" | "easytier" => net::PROC,
        _ => sync::syncthing::PROC,
    };
    println!("{}", process::tail_log(name, lines)?);
    Ok(())
}

fn cmd_upgrade(check: bool, proxy: Option<String>) -> Result<()> {
    let (installed, latest) = install::check_updates(proxy.as_deref())?;
    println!(
        "easytier:  installed {:<12} latest {}",
        installed.easytier.clone().unwrap_or_else(|| "-".into()),
        latest.easytier.clone().unwrap_or_else(|| "?".into())
    );
    println!(
        "syncthing: installed {:<12} latest {}",
        installed.syncthing.clone().unwrap_or_else(|| "-".into()),
        latest.syncthing.clone().unwrap_or_else(|| "?".into())
    );
    if check {
        return Ok(());
    }

    let need = installed.easytier != latest.easytier || installed.syncthing != latest.syncthing;
    if !need {
        println!("already up to date.");
        return Ok(());
    }

    // Stop services, reinstall latest, (re)start if config exists.
    let was_up = net::is_running() || process::is_running(sync::syncthing::PROC);
    if was_up {
        println!("stopping services for upgrade...");
        let _ = cmd_down();
    }
    let opts = install::InstallOptions {
        proxy: proxy.as_deref(),
        et_version: None,
        st_version: None,
        // Preserve easytier-only installs: only upgrade syncthing if present.
        no_sync: !paths::binary("syncthing")?.exists(),
    };
    install::install_all(&opts)?;
    if was_up {
        println!("restarting services...");
        cmd_up(false)?;
    }
    println!("upgrade complete.");
    Ok(())
}

fn default_folder_id(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("folder")
        .to_string()
}
