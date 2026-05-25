use crate::scripting::LuaEngine;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use hive_base::telemetry::{Event as HtlEvent, TelemetryBuffer};
use hive_base::{AgentIdentity, HiveChamber, Message, Role};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Gauge, List, ListItem, Paragraph, Row, Table, Tabs};
use ratatui::Frame;
use std::collections::VecDeque;
use std::io::stdout;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const MAX_EVENTS: usize = 500;
const MAX_LOG: usize = 200;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tab {
    Topology,
    Events,
    Consensus,
    Lua,
    Log,
}

impl Tab {
    fn title(&self) -> &str {
        match self {
            Tab::Topology => " Topology ",
            Tab::Events => " HTL Events ",
            Tab::Consensus => " Consensus ",
            Tab::Lua => " Lua Console ",
            Tab::Log => " Log ",
        }
    }
    fn all() -> &'static [Tab; 5] {
        &[Tab::Topology, Tab::Events, Tab::Consensus, Tab::Lua, Tab::Log]
    }
}

struct AppData {
    active_agents: Vec<(uuid::Uuid, Role, u64)>,
    messages: Vec<Message>,
    events: VecDeque<HtlEvent>,
    lua_output: VecDeque<String>,
    lua_input: String,
    log_lines: VecDeque<String>,
    directives: Vec<hive_base::hivemind::HiveDirective>,
    peer_count: usize,
    arena_name: String,
    connected: bool,
    last_update: Instant,
}

pub async fn run_tui(arena_name: &str) {
    let mut lua = LuaEngine::new();
    lua.init();

    let identity = AgentIdentity::new();
    let chamber = HiveChamber::connect(&identity, Role::Queen).await.ok();
    let arena_ptr = std::env::var("__HIVE_ARENA")
        .ok()
        .and_then(|_| {
            let size = hive_base::shared_arena::arena_size();
            let layout = std::alloc::Layout::from_size_align(size, 4096).ok()?;
            let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
            if ptr.is_null() { return None; }
            hive_base::shared_arena::init_arena(ptr);
            let tb = TelemetryBuffer::open(ptr);
            tb.init();
            Some(ptr)
        });

    let data = Arc::new(Mutex::new(AppData {
        active_agents: Vec::new(),
        messages: Vec::new(),
        events: VecDeque::with_capacity(MAX_EVENTS),
        lua_output: VecDeque::with_capacity(MAX_LOG),
        lua_input: String::new(),
        log_lines: VecDeque::with_capacity(MAX_LOG),
        directives: Vec::new(),
        peer_count: 0,
        arena_name: arena_name.to_string(),
        connected: chamber.is_some(),
        last_update: Instant::now(),
    }));

    enable_raw_mode().unwrap();
    stdout().execute(EnterAlternateScreen).unwrap();

    let mut terminal = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(stdout())).unwrap();

    let data_clone = data.clone();
    let chamber_clone = chamber;
    tokio::spawn(async move {
        let mut seq: u64 = 0;
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let mut d = data_clone.lock().await;
            d.last_update = Instant::now();
            if let Some(ref chamber) = chamber_clone {
                let agents: Vec<(uuid::Uuid, Role, u64)> = chamber.get_active_agents(30).await;
                d.active_agents = agents;
                d.messages = chamber.read_new().await;
            }
            d.peer_count = (seq as usize) % 8;
            seq += 1;
        }
    });

    let mut current_tab = Tab::Topology;
    let mut should_quit = false;

    while !should_quit {
        if let Some(ptr) = arena_ptr.as_ref() {
            let tb = TelemetryBuffer::open(*ptr);
            let evts = tb.peek(10);
            if !evts.is_empty() {
                let mut d = data.blocking_lock();
                for e in evts {
                    if d.events.len() >= MAX_EVENTS { d.events.pop_front(); }
                    d.events.push_back(e);
                }
            }
        }

        terminal.draw(|f| {
            let size = f.size();
            if size.width < 80 || size.height < 20 {
                let text = "Terminal too small — resize to at least 80x20";
                f.render_widget(Paragraph::new(text).style(Style::default().fg(Color::Red)), size);
                return;
            }
            render_tui(f, size, &data, current_tab, &mut lua);
        }).unwrap();

        if crossterm::event::poll(Duration::from_millis(100)).unwrap() {
            match crossterm::event::read().unwrap() {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => should_quit = true,
                        KeyCode::Char('1') => current_tab = Tab::Topology,
                        KeyCode::Char('2') => current_tab = Tab::Events,
                        KeyCode::Char('3') => current_tab = Tab::Consensus,
                        KeyCode::Char('4') => current_tab = Tab::Lua,
                        KeyCode::Char('5') => current_tab = Tab::Log,
                        KeyCode::Tab => {
                            let tabs = Tab::all();
                            let idx = tabs.iter().position(|t| *t == current_tab).unwrap_or(0);
                            current_tab = tabs[(idx + 1) % tabs.len()];
                        }
                        KeyCode::Enter if current_tab == Tab::Lua => {
                            let d = data.clone();
                            let input = { d.lock().await.lua_input.clone() };
                            if !input.is_empty() {
                                let result = lua.eval(&input);
                                let mut d = d.lock().await;
                                d.lua_output.push_back(format!("> {}", input));
                                d.lua_output.push_back(result);
                                d.lua_input.clear();
                            }
                        }
                        KeyCode::Backspace if current_tab == Tab::Lua => {
                            let mut d = data.lock().await;
                            d.lua_input.pop();
                        }
                        KeyCode::Char(c) if current_tab == Tab::Lua => {
                            let mut d = data.lock().await;
                            d.lua_input.push(c);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    disable_raw_mode().unwrap();
    stdout().execute(LeaveAlternateScreen).unwrap();
    if let Some(ptr) = arena_ptr.as_ref() {
        let size = hive_base::shared_arena::arena_size();
        let layout = std::alloc::Layout::from_size_align(size, 4096).unwrap();
        unsafe { std::alloc::dealloc(*ptr, layout); }
    }
}

fn render_tui(f: &mut Frame, area: Rect, data: &Arc<Mutex<AppData>>, current_tab: Tab, _lua: &mut LuaEngine) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1), Constraint::Length(6)])
        .split(area);

    let titles: Vec<String> = Tab::all().iter().map(|t| {
        let prefix = match t {
            Tab::Topology => " 1:",
            Tab::Events => " 2:",
            Tab::Consensus => " 3:",
            Tab::Lua => " 4:",
            Tab::Log => " 5:",
        };
        format!("{}{}", prefix, t.title())
    }).collect();
    let tab_refs: Vec<&str> = titles.iter().map(|s| s.as_str()).collect();

    let tabs = Tabs::new(tab_refs)
        .block(Block::default().borders(Borders::ALL).title(" Beekeeper v3.0 "));
    f.render_widget(tabs, chunks[0]);

    match current_tab {
        Tab::Topology => render_topology(f, chunks[1], data),
        Tab::Events => render_events(f, chunks[1], data),
        Tab::Consensus => render_consensus(f, chunks[1], data),
        Tab::Lua => render_lua(f, chunks[1], data),
        Tab::Log => render_log(f, chunks[1], data),
    }

    render_status_bar(f, chunks[2], data, current_tab);
}

