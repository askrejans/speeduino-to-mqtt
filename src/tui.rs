//! Terminal User Interface
//!
//! Provides a live TUI (using ratatui + crossterm) when the application is run
//! interactively (TTY detected).  In service/daemon mode the TUI is skipped and
//! structured logs are written to stdout instead.
//!
//! # Layout
//! ```text
//! ┌─────────────────── Speeduino-to-MQTT ────────────────────┐
//! │ CONNECTIONS         │ ECU DATA                           │
//! │ ECU: ● ONLINE       │  RPM: 3000  MAP: 98 kPa           │
//! │ …                   │  …                                 │
//! ├─────────────────────────────────────────────────────────-┤
//! │ LOG                                                       │
//! │ [INFO] Connected …                                        │
//! └───────────────────────────────────────────────────────────┘
//! ```

use crate::ecu_data_parser::SpeeduinoData;
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::StreamExt;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph, Wrap},
};
use std::{
    collections::VecDeque,
    io::{self, Write},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::RwLock;
use tokio::time::interval;
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

/// Shared state updated from the ECU/MQTT tasks and rendered by the TUI.
#[derive(Default)]
pub struct TuiState {
    pub ecu_connected: bool,
    pub mqtt_connected: bool,
    pub mqtt_enabled: bool,
    pub connection_address: String,
    pub mqtt_address: String,
    pub ecu_data: Option<SpeeduinoData>,
    pub messages_published: u64,
}

// ---------------------------------------------------------------------------
// Log writer that captures tracing output into the TUI log panel
// ---------------------------------------------------------------------------

/// An `io::Write` implementation that appends formatted log lines to a
/// shared ring-buffer, which the TUI renders in the bottom log panel.
#[derive(Clone)]
pub struct TuiWriter {
    buffer: Arc<Mutex<VecDeque<String>>>,
}

impl TuiWriter {
    pub fn new(buffer: Arc<Mutex<VecDeque<String>>>) -> Self {
        Self { buffer }
    }
}

impl Write for TuiWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Ok(s) = std::str::from_utf8(buf) {
            let trimmed = s.trim_end_matches('\n');
            if !trimmed.is_empty() {
                let mut guard = self.buffer.lock().unwrap();
                if guard.len() >= 200 {
                    guard.pop_front();
                }
                guard.push_back(trimmed.to_string());
            }
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for TuiWriter {
    type Writer = TuiWriter;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

// ---------------------------------------------------------------------------
// TUI runner
// ---------------------------------------------------------------------------

/// Run the interactive TUI until the user presses `q` / `Ctrl+C`, or until
/// `cancel` fires (e.g. on SIGTERM).
///
/// Rendering happens every 100 ms; new ECU data and log entries are reflected
/// on the next frame.
pub async fn run_tui(
    state: Arc<RwLock<TuiState>>,
    log_buffer: Arc<Mutex<VecDeque<String>>>,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = tui_loop(&mut terminal, state, log_buffer, cancel).await;

    // Restore terminal regardless of outcome
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn tui_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: Arc<RwLock<TuiState>>,
    log_buffer: Arc<Mutex<VecDeque<String>>>,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    let mut event_stream = EventStream::new();
    let mut render_tick = interval(Duration::from_millis(100));

    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,

            _ = render_tick.tick() => {
                let s = state.read().await;
                let logs: Vec<String> = log_buffer.lock().unwrap().iter().cloned().collect();
                let snap = StateSnapshot {
                    ecu_connected: s.ecu_connected,
                    mqtt_connected: s.mqtt_connected,
                    mqtt_enabled: s.mqtt_enabled,
                    connection_address: s.connection_address.clone(),
                    mqtt_address: s.mqtt_address.clone(),
                    ecu_data: s.ecu_data.clone(),
                    messages_published: s.messages_published,
                    logs,
                };
                drop(s);
                terminal.draw(|f| render(f, &snap))?;
            }

            Some(Ok(event)) = event_stream.next() => {
                if should_quit(&event) {
                    cancel.cancel();
                    break;
                }
            }
        }
    }
    Ok(())
}

fn should_quit(event: &Event) -> bool {
    matches!(
        event,
        Event::Key(k)
            if k.code == KeyCode::Char('q')
            || (k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL))
    )
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

struct StateSnapshot {
    ecu_connected: bool,
    mqtt_connected: bool,
    mqtt_enabled: bool,
    connection_address: String,
    mqtt_address: String,
    ecu_data: Option<SpeeduinoData>,
    messages_published: u64,
    logs: Vec<String>,
}

fn render(f: &mut Frame, snap: &StateSnapshot) {
    let area = f.area();

    // Vertical split: header | main | log
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(8),
        ])
        .split(area);

    let header_area = vertical[0];
    let main_area = vertical[1];
    let log_area = vertical[2];

    // Horizontal split: connections | data
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(28), Constraint::Min(1)])
        .split(main_area);

    let conn_area = horizontal[0];
    let data_area = horizontal[1];

    render_header(f, header_area);
    render_connections(f, conn_area, snap);
    render_ecu_data(f, data_area, snap);
    render_log(f, log_area, snap);
}

