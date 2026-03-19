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
use ratatui::widgets::{*, Clear};
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
    btns: Vec<Btn>,
    selected_btn: usize,
    hover_btn: Option<usize>,
    deploy_popup: bool,
    deploy_apps: Vec<String>,
    deploy_selected: usize,
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
        let (btns, deploy_apps) = discover_buttons();
        Self {
            tab: 0,
            logs: VecDeque::new(),
            satellites,
            orbits,
            sats,
            packet_count: 0,
            action_log: VecDeque::new(),
            btns,
            selected_btn: 0,
            hover_btn: None,
            deploy_popup: false,
            deploy_apps,
            deploy_selected: 0,
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

    if state.deploy_popup {
        draw_deploy_popup(frame, state);
    }
}

fn draw_deploy_popup(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    let popup_height = (state.deploy_apps.len() as u16 + 4).min(area.height.saturating_sub(4));
    let popup_width = 30u16.min(area.width.saturating_sub(4));
    let popup = Rect::new(
        (area.width.saturating_sub(popup_width)) / 2,
        (area.height.saturating_sub(popup_height)) / 2,
        popup_width,
        popup_height,
    );

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Deploy App ")
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if state.deploy_apps.is_empty() {
        frame.render_widget(
            Paragraph::new("No apps found"),
            inner,
        );
        return;
    }

    for (i, app) in state.deploy_apps.iter().enumerate() {
        let y = inner.y + i as u16;
        if y >= inner.y + inner.height {
            break;
        }
        let style = (i == state.deploy_selected)
            .then(|| {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            })
            .unwrap_or(Style::default().fg(Color::White));
        let r = Rect::new(inner.x + 1, y, inner.width.saturating_sub(2), 1);
        frame.render_widget(Span::styled(format!(" {app}"), style), r);
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

/// (label, command, tooltip)
type Btn = (String, String, String);

fn discover_buttons() -> (Vec<Btn>, Vec<String>) {
    let btns: Vec<Btn> = vec![
        (" Build ".into(), "build".into(), "Build NOS3 image, configure, compile sims and FSW".into()),
        (" Start Sim ".into(), "sim-start".into(), "Start the constellation simulation".into()),
        (" Stop Sim ".into(), "sim-stop".into(), "Stop all simulation containers".into()),
        (" Shell ".into(), "shell".into(), "Open an interactive shell in the FSW container".into()),
        (String::new(), String::new(), String::new()),
        (" Deploy... ".into(), "deploy-popup".into(), "Upload and reload a cFS app".into()),
        (" Datagen ".into(), "datagen".into(), "Generate synthetic thermal sensor data".into()),
        (" Enable TO_LAB ".into(), "enable-tolab".into(), "Enable telemetry output via TO_LAB".into()),
        (" Status ".into(), "status".into(), "Query system status".into()),
    ];

    let mut apps = Vec::new();
    if let Ok(entries) = std::fs::read_dir("apps") {
        apps = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().join("fsw/Cargo.toml").exists())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        apps.sort();
    }

    (btns, apps)
}

fn btn_index_to_real(btns: &[Btn], idx: usize) -> usize {
    let mut real = 0;
    for (i, (label, _, _)) in btns.iter().enumerate() {
        if label.is_empty() { continue; }
        if real == idx { return i; }
        real += 1;
    }
    0
}

fn real_to_btn_index(btns: &[Btn], real: usize) -> usize {
    let mut idx = 0;
    for (i, (label, _, _)) in btns.iter().enumerate() {
        if label.is_empty() { continue; }
        if i == real { return idx; }
        idx += 1;
    }
    0
}

fn btn_count(btns: &[Btn]) -> usize {
    btns.iter().filter(|(l, _, _)| !l.is_empty()).count()
}

fn draw_control(
    frame: &mut Frame,
    state: &AppState,
    area: Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(22),
            Constraint::Min(0),
        ])
        .split(area);

    let btn_area = chunks[0];
    let log_area = chunks[1];

    let btn_normal =
        Style::default().fg(Color::White).bg(Color::DarkGray);
    let btn_selected = Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let btn_hover =
        Style::default().fg(Color::Black).bg(Color::Gray);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Control ");
    let inner = block.inner(btn_area);
    frame.render_widget(block, btn_area);

    let selected_real = btn_index_to_real(&state.btns, state.selected_btn);

    let mut y_offset = 0u16;
    for (i, (label, _, _)) in state.btns.iter().enumerate() {
        if i as u16 + y_offset >= inner.height {
            break;
        }
        if label.is_empty() {
            y_offset += 1;
            continue;
        }
        let style = if i == selected_real {
            btn_selected
        } else if state.hover_btn == Some(i) {
            btn_hover
        } else {
            btn_normal
        };
        let r = Rect::new(
            inner.x + 1,
            inner.y + i as u16,
            label.len() as u16,
            1,
        );
        if r.y < inner.y + inner.height {
            frame.render_widget(
                Span::styled(label.as_str(), style),
                r,
            );
        }
    }

    // Separator + tooltip + key hints below buttons
    let btns_end = state.btns.len() as u16 + 1;
    if btns_end < inner.height {
        let sep_y = inner.y + btns_end;
        let w = inner.width.saturating_sub(2);
        let sep = "─".repeat(w as usize);
        frame.render_widget(
            Span::styled(&sep, Style::default().fg(Color::DarkGray)),
            Rect::new(inner.x + 1, sep_y, w, 1),
        );

        // Tooltip (word-wrapped into the button panel)
        let (_, _, tooltip) = &state.btns[selected_real];
        if !tooltip.is_empty() {
            let tooltip_area = Rect::new(
                inner.x + 1,
                sep_y + 1,
                w,
                inner.height.saturating_sub(btns_end + 2),
            );
            frame.render_widget(
                Paragraph::new(tooltip.as_str())
                    .wrap(Wrap { trim: true })
                    .style(Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC)),
                tooltip_area,
            );
        }

        // Key hints at bottom
        let hints_y = inner.y + inner.height - 1;
        if hints_y > sep_y + 1 {
            frame.render_widget(
                Span::styled(
                    "j/k ↑↓  Enter  q",
                    Style::default().fg(Color::DarkGray),
                ),
                Rect::new(inner.x + 1, hints_y, w, 1),
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
    let base_row = 4; // tab(3) + border(1)
    let base_col = 2;

    for (i, (label, cmd, _)) in state.btns.clone().iter().enumerate() {
        if label.is_empty() {
            continue;
        }
        let by = base_row + i as u16;
        let bx = base_col;
        let bw = label.len() as u16;
        if row == by && col >= bx && col < bx + bw {
            state.selected_btn = real_to_btn_index(&state.btns, i);
            return Some(trigger_btn(state, cmd));
        }
    }
    None
}

fn handle_hover(state: &mut AppState, _col: u16, row: u16) {
    let base_row = 4;
    state.hover_btn = None;
    for (i, (label, _, _)) in state.btns.iter().enumerate() {
        if label.is_empty() {
            continue;
        }
        if row == base_row + i as u16 {
            state.hover_btn = Some(i);
            return;
        }
    }
}

fn activate_selected(state: &mut AppState) -> Option<String> {
    let real = btn_index_to_real(&state.btns, state.selected_btn);
    let cmd = state.btns[real].1.clone();
    if cmd.is_empty() {
        return None;
    }
    Some(trigger_btn(state, &cmd))
}

fn trigger_btn(state: &mut AppState, cmd: &str) -> String {
    if cmd == "deploy-popup" {
        state.deploy_popup = true;
        state.deploy_selected = 0;
        return String::new();
    }
    state.action_log.push_back(format!("> {cmd}"));
    if state.action_log.len() > 50 {
        state.action_log.pop_front();
    }
    cmd.to_string()
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
                    let mut s = state.lock().unwrap();
                    if s.deploy_popup {
                        match key.code {
                            KeyCode::Esc => {
                                s.deploy_popup = false;
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                let count = s.deploy_apps.len();
                                if count > 0 {
                                    s.deploy_selected =
                                        (s.deploy_selected + count - 1) % count;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                let count = s.deploy_apps.len();
                                if count > 0 {
                                    s.deploy_selected =
                                        (s.deploy_selected + 1) % count;
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(app) =
                                    s.deploy_apps.get(s.deploy_selected).cloned()
                                {
                                    s.deploy_popup = false;
                                    let cmd = format!("deploy {app}");
                                    s.action_log.push_back(format!("> {cmd}"));
                                    drop(s);
                                    spawn_action(cmd, state.clone());
                                    continue;
                                }
                            }
                            _ => {}
                        }
                        continue;
                    }
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            drop(s);
                            break;
                        }
                        KeyCode::Tab => {
                            s.tab = (s.tab + 1) % 4;
                        }
                        KeyCode::BackTab => {
                            s.tab = (s.tab + 3) % 4;
                        }
                        KeyCode::Up | KeyCode::Char('k')
                            if s.tab == 3 =>
                        {
                            let count = btn_count(&s.btns);
                            s.selected_btn =
                                (s.selected_btn + count - 1) % count;
                        }
                        KeyCode::Down | KeyCode::Char('j')
                            if s.tab == 3 =>
                        {
                            s.selected_btn =
                                (s.selected_btn + 1) % btn_count(&s.btns);
                        }
                        KeyCode::Enter if s.tab == 3 => {
                            if let Some(cmd) =
                                activate_selected(&mut s)
                            {
                                if cmd.is_empty() { continue; }
                                drop(s);
                                spawn_action(
                                    cmd,
                                    state.clone(),
                                );
                                continue;
                            }
                        }
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => {
                    let mut s = state.lock().unwrap();
                    if s.tab == 3 {
                        match mouse.kind {
                            MouseEventKind::Down(
                                MouseButton::Left,
                            ) => {
                                if let Some(cmd) =
                                    handle_click(
                                        &mut s,
                                        mouse.column,
                                        mouse.row,
                                    )
                                {
                                    drop(s);
                                    spawn_action(
                                        cmd,
                                        state.clone(),
                                    );
                                    continue;
                                }
                            }
                            MouseEventKind::Moved => {
                                handle_hover(
                                    &mut s,
                                    mouse.column,
                                    mouse.row,
                                );
                            }
                            _ => {}
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
