//! Lightweight terminal management UI (ratatui). Read-only dashboard that
//! refreshes mesh + sync status; q to quit, r to refresh now.

use anyhow::Result;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table};

use crate::config::Config;
use crate::net;
use crate::paths;
use crate::sync::syncthing::Syncthing;
use crate::sync::{SyncBackend, SyncStatus};

struct Snapshot {
    net_up: bool,
    sync_up: bool,
    peers: String,
    sync: Option<SyncStatus>,
}

fn gather(cfg: &Config) -> Snapshot {
    let net_up = net::is_running();
    let st = cfg.sync.as_ref().map(Syncthing::from_config);
    let sync_up = st.as_ref().map(|s| s.is_running()).unwrap_or(false);
    Snapshot {
        net_up,
        sync_up,
        peers: if net_up {
            net::peers(cfg).unwrap_or_else(|e| format!("(error: {e})"))
        } else {
            "(not running)".into()
        },
        sync: if sync_up {
            st.and_then(|s| s.status().ok())
        } else {
            None
        },
    }
}

pub fn run() -> Result<()> {
    let p = paths::config_file()?;
    if !p.exists() {
        anyhow::bail!("no config at {}. run `kero init` first", p.display());
    }
    let cfg = Config::load(&p)?;

    let mut terminal = ratatui::init();
    let res = event_loop(&mut terminal, &cfg);
    ratatui::restore();
    res
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, cfg: &Config) -> Result<()> {
    let mut snap = gather(cfg);
    let mut last = Instant::now();
    loop {
        terminal.draw(|f| draw(f, &snap))?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(k) = event::read()? {
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('r') => {
                        snap = gather(cfg);
                        last = Instant::now();
                    }
                    _ => {}
                }
            }
        }
        // Auto-refresh every 3s.
        if last.elapsed() >= Duration::from_secs(3) {
            snap = gather(cfg);
            last = Instant::now();
        }
    }
    Ok(())
}

fn draw(f: &mut Frame, snap: &Snapshot) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(10),
            Constraint::Length(1),
        ])
        .split(f.area());

    // Header.
    let header = format!(
        " kero   mesh: {}   sync: {} ",
        status_word(snap.net_up),
        status_word(snap.sync_up),
    );
    f.render_widget(
        Paragraph::new(header).block(Block::default().borders(Borders::ALL).title("status")),
        chunks[0],
    );

    // Sync folders table.
    let rows: Vec<Row> = match &snap.sync {
        Some(s) => s
            .folders
            .iter()
            .map(|fo| {
                Row::new(vec![
                    fo.id.clone(),
                    fo.state.clone(),
                    format!("{:.1}%", fo.completion),
                    fo.path.clone(),
                ])
            })
            .collect(),
        None => vec![Row::new(vec!["(sync not running)".to_string()])],
    };
    let dev_line = snap
        .sync
        .as_ref()
        .map(|s| format!("sync folders  (device {} · peers {})", short(&s.device_id), s.connected_devices))
        .unwrap_or_else(|| "sync folders".into());
    let table = Table::new(
        rows,
        [
            Constraint::Length(18),
            Constraint::Length(14),
            Constraint::Length(8),
            Constraint::Min(10),
        ],
    )
    .header(Row::new(vec!["id", "state", "done", "path"]).style(Style::new().bold()))
    .block(Block::default().borders(Borders::ALL).title(dev_line));
    f.render_widget(table, chunks[1]);

    // Mesh peers (raw easytier-cli text).
    f.render_widget(
        Paragraph::new(snap.peers.clone())
            .block(Block::default().borders(Borders::ALL).title("mesh peers")),
        chunks[2],
    );

    // Footer.
    f.render_widget(
        Paragraph::new(" q quit   r refresh   (auto every 3s) ")
            .style(Style::new().dim()),
        chunks[3],
    );
}

fn status_word(up: bool) -> &'static str {
    if up {
        "UP"
    } else {
        "down"
    }
}

fn short(id: &str) -> String {
    id.chars().take(7).collect()
}
