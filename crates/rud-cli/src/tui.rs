// RUD TUI Dashboard - Real-time ratatui interface organized into panels:
//   [F1] Nodes     - live node registry with status indicators
//   [F2] Metrics   - per-node sparkline charts (latency, CPU, message rate)
//   [F3] Anomalies - CAD anomaly feed with severity coloring
//   [F4] Logs      - scrolling log stream with level filtering
//
// Keybindings:
//   Tab / Shift+Tab  - cycle panels
//   F1..F4           - jump to panel
//   q / Ctrl+C       - quit
//   r                - run Nexus remediation on selected anomaly
//   j/k or arrows    - navigate lists

use std::{
    collections::VecDeque,
    io,
    sync::Arc,
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Clear, Gauge, List, ListItem, ListState, Paragraph, Row, Sparkline,
        Table, Tabs, Wrap,
    },
    Frame, Terminal,
};

use rud_core::{
    node::NodeStatus,
    state::{LogLevel, SharedState},
};

const HELP_TEXT: &str = " Tab/F1-F4: switch panel  |  j/k: navigate  |  r: remediate  |  ?: help  |  q: quit ";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Panel {
    Nodes = 0,
    Metrics = 1,
    Anomalies = 2,
    Logs = 3,
}

impl Panel {
    fn titles() -> Vec<&'static str> {
        vec!["[F1] Nodes", "[F2] Metrics", "[F3] Anomalies", "[F4] Logs"]
    }

    fn index(&self) -> usize {
        match self {
            Panel::Nodes => 0,
            Panel::Metrics => 1,
            Panel::Anomalies => 2,
            Panel::Logs => 3,
        }
    }

    fn from_index(i: usize) -> Self {
        match i {
            0 => Panel::Nodes,
            1 => Panel::Metrics,
            2 => Panel::Anomalies,
            _ => Panel::Logs,
        }
    }

    fn next(&self) -> Self {
        Self::from_index((self.index() + 1) % 4)
    }

    fn prev(&self) -> Self {
        Self::from_index((self.index() + 3) % 4)
    }
}

struct AppState {
    active_panel: Panel,
    node_table_state: ratatui::widgets::TableState,
    anomaly_list_state: ListState,
    log_list_state: ListState,
    show_help: bool,
    sparkline_data: std::collections::HashMap<String, VecDeque<u64>>,
    last_sparkline_update: Instant,
}

impl AppState {
    fn new() -> Self {
        let mut node_table_state = ratatui::widgets::TableState::default();
        node_table_state.select(Some(0));
        let mut anomaly_list_state = ListState::default();
        anomaly_list_state.select(Some(0));
        let log_list_state = ListState::default();
        Self {
            active_panel: Panel::Nodes,
            node_table_state,
            anomaly_list_state,
            log_list_state,
            show_help: false,
            sparkline_data: Default::default(),
            last_sparkline_update: Instant::now(),
        }
    }

    fn nav_down(&mut self, len: usize) {
        if len == 0 { return; }
        match self.active_panel {
            Panel::Nodes | Panel::Metrics => {
                let next = self.node_table_state.selected().map(|i| (i + 1) % len).unwrap_or(0);
                self.node_table_state.select(Some(next));
            }
            Panel::Anomalies => {
                let next = self.anomaly_list_state.selected().map(|i| (i + 1) % len).unwrap_or(0);
                self.anomaly_list_state.select(Some(next));
            }
            Panel::Logs => {
                let next = self.log_list_state.selected().map(|i| (i + 1) % len).unwrap_or(0);
                self.log_list_state.select(Some(next));
            }
        }
    }

    fn nav_up(&mut self, len: usize) {
        if len == 0 { return; }
        match self.active_panel {
            Panel::Nodes | Panel::Metrics => {
                let prev = self.node_table_state.selected().map(|i| if i == 0 { len - 1 } else { i - 1 }).unwrap_or(0);
                self.node_table_state.select(Some(prev));
            }
            Panel::Anomalies => {
                let prev = self.anomaly_list_state.selected().map(|i| if i == 0 { len - 1 } else { i - 1 }).unwrap_or(0);
                self.anomaly_list_state.select(Some(prev));
            }
            Panel::Logs => {
                let prev = self.log_list_state.selected().map(|i| if i == 0 { len - 1 } else { i - 1 }).unwrap_or(0);
                self.log_list_state.select(Some(prev));
            }
        }
    }
}

