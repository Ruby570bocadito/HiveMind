use crate::scripting::LuaEngine;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use hive_base::hivemind::HiveDirective;
use hive_base::ldc::{Payload, Value};
use hive_base::telemetry::{Event as HtlEvent, TelemetryBuffer};
use hive_base::{AgentIdentity, HiveChamber, Message, Role};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Borders, Cell, Gauge, List, ListItem, Paragraph, Row, Table, Tabs,
};
use ratatui::Frame;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::stdout;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

const MAX_EVENTS: usize = 500;
const MAX_LOG: usize = 200;
const MAX_DIRECTIVES: usize = 64;

/// Short display form of a UUID (matches the topology table rendering).
fn short_uuid(id: &uuid::Uuid) -> String {
    format!("{:08x}", id.as_u128().to_le() as u32)
}

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
        &[
            Tab::Topology,
            Tab::Events,
            Tab::Consensus,
            Tab::Lua,
            Tab::Log,
        ]
    }
}

struct AppData {
    active_agents: Vec<(uuid::Uuid, Role, u64)>,
    events: VecDeque<HtlEvent>,
    lua_output: VecDeque<String>,
    lua_input: String,
    log_lines: VecDeque<String>,
    /// Observer-side directive state, fed by the live arena message stream
    /// (Proposal / Vote / StatusEvent / Belief payloads).
    directive_state: HashMap<uuid::Uuid, HiveDirective>,
    /// Proposal arrival order, for stable display + oldest-eviction.
    directive_order: VecDeque<uuid::Uuid>,
    /// Agents seen since attach, so topology transitions reach the log.
    seen_agents: HashSet<uuid::Uuid>,
    peer_count: usize,
    arena_name: String,
    connected: bool,
}

impl AppData {
    /// Append a timestamped line to the operator log (bounded).
    fn log(&mut self, line: String) {
        let ts = chrono::Local::now().format("%H:%M:%S");
        self.log_lines.push_back(format!("{} {}", ts, line));
        while self.log_lines.len() > MAX_LOG {
            self.log_lines.pop_front();
        }
    }