fn render_topology(f: &mut Frame, area: Rect, data: &Arc<Mutex<AppData>>) {
    let d = data.try_lock().unwrap_or_else(|_| data.blocking_lock());
    let block = Block::default()
        .title(" Colony Topology ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    let inner = block.inner(area);
    f.render_widget(block, area);

    if d.active_agents.is_empty() {
        let text = Text::from(vec![
            Line::from(Span::styled("  No agents connected", Style::default().fg(Color::DarkGray))),
            Line::from(Span::styled("  Start the colony: docker compose up -d", Style::default().fg(Color::DarkGray))),
        ]);
        f.render_widget(Paragraph::new(text).centered(), inner);
        return;
    }

    let header_cells = ["Agent", "Role", "Uptime", "Status"]
        .iter().map(|h| Cell::from(Span::styled(*h, Style::default().fg(Color::Cyan))));
    let header = Row::new(header_cells)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .height(1);

    let rows: Vec<Row> = d.active_agents.iter().map(|(pid, role, hb)| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
        let uptime = now.saturating_sub(*hb);
        let uptime_str = if uptime < 60 {
            format!("{}s", uptime)
        } else if uptime < 3600 {
            format!("{}m {}s", uptime / 60, uptime % 60)
        } else {
            format!("{}h {}m", uptime / 3600, (uptime % 3600) / 60)
        };
        let status_style = if uptime < 60 {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Yellow)
        };

        let cells = vec![
            Cell::from(format!("{:08x}", pid.as_u128().to_le() as u32)),
            Cell::from(format!("{} {}", crate::role_icon(role), format!("{:?}", role))),
            Cell::from(uptime_str),
            Cell::from(Span::styled("● alive", status_style)),
        ];
        Row::new(cells).height(1)
    }).collect();

    let table_widths = [
        Constraint::Length(12),
        Constraint::Length(14),
        Constraint::Length(10),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, table_widths)
        .header(header)
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(table, inner);
}

