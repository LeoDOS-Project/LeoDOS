use anyhow::Result;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event,
    KeyCode, KeyEventKind, MouseButton, MouseEventKind,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen,
    LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use leodos_protocols::network::spp::{PacketType, SpacePacket};
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
    packet_count: u64,
    action_log: VecDeque<String>,
    buttons: Vec<Button>,
}

#[derive(Clone)]
struct Button {
    label: String,
    action: Action,
    x: u16,
    y: u16,
    w: u16,
}

#[derive(Clone)]
enum Action {
    Build,
    SimStart,
    SimStop,
    SimShell,
    Deploy(&'static str),
    EnableToLab,
    Datagen,
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
            packet_count: 0,
            action_log: VecDeque::new(),
            buttons: Vec::new(),
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

// cFS TLM secondary header size (8-byte time + 2-byte spare).
const CFE_TLM_HDR_SIZE: usize = 10;

fn process_packet(state: &mut AppState, raw: &[u8]) {
    let Ok(pkt) = SpacePacket::parse(raw) else {
        return;
    };

    state.packet_count += 1;

    let payload = pkt.data_field();

    // Try EVS long event format first:
    //   AppName[20] + EventID(u16) + EventType(u16) + Message[...]
    if payload.len() >= 26 {
        let app_name = extract_cstr(&payload[..20]);
        let msg_offset = 24;
        if msg_offset < payload.len() {
            let message = extract_cstr(&payload[msg_offset..]);
            if !app_name.is_empty() && !message.is_empty() {
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
        }
    }

    // TODO: Currently all packets update satellite 0.0.
    // Per-satellite routing requires separate telemetry
    // ports or an in-band satellite identifier.
    if !state.satellites.is_empty() {
        update_sat_from_packet(
            &mut state.satellites[0],
            &pkt,
            payload,
        );
    }
}

fn update_sat_from_packet(
    sat: &mut SatInfo,
    pkt: &SpacePacket,
    payload: &[u8],
) {
    sat.last_seen = "active".into();

    if pkt.packet_type() != PacketType::Telemetry {
        return;
    }

    // HK telemetry payloads carry the cFS TLM secondary
    // header followed by cmd_count(u8) + err_count(u8).
    let hk_offset = CFE_TLM_HDR_SIZE;
    if payload.len() >= hk_offset + 2 {
        sat.cmd_count = payload[hk_offset] as u16;
        sat.err_count = payload[hk_offset + 1] as u16;
    }
}

fn draw(frame: &mut Frame, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(frame.area());

    let tab_titles =
        vec!["Logs", "Constellation", "Satellites", "Control"];
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
        3 => draw_control(frame, state, chunks[1]),
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

    let log_title = format!(
        " Event Log ({} packets) ",
        state.packet_count,
    );
    let paragraph = Paragraph::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(log_title),
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
    let link = "────";
    let margin = "    ";
    let label_style = Style::default().fg(Color::DarkGray);
    let link_style = Style::default().fg(Color::DarkGray);

    // Header — each node is 1 display-char wide, each link
    // is link_len chars. "S0" is 2 chars, so pad 1 less
    // after each label to stay aligned.
    let link_len = link.chars().count();
    let mut hdr_spans: Vec<Span> = Vec::new();
    hdr_spans.push(Span::styled(margin, label_style));
    for s in 0..state.sats {
        let label = format!("S{s}");
        let label_w = label.len();
        hdr_spans.push(Span::styled(label, label_style));
        if s < state.sats - 1 {
            // node is 1 char + link_len chars = gap.
            // label is label_w chars, so pad by
            // (1 + link_len - label_w) to realign.
            let pad = 1 + link_len - label_w;
            hdr_spans.push(Span::raw(" ".repeat(pad)));
        }
    }
    lines.push(Line::from(hdr_spans));
    lines.push(Line::from(""));

    for o in 0..state.orbits {
        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::styled(
            format!("O{o}  "),
            label_style,
        ));

        for s in 0..state.sats {
            let idx =
                o as usize * state.sats as usize + s as usize;
            let info = &state.satellites[idx];

            let (symbol, color) = if info.last_seen == "—" {
                ("○", Color::Gray)
            } else if info.err_count > 0 {
                ("◉", Color::LightRed)
            } else {
                ("●", Color::LightGreen)
            };

            spans.push(Span::styled(
                symbol,
                Style::default()
                    .fg(color)
                    .add_modifier(Modifier::BOLD),
            ));

            if s < state.sats - 1 {
                spans.push(Span::styled(link, link_style));
            }
        }
        lines.push(Line::from(spans));

        if o < state.orbits - 1 {
            let mut vspans: Vec<Span> = Vec::new();
            vspans.push(Span::raw(String::from(margin)));
            for s in 0..state.sats {
                vspans.push(Span::styled("│", link_style));
                if s < state.sats - 1 {
                    let pad = " ".repeat(link.chars().count());
                    vspans.push(Span::raw(pad));
                }
            }
            lines.push(Line::from(vspans));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "  ○",
            Style::default().fg(Color::Gray),
        ),
        Span::raw(" offline  "),
        Span::styled(
            "●",
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" active  "),
        Span::styled(
            "◉",
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        ),
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

fn draw_control(
    frame: &mut Frame,
    state: &AppState,
    area: Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(12),
            Constraint::Min(0),
        ])
        .split(area);

    let btn_area = chunks[0];
    let log_area = chunks[1];

    // Button layout
    let btn_style = Style::default().fg(Color::White).bg(Color::DarkGray);
    let btn_hl = Style::default().fg(Color::Black).bg(Color::Cyan);

    struct Btn {
        label: &'static str,
        col: u16,
        row: u16,
    }

    let buttons = [
        // Row 0: System
        Btn { label: " Build ", col: 2, row: 1 },
        Btn { label: " Start Sim ", col: 12, row: 1 },
        Btn { label: " Stop Sim ", col: 26, row: 1 },
        Btn { label: " Shell ", col: 39, row: 1 },
        // Row 1: Deploy
        Btn { label: " Deploy wildfire ", col: 2, row: 3 },
        Btn { label: " Deploy router ", col: 22, row: 3 },
        Btn { label: " Deploy fs_srv ", col: 40, row: 3 },
        Btn { label: " Deploy gossip ", col: 58, row: 3 },
        // Row 2: Tools
        Btn { label: " Datagen ", col: 2, row: 5 },
        Btn { label: " Enable TO_LAB ", col: 14, row: 5 },
        Btn { label: " Status ", col: 32, row: 5 },
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Control Panel (click buttons) ");
    let inner = block.inner(btn_area);
    frame.render_widget(block, btn_area);

    for btn in &buttons {
        let r = Rect::new(
            inner.x + btn.col,
            inner.y + btn.row,
            btn.label.len() as u16,
            1,
        );
        if r.x + r.width <= inner.x + inner.width
            && r.y < inner.y + inner.height
        {
            frame.render_widget(
                Span::styled(btn.label, btn_style),
                r,
            );
        }
    }

    // Action output log
    let items: Vec<Line> = state
        .action_log
        .iter()
        .map(|l| Line::styled(l.as_str(), Style::default().fg(Color::Cyan)))
        .collect();

    let paragraph = Paragraph::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Output "),
        )
        .scroll((
            state.action_log.len().saturating_sub(
                log_area.height.saturating_sub(2) as usize,
            ) as u16,
            0,
        ));
    frame.render_widget(paragraph, log_area);
}

fn handle_click(
    state: &mut AppState,
    col: u16,
    row: u16,
) -> Option<String> {
    // Tab bar is 3 rows, control panel block border is 1,
    // so button row 1 starts at screen row 5
    let base_row = 4; // tab(3) + border(1)
    let base_col = 3; // border(1) + margin(2)

    struct BtnHit {
        label: &'static str,
        cmd: &'static str,
        col: u16,
        row: u16,
    }

    let buttons = [
        BtnHit { label: " Build ", cmd: "build", col: 2, row: 1 },
        BtnHit { label: " Start Sim ", cmd: "sim-start", col: 12, row: 1 },
        BtnHit { label: " Stop Sim ", cmd: "sim-stop", col: 26, row: 1 },
        BtnHit { label: " Shell ", cmd: "shell", col: 39, row: 1 },
        BtnHit { label: " Deploy wildfire ", cmd: "deploy wildfire", col: 2, row: 3 },
        BtnHit { label: " Deploy router ", cmd: "deploy router", col: 22, row: 3 },
        BtnHit { label: " Deploy fs_srv ", cmd: "deploy fs_srv", col: 40, row: 3 },
        BtnHit { label: " Deploy gossip ", cmd: "deploy gossip", col: 58, row: 3 },
        BtnHit { label: " Datagen ", cmd: "datagen", col: 2, row: 5 },
        BtnHit { label: " Enable TO_LAB ", cmd: "enable-tolab", col: 14, row: 5 },
        BtnHit { label: " Status ", cmd: "status", col: 32, row: 5 },
    ];

    for btn in &buttons {
        let bx = base_col + btn.col;
        let by = base_row + btn.row;
        let bw = btn.label.len() as u16;
        if col >= bx && col < bx + bw && row == by {
            state.action_log.push_back(
                format!("> {}", btn.cmd),
            );
            if state.action_log.len() > 50 {
                state.action_log.pop_front();
            }
            return Some(btn.cmd.to_string());
        }
    }
    None
}

fn spawn_action(cmd: String, state: Arc<Mutex<AppState>>) {
    tokio::spawn(async move {
        let output = match cmd.as_str() {
            "build" => run_shell("make nos3-build && make nos3-config && make nos3-build-sim && make nos3-build-fsw").await,
            "sim-start" => run_shell("make constellation-build && make constellation-gen && docker compose -f docker-compose.constellation.yml up -d").await,
            "sim-stop" => run_shell("docker compose -f docker-compose.constellation.yml down").await,
            "shell" => { return; } // Can't run interactive shell from TUI
            "datagen" => run_shell("cd tools/eosim && uv run eosim wildfire examples/california_wildfire.yaml -o output/ --fmt bin").await,
            "status" => run_shell("echo 'Status query sent'").await,
            "enable-tolab" => run_shell("echo 'TO_LAB enable sent'").await,
            cmd if cmd.starts_with("deploy ") => {
                let app = &cmd[7..];
                run_shell(&format!(
                    "echo 'Deploying {app}...'"
                )).await
            }
            _ => Ok("Unknown command".into()),
        };
        let msg = match output {
            Ok(out) => format!("  done: {}", out.trim()),
            Err(e) => format!("  error: {e}"),
        };
        if let Ok(mut s) = state.lock() {
            s.action_log.push_back(msg);
        }
    });
}

async fn run_shell(cmd: &str) -> Result<String> {
    let output = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .await?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
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
    stdout().execute(EnableMouseCapture)?;
    let mut terminal =
        Terminal::new(CrosstermBackend::new(stdout()))?;

    // Main loop
    loop {
        {
            let s = state.lock().unwrap();
            terminal.draw(|f| draw(f, &s))?;
        }

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            break;
                        }
                        KeyCode::Tab => {
                            let mut s = state.lock().unwrap();
                            s.tab = (s.tab + 1) % 4;
                        }
                        KeyCode::BackTab => {
                            let mut s = state.lock().unwrap();
                            s.tab = (s.tab + 3) % 4;
                        }
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => {
                    if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                        let mut s = state.lock().unwrap();
                        if s.tab == 3 {
                            if let Some(cmd) = handle_click(
                                &mut s,
                                mouse.column,
                                mouse.row,
                            ) {
                                drop(s);
                                spawn_action(
                                    cmd,
                                    state.clone(),
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Cleanup
    stdout().execute(DisableMouseCapture)?;
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