    /// Update the observer-side directive state from one arena message.
    /// Mirrors `hivemind::HiveMind::process_arena_message` transitions that
    /// are observable from the wire, without executing anything.
    fn observe_message(&mut self, msg: &Message) {
        match &msg.payload {
            Payload::Proposal {
                action,
                argument,
                proposal_id,
            } => {
                let did = *proposal_id;
                let is_new = !self.directive_state.contains_key(&did);
                if is_new {
                    if self.directive_order.len() >= MAX_DIRECTIVES {
                        if let Some(oldest) = self.directive_order.pop_front() {
                            self.directive_state.remove(&oldest);
                        }
                    }
                    self.directive_order.push_back(did);
                }
                let entry = self
                    .directive_state
                    .entry(did)
                    .or_insert_with(|| HiveDirective {
                        directive_id: did,
                        proposer_id: msg.agent_id,
                        action: action.clone(),
                        params: HashMap::new(),
                        threshold: 0.0,
                        approved: false,
                        executed: false,
                        votes: HashMap::new(),
                    });
                if is_new {
                    entry.params.insert("argument".into(), argument.clone());
                    self.log(format!(
                        "[dir] {} proposed '{}'",
                        short_uuid(&msg.agent_id),
                        action
                    ));
                }
            }
            Payload::Vote {
                proposal_id,
                decision,
                ..
            } => {
                let vote_info = self.directive_state.get_mut(proposal_id).map(|dir| {
                    dir.votes.insert(msg.agent_id, decision.clone());
                    (dir.action.clone(), dir.votes.len())
                });
                if let Some((action, n)) = vote_info {
                    self.log(format!(
                        "[dir] {} voted {:?} on '{}' ({} votes)",
                        short_uuid(&msg.agent_id),
                        decision,
                        action,
                        n
                    ));
                }
            }
            Payload::StatusEvent {
                event_type,
                subject_id,
                detail,
                ..
            } if event_type == "hive_directive_approved" => {
                // The proposal may predate this TUI session — synthesize a
                // minimal entry so the approval is still visible.
                let newly_approved = {
                    let dir =
                        self.directive_state
                            .entry(*subject_id)
                            .or_insert_with(|| HiveDirective {
                                directive_id: *subject_id,
                                proposer_id: msg.agent_id,
                                action: format!(
                                    "(late) {}",
                                    detail.chars().take(24).collect::<String>()
                                ),
                                params: HashMap::new(),
                                threshold: 0.0,
                                approved: false,
                                executed: false,
                                votes: HashMap::new(),
                            });
                    if !dir.approved {
                        dir.approved = true;
                        Some(dir.action.clone())
                    } else {
                        None
                    }
                };
                if let Some(action) = newly_approved {
                    self.log(format!("[dir] '{}' APPROVED", action));
                }
            }
            Payload::Belief { asset, value, .. } if asset.starts_with("directive:") => {
                if let Ok(did) = uuid::Uuid::parse_str(asset.trim_start_matches("directive:")) {
                    if let Value::String(meta) = value {
                        if meta.contains("approved") {
                            let newly_approved =
                                self.directive_state.get_mut(&did).and_then(|dir| {
                                    if !dir.approved {
                                        dir.approved = true;
                                        Some(dir.action.clone())
                                    } else {
                                        None
                                    }
                                });
                            if let Some(action) = newly_approved {
                                self.log(format!("[dir] '{}' APPROVED (belief)", action));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

pub async fn run_tui(arena_name: &str) {
    let mut lua = LuaEngine::new();
    lua.init();

    let identity = AgentIdentity::new();
    let chamber = HiveChamber::connect(&identity, Role::Queen).await.ok();

    // Attach to the SAME arena the colony uses: shm by name when the
    // launcher sets __HIVE_ARENA (same path as every agent), memfd/heap
    // fallback otherwise. The telemetry region lives in the arena — mounting
    // a private zeroed buffer here (the old behavior) left the HTL Events
    // tab permanently empty while a real colony was running.
    //
    // The mapping must outlive every use of `arena_ptr` (its Drop unmaps); a
    // successful HiveChamber::connect already initialized the arena when it
    // was the first process in, so re-init only happens if the chamber path
    // failed but the segment exists.
    let arena_mapping = hive_base::arena_mgr::connect_to_arena().ok();
    let arena_ptr: Option<*mut u8> = arena_mapping.as_ref().map(|m| m.as_ptr());
    if let Some(ptr) = arena_ptr {
        if !hive_base::shared_arena::verify_arena(ptr) {
            // First process in: initialize once. NEVER re-init an already
            // initialized arena — that would reset live cursors and slots.
            hive_base::shared_arena::init_arena(ptr);
            TelemetryBuffer::open(ptr).init();
        }
    }
    // Observer-local telemetry cursor: `read_from` never touches the shared
    // read cursor, so the TUI cannot steal events from the agents' drainers
    // (and cannot re-read the same batch every frame either).
    let mut local_telem_cursor: u64 = 0;

    let data = Arc::new(Mutex::new(AppData {
        active_agents: Vec::new(),
        events: VecDeque::with_capacity(MAX_EVENTS),
        lua_output: VecDeque::with_capacity(MAX_LOG),
        lua_input: String::new(),
        log_lines: VecDeque::with_capacity(MAX_LOG),
        directive_state: HashMap::new(),
        directive_order: VecDeque::with_capacity(MAX_DIRECTIVES),
        seen_agents: HashSet::new(),
        peer_count: 0,
        arena_name: arena_name.to_string(),
        connected: chamber.is_some(),
    }));

    // Seed the operator log with the attach state (real facts only).
    {
        let mut d = data.lock().await;
        d.log("[hive] beekeeper operator console started".to_string());
        d.log(format!(
            "[hive] arena '{}' — chamber {}",
            arena_name,
            if chamber.is_some() {
                "connected"
            } else {
                "NOT connected (start the colony first)"
            }
        ));
    }

    // Restore the terminal even if we panic, otherwise the user is left
    // with a broken shell (no echo, alternate screen stuck).
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
        original_hook(info);
    }));

    enable_raw_mode().unwrap();
    stdout().execute(EnterAlternateScreen).unwrap();

    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(stdout())).unwrap();

    let data_clone = data.clone();
    let chamber_clone = chamber;
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            // Perform arena reads BEFORE taking the lock so the guard is
            // never held across an await point (that would stall the
            // executor and make try_lock() in render fail constantly).
            let (agents, messages) = if let Some(ref chamber) = chamber_clone {
                let agents: Vec<(uuid::Uuid, Role, u64)> = chamber.get_active_agents(30).await;
                let messages = chamber.read_new().await;
                (agents, messages)
            } else {
                (Vec::new(), Vec::new())
            };
            let mut d = data_clone.lock().await;

            // Topology: honest table refresh + join/leave transitions in
            // the operator log.
            let current: HashSet<uuid::Uuid> = agents.iter().map(|(id, _, _)| *id).collect();
            for (id, role, _hb) in &agents {
                if d.seen_agents.insert(*id) {
                    d.log(format!(
                        "[hive] agent {} ({:?}) joined the arena",
                        short_uuid(id),
                        role
                    ));
                }
            }
            for id in d
                .seen_agents
                .difference(&current)
                .copied()
                .collect::<Vec<_>>()
            {
                d.seen_agents.remove(&id);
                d.log(format!("[hive] agent {} left the arena", short_uuid(&id)));
            }
            d.active_agents = agents;

            // Directives: observer-side state fed by the live stream.
            for msg in &messages {
                d.observe_message(msg);
            }

            // Honest peer count: number of agents actually registered in
            // the arena, not a cosmetic counter.
            d.peer_count = d.active_agents.len();
        }
    });

    let mut current_tab = Tab::Topology;
    let mut should_quit = false;

    while !should_quit {
        if let Some(ptr) = arena_ptr.as_ref() {
            let tb = TelemetryBuffer::open(*ptr);
            let (evts, next_pos, skipped) = tb.read_from(local_telem_cursor, 32);
            // Never blocking_lock inside the async runtime: if the poller
            // holds the lock, defer the WHOLE batch to the next frame (the
            // local cursor is only advanced once the events are stored, so
            // nothing is lost or duplicated).
            let Ok(mut d) = data.try_lock() else {
                continue;
            };
            if skipped > 0 {
                d.log(format!(
                    "[hive] telemetry ring lapped: jumped forward {} bytes (older events overwritten by writers)",
                    skipped
                ));
            }
            for e in evts {
                if d.events.len() >= MAX_EVENTS {
                    d.events.pop_front();
                }
                d.events.push_back(e);
            }
            local_telem_cursor = next_pos;
        }

        terminal
            .draw(|f| {
                let size = f.size();
                if size.width < 80 || size.height < 20 {
                    let text = "Terminal too small — resize to at least 80x20";
                    f.render_widget(
                        Paragraph::new(text).style(Style::default().fg(Color::Red)),
                        size,
                    );
                    return;
                }
                render_tui(f, size, &data, current_tab, &mut lua);
            })
            .unwrap();

        if crossterm::event::poll(Duration::from_millis(100)).unwrap() {
            match crossterm::event::read().unwrap() {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
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
                            // Bounded output: VecDeque::with_capacity only
                            // preallocates — without this the console
                            // grows unbounded over long sessions.
                            while d.lua_output.len() > MAX_LOG {
                                d.lua_output.pop_front();
                            }
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
                },
                _ => {}
            }
        }
    }

