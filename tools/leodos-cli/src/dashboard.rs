use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen,
    LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use leodos_protocols::network::spp::SpacePacket;
use ratatui::prelude::*;
use ratatui::widgets::*;
use std::collections::VecDeque;
use std::io::stdout;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::UdpSocket;

const MAX_LOG_LINES: usize = 500;

#[derive(Clone)]
struct SatInfo {
    orb: u8,
    sat: u8,
    lat: f32,
    lon: f32,
    alt: f32,
    cmd_count: u16,
    err_count: u16,
    last_seen: String,
}

struct AppState {
    tab: usize,
    logs: VecDeque<String>,
    satellites: Vec<SatInfo>,
    orbits: u8,
    sats: u8,
}

impl AppState {
    fn new(orbits: u8, sats: u8) -> Self {
        let mut satellites = Vec::new();
        for o in 0..orbits {
            for s in 0..sats {
                satellites.push(SatInfo {
                    orb: o,
                    sat: s,
                    lat: 0.0,
                    lon: 0.0,
                    alt: 550.0,
                    cmd_count: 0,
                    err_count: 0,
                    last_seen: "—".into(),
                });
            }
        }
        Self {
            tab: 0,
            logs: VecDeque::new(),
            satellites,
            orbits,
            sats,
        }
    }

    fn push_log(&mut self, line: String) {
        self.logs.push_back(line);
        if self.logs.len() > MAX_LOG_LINES {
            self.logs.pop_front();
        }
    }
}

fn extract_cstr(data: &[u8]) -> &str {
    let end = data
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(data.len());
    core::str::from_utf8(&data[..end]).unwrap_or("")
}

fn process_packet(state: &mut AppState, raw: &[u8]) {
    let Ok(pkt) = SpacePacket::parse(raw) else {
        return;
    };

    let payload = pkt.data_field();
    if payload.len() < 26 {
        return;
    }

    let app_name = extract_cstr(&payload[..20]);
    let msg_offset = 24;
    if msg_offset >= payload.len() {
        return;
    }

    let message = extract_cstr(&payload[msg_offset..]);
    if app_name.is_empty() || message.is_empty() {
        return;
    }

    let event_type = u16::from_le_bytes([
        payload[22],
        payload[23],
    ]);
    let severity = match event_type {
        1 => "DEBUG",
        2 => "INFO",
        3 => "ERROR",
        4 => "CRIT",
        _ => "???",
    };

    state.push_log(format!(
        "[{severity}] {app_name}: {message}"
    ));
}

fn draw(frame: &mut Frame, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(frame.area());

    let tab_titles = vec!["Logs", "Constellation", "Satellites"];
    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" LeoDOS Dashboard "),
        )
        .select(state.tab)
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(tabs, chunks[0]);

    match state.tab {
        0 => draw_logs(frame, state, chunks[1]),
        1 => draw_constellation(frame, state, chunks[1]),
        2 => draw_satellites(frame, state, chunks[1]),
        _ => {}
    }
}

fn draw_logs(frame: &mut Frame, state: &AppState, area: Rect) {
    let items: Vec<Line> = state
        .logs
        .iter()
        .map(|l| {
            let style = if l.contains("[CRIT]") || l.contains("[ERROR]") {
                Style::default().fg(Color::Red)
            } else if l.contains("[INFO]") {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Line::styled(l.as_str(), style)
        })
        .collect();

    let paragraph = Paragraph::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Event Log "),
        )
        .scroll((
            state.logs.len().saturating_sub(
                area.height.saturating_sub(2) as usize,
            ) as u16,
            0,
        ));
    frame.render_widget(paragraph, area);
}