pub async fn run_tui(state: Arc<SharedState>, refresh_ms: u64) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_event_loop(&mut terminal, state, refresh_ms).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: Arc<SharedState>,
    refresh_ms: u64,
) -> anyhow::Result<()> {
    let mut app = AppState::new();
    let tick = Duration::from_millis(refresh_ms);

    loop {
        if app.last_sparkline_update.elapsed() > Duration::from_millis(200) {
            update_sparklines(&mut app, &state);
            app.last_sparkline_update = Instant::now();
        }

        terminal.draw(|f| draw(f, &mut app, &state))?;

        if crossterm::event::poll(tick)? {
            if let Event::Key(key) = event::read()? {
                match (key.code, key.modifiers) {
                    (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => break,
                    (KeyCode::Tab, KeyModifiers::NONE) => {
                        app.active_panel = app.active_panel.next();
                    }
                    (KeyCode::BackTab, _) => {
                        app.active_panel = app.active_panel.prev();
                    }
                    (KeyCode::F(1), _) => app.active_panel = Panel::Nodes,
                    (KeyCode::F(2), _) => app.active_panel = Panel::Metrics,
                    (KeyCode::F(3), _) => app.active_panel = Panel::Anomalies,
                    (KeyCode::F(4), _) => app.active_panel = Panel::Logs,
                    (KeyCode::Char('?'), _) => app.show_help = !app.show_help,
                    (KeyCode::Esc, _) => app.show_help = false,
                    (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                        let len = panel_len(&app.active_panel, &state);
                        app.nav_down(len);
                    }
                    (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                        let len = panel_len(&app.active_panel, &state);
                        app.nav_up(len);
                    }
                    (KeyCode::Char('r'), _) => {
                        run_remediate(&app, &state);
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn panel_len(panel: &Panel, state: &SharedState) -> usize {
    match panel {
        Panel::Nodes | Panel::Metrics => state.nodes.len(),
        Panel::Anomalies => state.anomalies.read().len(),
        Panel::Logs => state.logs.read().len(),
    }
}

fn run_remediate(app: &AppState, state: &SharedState) {
    let anomalies = state.anomalies.read().clone();
    if let Some(idx) = app.anomaly_list_state.selected() {
        let rev_idx = anomalies.len().saturating_sub(1).saturating_sub(idx);
        if let Some(anomaly) = anomalies.get(rev_idx) {
            let anomaly = anomaly.clone();
            drop(anomalies);
            let mut nexus = rud_ghost::nexus::NexusRemediationEngine::new();
            nexus.analyze(&anomaly, state);
        }
    }
}

fn update_sparklines(app: &mut AppState, state: &SharedState) {
    for entry in state.metrics.iter() {
        if let Some(sample) = entry.value().samples.back() {
            let key = entry.key().to_string();
            let hist = app
                .sparkline_data
                .entry(key)
                .or_insert_with(|| VecDeque::with_capacity(64));
            if hist.len() >= 64 {
                hist.pop_front();
            }
            hist.push_back(sample.latency_us.round() as u64);
        }
    }
}

fn draw(f: &mut Frame, app: &mut AppState, state: &SharedState) {
    let area = f.area();

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    draw_header(f, app, root[0]);
    draw_body(f, app, state, root[1]);
    draw_footer(f, root[2]);

    if app.show_help {
        draw_help_overlay(f, area);
    }
}

fn draw_header(f: &mut Frame, app: &AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    let brand = Paragraph::new(Line::from(vec![
        Span::styled(
            "RUD",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" :: Robotics Universal Debugger  "),
        Span::styled("v0.1.0", Style::default().fg(Color::DarkGray)),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(brand, chunks[0]);

    let tabs = Tabs::new(Panel::titles())
        .select(app.active_panel.index())
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(tabs, chunks[1]);
}

fn draw_footer(f: &mut Frame, area: Rect) {
    let footer = Paragraph::new(HELP_TEXT)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(footer, area);
}

fn draw_body(f: &mut Frame, app: &mut AppState, state: &SharedState, area: Rect) {
    match app.active_panel {
        Panel::Nodes => draw_nodes(f, app, state, area),
        Panel::Metrics => draw_metrics(f, app, state, area),
        Panel::Anomalies => draw_anomalies(f, app, state, area),
        Panel::Logs => draw_logs(f, app, state, area),
    }
}

fn status_color(status: &NodeStatus) -> Color {
    match status {
        NodeStatus::Online => Color::Green,
        NodeStatus::Degraded => Color::Yellow,
        NodeStatus::Offline => Color::Red,
        NodeStatus::Anomalous => Color::Magenta,
        NodeStatus::Remediating => Color::Cyan,
    }
}

fn draw_nodes(f: &mut Frame, app: &mut AppState, state: &SharedState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let mut nodes: Vec<_> = state.nodes.iter().map(|e| e.value().clone()).collect();
    nodes.sort_by(|a, b| a.name.cmp(&b.name));

    let header = Row::new(vec!["NAME", "KIND", "PROTOCOL", "ENDPOINT", "STATUS"])
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .height(1);

    let rows: Vec<Row> = nodes
        .iter()
        .map(|n| {
            let color = status_color(&n.status);
            Row::new(vec![
                Cell::from(n.name.clone()),
                Cell::from(n.kind.to_string()),
                Cell::from(n.protocol.to_string()),
                Cell::from(n.endpoint.chars().take(20).collect::<String>()),
                Cell::from(n.status.to_string())
                    .style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
            ])
        })
        .collect();

    let selected_style = Style::default()
        .bg(Color::DarkGray)
        .add_modifier(Modifier::BOLD);

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(22),
            Constraint::Percentage(10),
            Constraint::Percentage(16),
            Constraint::Percentage(26),
            Constraint::Percentage(16),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Nodes ({}) ", nodes.len()))
            .title_style(Style::default().fg(Color::Cyan)),
    )
    .row_highlight_style(selected_style);

    f.render_stateful_widget(table, chunks[0], &mut app.node_table_state);
    draw_system_stats(f, state, chunks[1]);
}

fn draw_system_stats(f: &mut Frame, state: &SharedState, area: Rect) {
    let shards = *state.qds_shards_allocated.read();
    let daemon_state = state.daemon_state.read().clone();
    let anomaly_count = state.anomalies.read().len();
    let uptime = (chrono::Utc::now() - state.started_at).num_seconds();

    let text = vec![
        Line::from(vec![
            Span::styled("Daemon State  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                daemon_state.to_string(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("QDS Shards    ", Style::default().fg(Color::DarkGray)),
            Span::styled(shards.to_string(), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Anomalies     ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                anomaly_count.to_string(),
                if anomaly_count > 0 {
                    Style::default().fg(Color::Magenta)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("Uptime        ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}s", uptime), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "-- Aether-Link --",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(vec![
            Span::styled("NRE           ", Style::default().fg(Color::DarkGray)),
            Span::styled("ONLINE", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("QDS Fabric    ", Style::default().fg(Color::DarkGray)),
            Span::styled("ONLINE", Style::default().fg(Color::Green)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "-- Ghost-Trace --",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(vec![
            Span::styled("CAD           ", Style::default().fg(Color::DarkGray)),
            Span::styled("LEARNING", Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("ESM           ", Style::default().fg(Color::DarkGray)),
            Span::styled("ACTIVE", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("Nexus         ", Style::default().fg(Color::DarkGray)),
            Span::styled("STANDBY", Style::default().fg(Color::Cyan)),
        ]),
    ];

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" System Status ")
                .title_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn draw_metrics(f: &mut Frame, app: &mut AppState, state: &SharedState, area: Rect) {
    let mut nodes: Vec<_> = state.nodes.iter().map(|e| e.value().clone()).collect();
    nodes.sort_by(|a, b| a.name.cmp(&b.name));

    if nodes.is_empty() {
        let msg = Paragraph::new(
            "No nodes registered. Run 'rud --init' or 'rud scan --all-protocols'.",
        )
        .block(Block::default().borders(Borders::ALL).title(" Metrics "));
        f.render_widget(msg, area);
        return;
    }

    let selected = app
        .node_table_state
        .selected()
        .unwrap_or(0)
        .min(nodes.len() - 1);
    let node = &nodes[selected];

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Node selector bar
    let spans: Vec<Span> = nodes
        .iter()
        .enumerate()
        .flat_map(|(i, n)| {
            let style = if i == selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            vec![Span::styled(format!("  {}  ", n.name), style)]
        })
        .collect();

    f.render_widget(
        Paragraph::new(Line::from(spans))
            .block(Block::default().borders(Borders::ALL).title(" Node ")),
        chunks[0],
    );

    let metric_area = chunks[1];

    if let Some(window) = state.metrics.get(&node.id) {
        let mean_lat = window.mean_latency();
        let mean_cpu = window.mean_cpu();
        let mean_drop = if window.samples.is_empty() {
            0.0
        } else {
            window
                .samples
                .iter()
                .map(|s| s.drop_rate_pct)
                .sum::<f64>()
                / window.samples.len() as f64
        };
        let mean_msg = if window.samples.is_empty() {
            0.0
        } else {
            window
                .samples
                .iter()
                .map(|s| s.msg_rate_hz)
                .sum::<f64>()
                / window.samples.len() as f64
        };
        drop(window);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(metric_area);
        let top_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(rows[0]);
        let bot_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(rows[1]);

        let spark_data: Vec<u64> = app
            .sparkline_data
            .get(&node.id.to_string())
            .map(|d| d.iter().cloned().collect())
            .unwrap_or_default();

        let latency_spark = Sparkline::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" Latency  mean={:.1}us ", mean_lat))
                    .title_style(Style::default().fg(Color::Cyan)),
            )
            .data(&spark_data)
            .style(Style::default().fg(Color::Green));
        f.render_widget(latency_spark, top_cols[0]);

        let cpu_gauge = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" CPU  mean={:.1}% ", mean_cpu))
                    .title_style(Style::default().fg(Color::Cyan)),
            )
            .gauge_style(Style::default().fg(
                if mean_cpu > 80.0 {
                    Color::Red
                } else if mean_cpu > 50.0 {
                    Color::Yellow
                } else {
                    Color::Green
                },
            ))
            .percent(mean_cpu.round() as u16);
        f.render_widget(cpu_gauge, top_cols[1]);

        let msg_vals: Vec<u64> = (0..spark_data.len())
            .map(|_| mean_msg.round() as u64)
            .collect();
        let msg_spark = Sparkline::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" Msg Rate  mean={:.1}Hz ", mean_msg))
                    .title_style(Style::default().fg(Color::Cyan)),
            )
            .data(&msg_vals)
            .style(Style::default().fg(Color::Blue));
        f.render_widget(msg_spark, bot_cols[0]);

        let drop_gauge = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" Drop Rate  mean={:.2}% ", mean_drop))
                    .title_style(Style::default().fg(Color::Cyan)),
            )
            .gauge_style(Style::default().fg(if mean_drop > 5.0 {
                Color::Red
            } else {
                Color::Green
            }))
            .percent((mean_drop * 10.0).round().min(100.0) as u16);
        f.render_widget(drop_gauge, bot_cols[1]);
    } else {
        let msg = Paragraph::new("No metric data yet. Waiting for Sentinel probe ...")
            .block(Block::default().borders(Borders::ALL).title(" Metrics "));
        f.render_widget(msg, metric_area);
    }
}

fn draw_anomalies(f: &mut Frame, app: &mut AppState, state: &SharedState, area: Rect) {
    let anomalies = state.anomalies.read();

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    let items: Vec<ListItem> = anomalies
        .iter()
        .rev()
        .map(|a| {
            let color = match a.severity {
                rud_core::state::Severity::Critical => Color::Red,
                rud_core::state::Severity::High => Color::LightRed,
                rud_core::state::Severity::Medium => Color::Yellow,
                rud_core::state::Severity::Low => Color::White,
            };
            let prefix = match a.severity {
                rud_core::state::Severity::Critical => "!! ",
                rud_core::state::Severity::High => "!  ",
                _ => "   ",
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    prefix,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<20}", a.node_name),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!(" {:<16}", a.kind.to_string()),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!(" {:>8}", a.severity.to_string()),
                    Style::default().fg(color),
                ),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Anomalies ({}) ", anomalies.len()))
                .title_style(Style::default().fg(Color::Magenta)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));

    f.render_stateful_widget(list, chunks[0], &mut app.anomaly_list_state);

    let detail = if let Some(idx) = app.anomaly_list_state.selected() {
        let rev_idx = anomalies.len().saturating_sub(1).saturating_sub(idx);
        anomalies.get(rev_idx).cloned()
    } else {
        None
    };
    drop(anomalies);

    if let Some(a) = detail {
        let text = vec![
            Line::from(vec![
                Span::styled("NODE       ", Style::default().fg(Color::DarkGray)),
                Span::raw(a.node_name.clone()),
            ]),
            Line::from(vec![
                Span::styled("KIND       ", Style::default().fg(Color::DarkGray)),
                Span::raw(a.kind.to_string()),
            ]),
            Line::from(vec![
                Span::styled("SEVERITY   ", Style::default().fg(Color::DarkGray)),
                Span::raw(a.severity.to_string()),
            ]),
            Line::from(vec![
                Span::styled("Z-SCORE    ", Style::default().fg(Color::DarkGray)),
                Span::raw(
                    a.z_score
                        .map(|z| format!("{:.3}", z))
                        .unwrap_or_else(|| "n/a".into()),
                ),
            ]),
            Line::from(vec![
                Span::styled("DETECTED   ", Style::default().fg(Color::DarkGray)),
                Span::raw(a.detected_at.format("%H:%M:%S%.3f").to_string()),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "DESCRIPTION",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::raw(a.description.clone())),
            Line::from(""),
            Line::from(Span::styled(
                "REMEDIATION",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                a.remediation
                    .clone()
                    .unwrap_or_else(|| "press [r] to invoke Nexus".into()),
                if a.remediation.is_some() {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            )),
        ];
        let detail_widget = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Anomaly Detail ")
                    .title_style(Style::default().fg(Color::Magenta)),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(detail_widget, chunks[1]);
    } else {
        let placeholder = Paragraph::new("Select an anomaly to view details.")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Anomaly Detail "),
            );
        f.render_widget(placeholder, chunks[1]);
    }
}

fn draw_logs(f: &mut Frame, app: &mut AppState, state: &SharedState, area: Rect) {
    let logs = state.logs.read();

    let items: Vec<ListItem> = logs
        .iter()
        .rev()
        .take(500)
        .map(|entry| {
            let (level_color, level_str) = match entry.level {
                LogLevel::Trace => (Color::DarkGray, "TRACE"),
                LogLevel::Debug => (Color::Blue, "DEBUG"),
                LogLevel::Info => (Color::Green, "INFO "),
                LogLevel::Warn => (Color::Yellow, "WARN "),
                LogLevel::Error => (Color::Red, "ERROR"),
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    entry.timestamp.format("%H:%M:%S%.3f").to_string(),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(" "),
                Span::styled(
                    level_str,
                    Style::default()
                        .fg(level_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("[{:<12}] ", entry.source),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(entry.message.clone()),
            ]))
        })
        .collect();

    let len = logs.len();
    drop(logs);

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Logs ({}) - newest first ", len))
                .title_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));

    f.render_stateful_widget(list, area, &mut app.log_list_state);
}

fn draw_help_overlay(f: &mut Frame, area: Rect) {
    let popup_area = centered_rect(50, 60, area);
    f.render_widget(Clear, popup_area);
    f.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title(" Keybindings ")
            .title_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().bg(Color::Black)),
        popup_area,
    );

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Navigation",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  Tab / Shift+Tab   Next / Previous panel"),
        Line::from("  F1..F4            Jump to panel"),
        Line::from("  j / k / arrows    Navigate list"),
        Line::from(""),
        Line::from(Span::styled(
            "  Actions",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  r                 Run Nexus remediation"),
        Line::from("  ?                 Toggle this help"),
        Line::from("  Esc               Close help"),
        Line::from(""),
        Line::from(Span::styled(
            "  General",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  q / Ctrl+C        Quit"),
    ];

    let inner = Rect {
        x: popup_area.x + 1,
        y: popup_area.y + 1,
        width: popup_area.width.saturating_sub(2),
        height: popup_area.height.saturating_sub(2),
    };
    f.render_widget(
        Paragraph::new(lines).style(Style::default().fg(Color::White)),
        inner,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