fn render_events(f: &mut Frame, area: Rect, data: &Arc<Mutex<AppData>>) {
    let d = data.try_lock().unwrap_or_else(|_| data.blocking_lock());
    let block = Block::default()
        .title(" HTL Event Stream ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    if d.events.is_empty() {
        f.render_widget(block, area);
        return;
    }

    let items: Vec<ListItem> = d.events.iter().rev().take(50).map(|e| {
        let ts = chrono::DateTime::from_timestamp(e.timestamp as i64, 0)
            .map(|t| t.format("%H:%M:%S").to_string())
            .unwrap_or_else(|| "??".into());
        let et = format!("{:?}", e.event_type);
        let event_str = format!("{} [{}]", ts, &et.chars().take(20).collect::<String>());
        ListItem::new(Line::from(Span::raw(event_str)))
    }).collect();

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn render_consensus(f: &mut Frame, area: Rect, data: &Arc<Mutex<AppData>>) {
    let d = data.try_lock().unwrap_or_else(|_| data.blocking_lock());
    let block = Block::default()
        .title(" Consensus & Directives ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    if d.directives.is_empty() {
        let msg = vec![
            Line::from(Span::styled("  No active directives", Style::default().fg(Color::DarkGray))),
            Line::from(Span::styled("  HiveMind must be enabled by the Queen", Style::default().fg(Color::DarkGray))),
        ];
        f.render_widget(Paragraph::new(Text::from(msg)).block(block).centered(), area);
        return;
    }

    let header_cells = ["ID", "Action", "Votes", "Status"]
        .iter().map(|h| Cell::from(Span::styled(*h, Style::default().fg(Color::Cyan))));
    let header = Row::new(header_cells)
        .style(Style::default().add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = d.directives.iter().map(|dir| {
        let status = if dir.approved { "✓ approved" } else { "⏳ pending" };
        let status_style = if dir.approved {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Yellow)
        };
        let cells = vec![
            Cell::from(format!("{:08x}", dir.directive_id.to_u128_le() as u32)),
            Cell::from(dir.action.chars().take(20).collect::<String>()),
            Cell::from("-"),
            Cell::from(Span::styled(status, status_style)),
        ];
        Row::new(cells).height(1)
    }).collect();

    let widths = [Constraint::Length(12), Constraint::Length(22), Constraint::Length(8), Constraint::Length(14)];
    let table = Table::new(rows, widths).header(header).block(Block::default().borders(Borders::NONE));
    f.render_widget(table, area);
}

fn render_lua(f: &mut Frame, area: Rect, data: &Arc<Mutex<AppData>>) {
    let d = data.try_lock().unwrap_or_else(|_| data.blocking_lock());
    let block = Block::default()
        .title(" Lua Console ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    let inner = block.inner(area);
    f.render_widget(block, area);

    let (output_area, input_area) = {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(inner);
        (chunks[0], chunks[1])
    };

    let output_items: Vec<ListItem> = d.lua_output.iter().rev().take(30).map(|line| {
        let style = if line.starts_with('>') {
            Style::default().fg(Color::Green)
        } else if line.starts_with("Err") {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::White)
        };
        ListItem::new(Line::from(Span::styled(line.clone(), style)))
    }).collect();
    let output_list = List::new(output_items).block(Block::default().borders(Borders::NONE));
    f.render_widget(output_list, output_area);

    let input_text = format!("> {}", d.lua_input);
    let input_para = Paragraph::new(input_text.as_str())
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::TOP));
    f.render_widget(input_para, input_area);
}

fn render_log(f: &mut Frame, area: Rect, data: &Arc<Mutex<AppData>>) {
    let d = data.try_lock().unwrap_or_else(|_| data.blocking_lock());
    let block = Block::default()
        .title(" Operator Log ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    let items: Vec<ListItem> = d.log_lines.iter().rev().take(100).map(|line| {
        ListItem::new(Line::from(Span::raw(line.clone())))
    }).collect();
    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn render_status_bar(f: &mut Frame, area: Rect, data: &Arc<Mutex<AppData>>, _tab: Tab) {
    let d = data.try_lock().unwrap_or_else(|_| data.blocking_lock());
    let status = format!(
        " Arena: {} | Agents: {} | Events: {} | Peers: {} | Connected: {} | [1-5] Tab [q] Quit",
        d.arena_name,
        d.active_agents.len(),
        d.events.len(),
        d.peer_count,
        if d.connected { "✓" } else { "✗" },
    );
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(" Status "))
        .label(status)
        .ratio(if d.connected { 1.0 } else { 0.3 });
    f.render_widget(gauge, area);
}