fn draw_constellation(
    frame: &mut Frame,
    state: &AppState,
    area: Rect,
) {
    let mut lines: Vec<Line> = Vec::new();
    let col_w = 10;
    let link = "───────";
    let margin = "       ";

    // Header
    let mut header = String::from(margin);
    for s in 0..state.sats {
        let label = format!("Sat {s}");
        header.push_str(&format!("{label:<col_w$}"));
    }
    lines.push(Line::from(header));
    lines.push(Line::from(""));

    for o in 0..state.orbits {
        // Row with nodes and horizontal ISL links between them
        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::raw(format!("Orb {o}  ")));

        for s in 0..state.sats {
            let idx =
                o as usize * state.sats as usize + s as usize;
            let info = &state.satellites[idx];

            let (symbol, color) = if info.last_seen == "—" {
                ("○", Color::DarkGray)
            } else if info.err_count > 0 {
                ("◉", Color::Red)
            } else {
                ("●", Color::Green)
            };

            spans.push(Span::styled(
                symbol,
                Style::default().fg(color),
            ));

            if s < state.sats - 1 {
                spans.push(Span::styled(
                    link,
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }
        lines.push(Line::from(spans));

        // Vertical ISL links to next orbit row
        if o < state.orbits - 1 {
            let mut vlink = String::from(margin);
            for s in 0..state.sats {
                vlink.push('│');
                if s < state.sats - 1 {
                    for _ in 0..link.chars().count() {
                        vlink.push(' ');
                    }
                }
            }
            lines.push(Line::styled(
                vlink,
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ○", Style::default().fg(Color::DarkGray)),
        Span::raw(" not seen  "),
        Span::styled("●", Style::default().fg(Color::Green)),
        Span::raw(" active  "),
        Span::styled("◉", Style::default().fg(Color::Red)),
        Span::raw(" errors"),
    ]));

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Constellation "),
    );
    frame.render_widget(paragraph, area);
}

fn draw_satellites(
    frame: &mut Frame,
    state: &AppState,
    area: Rect,
) {
    let header = Row::new(vec![
        "Sat", "Lat", "Lon", "Alt (km)", "Cmds", "Errs",
        "Status",
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = state
        .satellites
        .iter()
        .map(|s| {
            let status = if s.last_seen == "—" {
                "offline"
            } else if s.err_count > 0 {
                "error"
            } else {
                "ok"
            };
            let style = match status {
                "error" => Style::default().fg(Color::Red),
                "offline" => {
                    Style::default().fg(Color::DarkGray)
                }
                _ => Style::default(),
            };
            Row::new(vec![
                format!("{}.{}", s.orb, s.sat),
                format!("{:.2}", s.lat),
                format!("{:.2}", s.lon),
                format!("{:.0}", s.alt),
                format!("{}", s.cmd_count),
                format!("{}", s.err_count),
                status.into(),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(6),
        Constraint::Length(8),
        Constraint::Length(9),
        Constraint::Length(10),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(8),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Satellites "),
        );
    frame.render_widget(table, area);
}

pub async fn run(
    port: u16,
    orbits: u8,
    sats: u8,
) -> Result<()> {
    let state = Arc::new(Mutex::new(AppState::new(orbits, sats)));

    // Background telemetry receiver
    let recv_state = state.clone();
    tokio::spawn(async move {
        let Ok(sock) =
            UdpSocket::bind(format!("0.0.0.0:{port}")).await
        else {
            return;
        };
        let mut buf = [0u8; 2048];
        loop {
            if let Ok((len, _)) = sock.recv_from(&mut buf).await
            {
                let mut s = recv_state.lock().unwrap();
                process_packet(&mut s, &buf[..len]);
            }
        }
    });

    // Terminal setup
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal =
        Terminal::new(CrosstermBackend::new(stdout()))?;

    // Main loop
    loop {
        {
            let s = state.lock().unwrap();
            terminal.draw(|f| draw(f, &s))?;
        }

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Tab => {
                        let mut s = state.lock().unwrap();
                        s.tab = (s.tab + 1) % 3;
                    }
                    KeyCode::BackTab => {
                        let mut s = state.lock().unwrap();
                        s.tab = (s.tab + 2) % 3;
                    }
                    _ => {}
                }
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