    disable_raw_mode().unwrap();
    stdout().execute(LeaveAlternateScreen).unwrap();
    // arena_mapping (and any private fallback memory) is released by Drop:
    // SharedArenaMapping munmaps the shm mapping / deallocates the heap
    // arena. The old manual dealloc here was only correct for the private
    // heap-arena path and would have been a double-free/UAF hazard on the
    // shm path.
}

fn render_tui(
    f: &mut Frame,
    area: Rect,
    data: &Arc<Mutex<AppData>>,
    current_tab: Tab,
    _lua: &mut LuaEngine,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(6),
        ])
        .split(area);

    let titles: Vec<String> = Tab::all()
        .iter()
        .map(|t| {
            let prefix = match t {
                Tab::Topology => " 1:",
                Tab::Events => " 2:",
                Tab::Consensus => " 3:",
                Tab::Lua => " 4:",
                Tab::Log => " 5:",
            };
            format!("{}{}", prefix, t.title())
        })
        .collect();
    let tab_refs: Vec<&str> = titles.iter().map(|s| s.as_str()).collect();

    let tabs = Tabs::new(tab_refs).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Beekeeper v3.0 "),
    );
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
    // Skip this frame if the data lock is contended — rendering must
    // never block (blocking_lock panics inside the tokio runtime).
    let Ok(d) = data.try_lock() else {
        return;
    };
    let block = Block::default()
        .title(" Colony Topology ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    let inner = block.inner(area);
    f.render_widget(block, area);

    if d.active_agents.is_empty() {
        let text = Text::from(vec![
            Line::from(Span::styled(
                "  No agents connected",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "  Start the colony: docker compose up -d",
                Style::default().fg(Color::DarkGray),
            )),
        ]);
        f.render_widget(Paragraph::new(text).centered(), inner);
        return;
    }

    let header_cells = ["Agent", "Role", "Uptime", "Status"]
        .iter()
        .map(|h| Cell::from(Span::styled(*h, Style::default().fg(Color::Cyan))));
    let header = Row::new(header_cells)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .height(1);

    let rows: Vec<Row> = d
        .active_agents
        .iter()
        .map(|(pid, role, hb)| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
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
                Cell::from(short_uuid(pid)),
                Cell::from(format!("{} {:?}", crate::role_icon(role), role)),
                Cell::from(uptime_str),
                Cell::from(Span::styled("● alive", status_style)),
            ];
            Row::new(cells).height(1)
        })
        .collect();

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
    // Skip this frame if the data lock is contended — rendering must
    // never block (blocking_lock panics inside the tokio runtime).
    let Ok(d) = data.try_lock() else {
        return;
    };
    let block = Block::default()
        .title(" HTL Event Stream ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    if d.events.is_empty() {
        f.render_widget(block, area);
        return;
    }

    let items: Vec<ListItem> = d
        .events
        .iter()
        .rev()
        .take(50)
        .map(|e| {
            let ts = chrono::DateTime::from_timestamp(e.timestamp as i64, 0)
                .map(|t| t.format("%H:%M:%S").to_string())
                .unwrap_or_else(|| "??".into());
            let et = format!("{:?}", e.event_type);
            let event_str = format!("{} [{}]", ts, et.chars().take(20).collect::<String>());
            ListItem::new(Line::from(Span::raw(event_str)))
        })
        .collect();

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn render_consensus(f: &mut Frame, area: Rect, data: &Arc<Mutex<AppData>>) {
    // Skip this frame if the data lock is contended — rendering must
    // never block (blocking_lock panics inside the tokio runtime).
    let Ok(d) = data.try_lock() else {
        return;
    };
    let block = Block::default()
        .title(" Consensus & Directives ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    if d.directive_order.is_empty() {
        let msg = vec![
            Line::from(Span::styled(
                "  No directives observed on the arena yet",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "  Proposals and votes appear here as they circulate",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        f.render_widget(
            Paragraph::new(Text::from(msg)).block(block).centered(),
            area,
        );
        return;
    }

    let header_cells = ["ID", "Action", "Votes", "Status"]
        .iter()
        .map(|h| Cell::from(Span::styled(*h, Style::default().fg(Color::Cyan))));
    let header = Row::new(header_cells)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .height(1);

    // Newest proposals first (the order deque is insertion-ordered).
    let rows: Vec<Row> = d
        .directive_order
        .iter()
        .rev()
        .take(50)
        .filter_map(|id| d.directive_state.get(id))
        .map(|dir| {
            let status = if dir.approved {
                "✓ approved"
            } else {
                "⏳ pending"
            };
            let status_style = if dir.approved {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Yellow)
            };
            let cells = vec![
                Cell::from(short_uuid(&dir.directive_id)),
                Cell::from(dir.action.chars().take(20).collect::<String>()),
                Cell::from(dir.votes.len().to_string()),
                Cell::from(Span::styled(status, status_style)),
            ];
            Row::new(cells).height(1)
        })
        .collect();

    let widths = [
        Constraint::Length(12),
        Constraint::Length(22),
        Constraint::Length(8),
        Constraint::Length(14),
    ];
    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(table, area);
}

fn render_lua(f: &mut Frame, area: Rect, data: &Arc<Mutex<AppData>>) {
    // Skip this frame if the data lock is contended — rendering must
    // never block (blocking_lock panics inside the tokio runtime).
    let Ok(d) = data.try_lock() else {
        return;
    };
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

    let output_items: Vec<ListItem> = d
        .lua_output
        .iter()
        .rev()
        .take(30)
        .map(|line| {
            let style = if line.starts_with('>') {
                Style::default().fg(Color::Green)
            } else if line.starts_with("Err") {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(line.clone(), style)))
        })
        .collect();
    let output_list = List::new(output_items).block(Block::default().borders(Borders::NONE));
    f.render_widget(output_list, output_area);

    let input_text = format!("> {}", d.lua_input);
    let input_para = Paragraph::new(input_text.as_str())
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::TOP));
    f.render_widget(input_para, input_area);
}

fn render_log(f: &mut Frame, area: Rect, data: &Arc<Mutex<AppData>>) {
    // Skip this frame if the data lock is contended — rendering must
    // never block (blocking_lock panics inside the tokio runtime).
    let Ok(d) = data.try_lock() else {
        return;
    };
    let block = Block::default()
        .title(" Operator Log ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    let items: Vec<ListItem> = d
        .log_lines
        .iter()
        .rev()
        .take(100)
        .map(|line| ListItem::new(Line::from(Span::raw(line.clone()))))
        .collect();
    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn render_status_bar(f: &mut Frame, area: Rect, data: &Arc<Mutex<AppData>>, _tab: Tab) {
    // Skip this frame if the data lock is contended — rendering must
    // never block (blocking_lock panics inside the tokio runtime).
    let Ok(d) = data.try_lock() else {
        return;
    };
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