fn render_header(f: &mut Frame, area: Rect) {
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            concat!(" Speeduino-to-MQTT v", env!("CARGO_PKG_VERSION"), " "),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" │ press "),
        Span::styled(
            "q",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" to quit"),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, area);
}

fn status_indicator(connected: bool) -> Span<'static> {
    if connected {
        Span::styled("● ONLINE ", Style::default().fg(Color::Green))
    } else {
        Span::styled("○ OFFLINE", Style::default().fg(Color::Red))
    }
}

fn render_connections(f: &mut Frame, area: Rect, snap: &StateSnapshot) {
    let mut lines: Vec<Line> = Vec::new();

    // ECU connection
    lines.push(Line::from(vec![
        Span::styled("ECU:  ", Style::default().add_modifier(Modifier::BOLD)),
        status_indicator(snap.ecu_connected),
    ]));
    if !snap.connection_address.is_empty() {
        lines.push(Line::from(Span::raw(format!(
            "  {}",
            snap.connection_address
        ))));
    }
    lines.push(Line::default());

    // MQTT connection
    lines.push(Line::from(vec![
        Span::styled("MQTT: ", Style::default().add_modifier(Modifier::BOLD)),
        if snap.mqtt_enabled {
            status_indicator(snap.mqtt_connected)
        } else {
            Span::styled("DISABLED ", Style::default().fg(Color::DarkGray))
        },
    ]));
    if snap.mqtt_enabled && !snap.mqtt_address.is_empty() {
        lines.push(Line::from(Span::raw(format!("  {}", snap.mqtt_address))));
    }
    lines.push(Line::default());

    // Published count
    if snap.mqtt_enabled {
        lines.push(Line::from(vec![
            Span::styled("Msgs: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(snap.messages_published.to_string()),
        ]));
    }

    let block = Block::default()
        .title(" CONNECTIONS ")
        .borders(Borders::ALL)
        .padding(Padding::horizontal(1));
    let para = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
    f.render_widget(para, area);
}

fn render_ecu_data(f: &mut Frame, area: Rect, snap: &StateSnapshot) {
    let block = Block::default()
        .title(" ECU DATA ")
        .borders(Borders::ALL)
        .padding(Padding::horizontal(1));

    let Some(ref d) = snap.ecu_data else {
        let para = Paragraph::new("Waiting for ECU data…")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(para, area);
        return;
    };

    // ── Derived values ────────────────────────────────────────────────────
    // Gauge boost pressure: positive = boost, negative = vacuum
    let boost_rel = d.map as i32 - d.baro as i32;
    // Lambda from AFR target (afr_target stored ×10, stoich 14.7 → 147)
    let afr_lambda = d.afr_target as f32 / 147.0;
    // Dwell efficiency: actual measured vs requested dwell (%)
    let dwell_eff = d
        .actual_dwell
        .filter(|_| d.dwell > 0)
        .map(|ad| ad as f32 / d.dwell as f32 * 100.0);

    // ── Layout helpers ────────────────────────────────────────────────────
    // inner_w: usable width inside borders (2) + padding (2)
    let inner_w = area.width.saturating_sub(4) as usize;
    let col_w = (inner_w / 3).max(10);
    // Width reserved per value in non-last columns: label(4) + ": "(2) + val_w + "  "(2) = col_w
    let val_w = col_w.saturating_sub(8);

    let lbl = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let sec = Style::default().fg(Color::DarkGray);

    // Nested helpers (no closures needed — pure conversions)
    fn c(label: &str, value: String) -> (String, String) {
        (label.to_string(), value)
    }
    fn ms10(raw: u16) -> String {
        format!("{:.1}ms", raw as f32 / 10.0)
    }
    fn opt_ms10(o: Option<u16>) -> String {
        o.map_or_else(|| "—".into(), |v| ms10(v))
    }
    fn opt_str<T: std::fmt::Display>(o: Option<T>) -> String {
        o.map_or_else(|| "—".into(), |v| v.to_string())
    }
    fn opt_unit<T: std::fmt::Display>(o: Option<T>, unit: &str) -> String {
        o.map_or_else(|| "—".into(), |v| format!("{}{}", v, unit))
    }

    let section_line = |title: &str| -> Line<'static> {
        Line::from(Span::styled(format!("── {} ──", title), sec))
    };

    let row = |cells: Vec<(String, String)>| -> Line<'static> {
        let n = cells.len();
        let mut spans: Vec<Span<'static>> = Vec::new();
        for (i, (label, value)) in cells.into_iter().enumerate() {
            spans.push(Span::styled(format!("{:<4}", label), lbl));
            let part = if i + 1 < n {
                format!(": {:<width$}  ", value, width = val_w)
            } else {
                format!(": {}", value)
            };
            spans.push(Span::raw(part));
        }
        Line::from(spans)
    };

    let mut lines: Vec<Line> = Vec::new();

    // ── ENGINE ───────────────────────────────────────────────────────────
    lines.push(section_line("ENGINE"));
    lines.push(row(vec![
        c("RPM", d.rpm.to_string()),
        c("MAP", format!("{} kPa", d.map)),
        c("TPS", format!("{}%", d.tps)),
    ]));
    lines.push(row(vec![
        c("DRPM", format!("{:+}/s", d.rpm_dot)),
        c("DMAP", format!("{:.1} kPa/s", d.map_dot as f32 / 10.0)),
        c("DTPS", format!("{:.1}%/s", d.tps_dot as f32 / 10.0)),
    ]));
    lines.push(row(vec![
        c("ADV", format!("{}°", d.advance)),
        c("VE", format!("{}%", d.ve_current)),
        c(
            "AFR>",
            format!("{:.1} λ{:.2}", d.afr_target_real(), afr_lambda),
        ),
    ]));
    lines.push(row(vec![
        c("BARO", format!("{} kPa", d.baro)),
        c(
            "EMAP",
            d.emap.map_or_else(|| "—".into(), |v| format!("{} kPa", v)),
        ),
        c("GBST", format!("{:+} kPa", boost_rel)),
    ]));

    // ── SENSORS ──────────────────────────────────────────────────────────
    lines.push(section_line("SENSORS"));
    lines.push(row(vec![
        c("IAT", format!("{}°C", d.iat_celsius())),
        c("CLT", format!("{}°C", d.coolant_celsius())),
        c("FTMP", format!("{}°C", d.fuel_temp_celsius())),
    ]));
    lines.push(row(vec![
        c("BAT", format!("{:.1}V", d.battery_voltage())),
        c("O2P", d.o2_primary.to_string()),
        c("O2S", d.o2_secondary.to_string()),
    ]));

    // ── FUELING ──────────────────────────────────────────────────────────
    lines.push(section_line("FUELING"));
    lines.push(row(vec![
        c("PW1", ms10(d.pw1)),
        c("PW2", ms10(d.pw2)),
        c("PW3", ms10(d.pw3)),
    ]));
    lines.push(row(vec![
        c("PW4", ms10(d.pw4)),
        c("EGO", format!("{}%", d.ego_correction)),
        c("TAE", format!("{}%", d.tae_amount_pct())),
    ]));
    if d.pw5.is_some() || d.pw6.is_some() {
        lines.push(row(vec![
            c("PW5", opt_ms10(d.pw5)),
            c("PW6", opt_ms10(d.pw6)),
            c("PW7", opt_ms10(d.pw7)),
        ]));
        lines.push(row(vec![c("PW8", opt_ms10(d.pw8))]));
    }
    lines.push(row(vec![
        c("IATC", format!("{}%", d.iat_correction)),
        c("WUEC", format!("{}%", d.wue_correction)),
        c("BARC", format!("{}%", d.baro_correction)),
    ]));
    lines.push(row(vec![
        c("BATC", format!("{}%", d.bat_correction)),
        c("FTEC", format!("{}%", d.fuel_temp_correction)),
        c("CORR", format!("{}%", d.corrections)),
    ]));
    lines.push(row(vec![
        c("ASE", format!("{}%", d.ase_value)),
        c("FLXC", format!("{}%", d.flex_correction)),
    ]));

    // ── IGNITION ─────────────────────────────────────────────────────────
    lines.push(section_line("IGNITION"));
    lines.push(row(vec![
        c("DWL", ms10(d.dwell)),
        c("ADW", opt_ms10(d.actual_dwell)),
        c(
            "DEFF",
            dwell_eff.map_or_else(|| "—".into(), |e| format!("{:.0}%", e)),
        ),
    ]));
    lines.push(row(vec![
        c("ADV1", format!("{}°", d.advance1)),
        c("ADV2", format!("{}°", d.advance2)),
        c("KNK", opt_str(d.knock_count)),
    ]));
    lines.push(row(vec![c("KRET", opt_unit(d.knock_retard, "°"))]));

    // ── BOOST / VVT / FLEX ───────────────────────────────────────────────
    lines.push(section_line("BOOST / VVT / FLEX"));
    lines.push(row(vec![
        // boost_duty_raw stores duty as 0-100 (= actual %)
        c("BTGT", format!("{} kPa", d.boost_target_kpa())),
        c("BDUT", format!("{}%", d.boost_duty_raw)),
        c("ETH", format!("{}%", d.ethanol_pct)),
    ]));
    lines.push(row(vec![
        c("VVT1", format!("{}°", d.vvt1_angle)),
        c("VT1T", format!("{}°", d.vvt1_target_angle)),
        c("VT1D", format!("{}%", d.vvt1_duty)),
    ]));
    lines.push(row(vec![
        c("VVT2", format!("{}°", d.vvt2_angle)),
        c("VT2T", format!("{}°", d.vvt2_target_angle)),
        c("VT2D", format!("{}%", d.vvt2_duty)),
    ]));
    lines.push(row(vec![
        c("FLXI", format!("{}°", d.flex_ign_correction)),
        c("FLXB", d.flex_boost_correction.to_string()),
    ]));

    // ── VEHICLE ──────────────────────────────────────────────────────────
    lines.push(section_line("VEHICLE"));
    lines.push(row(vec![
        c("VSS", format!("{} km/h", d.vss)),
        c(
            "GEAR",
            if d.gear == 0 {
                "N".into()
            } else {
                d.gear.to_string()
            },
        ),
        c("WMI", format!("{} µs", d.wmi_pw)),
    ]));
    lines.push(row(vec![
        c("OIL", format!("{} kPa", d.oil_pressure)),
        c("FPRS", format!("{} kPa", d.fuel_pressure)),
    ]));
    lines.push(row(vec![
        c("FAN", opt_unit(d.fan_duty, "%")),
        c(
            "ACS",
            d.air_con_status
                .map_or_else(|| "—".into(), |v| format!("{:#04x}", v)),
        ),
    ]));

    // ── SYSTEM ───────────────────────────────────────────────────────────
    lines.push(section_line("SYSTEM"));
    lines.push(row(vec![
        c("LPS", d.loops_per_second.to_string()),
        c("RAM", format!("{} B", d.free_ram)),
        c("SECL", d.secl.to_string()),
    ]));
    lines.push(row(vec![
        c("SYNC", d.sync_loss_counter.to_string()),
        c("ERR", format!("{:#04x}", d.next_error)),
        c("SDCS", format!("{:#04x}", d.ts_sd_status)),
    ]));
    lines.push(row(vec![
        c("LOAD", d.fuel_load.to_string()),
        c("IGLD", d.ign_load.to_string()),
        c("CILT", d.cl_idle_target.to_string()),
    ]));
    lines.push(row(vec![
        c("STS1", format!("{:#04x}", d.status1)),
        c("ENG", format!("{:#04x}", d.engine)),
        c("SPRK", format!("{:#04x}", d.spark)),
    ]));
    lines.push(row(vec![
        c("STS3", format!("{:#04x}", d.status3)),
        c("STS4", format!("{:#04x}", d.status4)),
        c("STS5", opt_str(d.status5.map(|v| format!("{:#04x}", v)))),
    ]));

    // ── CAN INPUTS (only shown when any channel is non-zero) ─────────────
    if d.canin.iter().any(|&v| v != 0) {
        lines.push(section_line("CAN INPUTS"));
        for i in (0..16usize).step_by(3) {
            let end = (i + 3).min(16);
            let cells: Vec<(String, String)> = (i..end)
                .map(|j| (format!("C{:<2}", j), d.canin[j].to_string()))
                .collect();
            lines.push(row(cells));
        }
    }

    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}

fn render_log(f: &mut Frame, area: Rect, snap: &StateSnapshot) {
    let max_lines = (area.height.saturating_sub(2)) as usize;
    let start = snap.logs.len().saturating_sub(max_lines);
    let items: Vec<ListItem> = snap.logs[start..]
        .iter()
        .map(|line| {
            let style = if line.contains("ERROR") {
                Style::default().fg(Color::Red)
            } else if line.contains("WARN") {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Gray)
            };
            ListItem::new(Line::from(Span::styled(line.clone(), style)))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" LOG (most recent) ")
            .borders(Borders::ALL),
    );
    f.render_widget(list, area);
}
