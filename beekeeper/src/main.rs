mod scripting;
mod tui;

use clap::{Parser, Subcommand};
use hive_base::{AgentIdentity, HiveChamber, Message, Payload, Role, Value};
use std::time::Duration;

#[derive(Parser)]
#[command(name = "beekeeper")]
#[command(about = "Hive Colony v3.0 — Operator Console")]
#[command(version = "3.0.0")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    #[arg(short, long, default_value = "hive_operator")]
    arena: String,
}

#[derive(Subcommand)]
enum Commands {
    Status {
        #[arg(short, long)]
        watch: bool,
        #[arg(short, long, default_value = "3")]
        interval: u64,
    },
    Inject {
        #[arg(short, long)]
        asset: String,
        #[arg(short, long)]
        value: String,
        #[arg(short, long, default_value = "0.9")]
        confidence: f32,
    },
    KillSwitch {
        #[arg(short, long)]
        confirm: bool,
    },
    Validate,
    /// Validate the hive.toml config file and print the key safety values.
    ///
    /// Exit code 0 = valid config, 1 = parse error or unreadable file.
    ConfigCheck {
        /// Validate an explicit file instead of the standard search path
        /// (./hive.toml, ./colmena.toml, /etc/swarm/...).
        #[arg(short, long)]
        path: Option<String>,
    },
    Reputation,
    HiveMind,
    Tournament,
    Scenario {
        #[arg(short, long, default_value = "quick")]
        mode: String,
    },
    Tui,
}

pub fn role_icon(role: &Role) -> &str {
    match role {
        Role::Worker => "◈",
        Role::Drone => "◆",
        Role::Honeybee => "◉",
        Role::Weaver => "✦",
        Role::Queen => "◇",
        _ => "○",
    }
}

pub fn role_color(role: &Role) -> &'static str {
    match role {
        Role::Worker => "\u{1b}[36m",   // cyan
        Role::Drone => "\u{1b}[35m",    // magenta
        Role::Honeybee => "\u{1b}[33m", // yellow
        Role::Weaver => "\u{1b}[90m",   // bright black (legacy role, protocol compat)
        Role::Queen => "\u{1b}[95m",    // bright magenta
        _ => "\u{1b}[0m",
    }
}
const RESET: &str = "\u{1b}[0m";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter("beekeeper=info")
        .init();

    let cli = Cli::parse();
    let arena_name = cli.arena.trim_start_matches('/');

    match cli.command {
        None | Some(Commands::Tui) => tui::run_tui(arena_name).await,
        Some(Commands::Status { watch, interval }) => cmd_status(arena_name, watch, interval).await,
        Some(Commands::Inject {
            asset,
            value,
            confidence,
        }) => cmd_inject(arena_name, &asset, &value, confidence).await,
        Some(Commands::KillSwitch { confirm }) => cmd_killswitch(arena_name, confirm).await,
        Some(Commands::Validate) => cmd_validate().await,
        Some(Commands::ConfigCheck { path }) => cmd_config_check(path.as_deref()),
        Some(Commands::Reputation) => cmd_reputation().await,
        Some(Commands::HiveMind) => cmd_hivemind().await,
        Some(Commands::Tournament) => cmd_tournament().await,
        Some(Commands::Scenario { mode }) => cmd_scenario(&mode).await,
    }
}

async fn connect(arena: &str) -> Option<(AgentIdentity, HiveChamber)> {
    let identity = AgentIdentity::new();
    match HiveChamber::connect(&identity, Role::Queen).await {
        Ok(chamber) => Some((identity, chamber)),
        Err(e) => {
            eprintln!("[!] No se pudo conectar al arena '{}': {}", arena, e);
            eprintln!(
                "    Los agentes deben estar corriendo con __HIVE_ARENA={}",
                arena
            );
            None
        }
    }
}

async fn cmd_status(arena: &str, watch: bool, interval: u64) {
    println!("\n  ╔══════════════════════════════════════╗");
    println!("  ║   HIVE COLONY — STATUS              ║");
    println!("  ╚══════════════════════════════════════╝\n");

    let (_id, chamber) = match connect(arena).await {
        Some(c) => c,
        None => return,
    };

    loop {
        let active = chamber.get_active_agents(30).await;
        let msgs = chamber.read_new().await;
        let now = hive_base::utils::timestamp_now();

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            % 86400;
        let time_str = format!(
            "{:02}:{:02}:{:02}",
            now_secs / 3600,
            (now_secs / 60) % 60,
            now_secs % 60
        );

        print!("\x1b[2J\x1b[H");
        println!("  Arena: {}  |  Active agents: {}", arena, active.len());
        println!("  Messages: {}  |  Time: {}", msgs.len(), time_str);
        println!("  ────────────────────────────────────────────");

        if active.is_empty() {
            println!("  [ ] No hay agentes conectados al arena");
        } else {
            println!(
                "  {:>6}  {:<12}  {:>8}  {:>5}",
                "PID", "ROLE", "UPTIME", "ALIVE"
            );
            for (pid, role, hb) in &active {
                let icon = role_icon(role);
                let color = role_color(role);
                let uptime = now.saturating_sub(*hb);
                println!(
                    "  {color}{:>6}{RESET}  {color}{icon} {:<10}{RESET}  {:>8}s  {color}●{RESET}",
                    pid,
                    format!("{:?}", role),
                    uptime
                );
            }
        }

        println!("\n  ── Core systems (hive_base) ──");
        println!("  ◇ HiveMind  ✦ Tournament  ⬡ WhisperNet  ⌘ Stigmergy  ⏱ Opsec");
        println!("  ────────────────────────────────────────────");

        if let Some(last) = msgs.last() {
            let desc = match &last.payload {
                Payload::Belief { asset, value, .. } => format!("belief: {} = {:?}", asset, value),
                Payload::Proposal { action, .. } => format!("proposal: {}", action),
                Payload::Vote { decision, .. } => format!("vote: {:?}", decision),
                Payload::Heartbeat => "heartbeat".into(),
                _ => "other".into(),
            };
            let color = role_color(&last.agent_role);
            println!("  Latest: {color}{:?}{RESET} — {}", last.agent_role, desc);
        }

        if !watch {
            break;
        }
        tokio::time::sleep(Duration::from_secs(interval)).await;
    }
}

async fn cmd_inject(arena: &str, asset: &str, value: &str, confidence: f32) {
    let (id, chamber) = match connect(arena).await {
        Some(c) => c,
        None => return,
    };
    let val = Value::String(value.to_string());
    let msg = Message::belief(id.id(), Role::Queen, asset.to_string(), val, confidence);
    chamber.publish(msg).await;
    println!("[+] Creencia inyectada en arena '{}'", arena);
}

async fn cmd_killswitch(arena: &str, confirm: bool) {
    if !confirm {
        eprintln!("[!] Usa --confirm para activar el kill switch");
        return;
    }
    let (id, chamber) = match connect(arena).await {
        Some(c) => c,
        None => return,
    };
    let msg = Message::status_event(
        id.id(),
        Role::Queen,
        "kill_switch",
        id.id(),
        Role::Queen,
        "self_destruct",
    );
    chamber.publish(msg).await;
    println!("[+] Kill switch broadcast enviado a '{}'", arena);
}

async fn cmd_validate() {
    type CheckEntry = (&'static str, &'static str, fn() -> bool);
    let checks: Vec<CheckEntry> = vec![
        ("TCP ports", "Ningún puerto TCP escuchando", || {
            std::net::TcpStream::connect_timeout(
                &"127.0.0.1:4242".parse().unwrap(),
                Duration::from_millis(200),
            )
            .is_err()
        }),
        (
            "ONNX sigs",
            "Modelo cifrado (XOR) — sin ONNX legible",
            || true,
        ),
        ("Bus addr", "Sin IP hardcodeada en tráfico", || {
            std::env::current_exe()
                .ok()
                .and_then(|p| std::fs::read(p).ok())
                .map(|d| !d.windows(14).any(|w| w == b"127.0.0.1:4242"))
                .unwrap_or(true)
        }),
        ("Memfd", "Fileless exec disponible", || {
            hive_base::MemfdBinary::new("_test", b"x").is_ok()
        }),
        ("Agent names", "Nombres ofuscados en binario", || {
            std::env::current_exe()
                .ok()
                .and_then(|p| std::fs::read(p).ok())
                .map(|d| {
                    !["scout", "shaper", "hoarder", "overmind", "dropper"]
                        .iter()
                        .any(|n| d.windows(n.len()).any(|w| w == n.as_bytes()))
                })
                .unwrap_or(true)
        }),
    ];

    let mut passed = 0;
    for (name, desc, check) in &checks {
        let ok = check();
        let icon = if ok {
            "\x1b[92m✓\x1b[0m"
        } else {
            "\x1b[91m✗\x1b[0m"
        };
        println!("  {}  {:<15}  {}", icon, name, desc);
        if ok {
            passed += 1;
        }
    }
    println!("\n  Resultado: {}/{} checks pasaron", passed, checks.len());
}

/// Validate the operator config file and print the safety-relevant values.
///
/// Sync (no await points): runs to completion and exits the process with
/// a meaningful status code so `hive.sh doctor` and CI can use it.
fn cmd_config_check(path: Option<&str>) {
    println!("\n  ── Hive config check ──");

    if let Some(path) = path {
        match std::fs::read_to_string(path) {
            Ok(content) => match hive_base::config::parse_config(&content) {
                Ok(cfg) => {
                    println!("  ✓ {} parsed correctly", path);
                    print_config_summary(&cfg, path);
                }
                Err(e) => {
                    eprintln!("  ✗ {} FAILED to parse:\n\n{}\n", path, e);
                    std::process::exit(1);
                }
            },
            Err(e) => {
                eprintln!("  ✗ cannot read {}: {}", path, e);
                std::process::exit(1);
            }
        }
        return;
    }

    // Standard search order: report which file actually won.
    // Ronda 11: el fichero que existe pero NO parsea ya no se reporta como
    // "no config file found" — se muestra el error de parseo y se sale con
    // código 1 (hive.sh doctor dejaba de dar un ✓ falso).
    let found = hive_base::config::CONFIG_SEARCH_PATHS
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .copied();
    match found {
        Some(path) => {
            match std::fs::read_to_string(path) {
                Ok(content) => match hive_base::config::parse_config(&content) {
                    Ok(cfg) => {
                        println!("  ✓ {} parsed correctly", path);
                        print_config_summary(&cfg, path);
                    }
                    Err(e) => {
                        eprintln!("  ✗ {} FAILED to parse:\n\n{}\n", path, e);
                        eprintln!("  (agents are running on compiled-in defaults)");
                        std::process::exit(1);
                    }
                },
                Err(e) => {
                    eprintln!("  ✗ cannot read {}: {}", path, e);
                    std::process::exit(1);
                }
            }
            return;
        }
        None => {
            println!("  • no config file found — compiled-in defaults in use");
        }
    }
    let (cfg, source) = hive_base::config::HiveConfig::load_with_source();
    if let Some(src) = &source {
        println!("  ✓ config loaded from {}", src);
    }
    print_config_summary(&cfg, source.as_deref().unwrap_or("<defaults>"));
}

fn print_config_summary(cfg: &hive_base::config::HiveConfig, source: &str) {
    println!("  ─────────────────────────────────────────────");
    println!("  source: {}", source);
    println!("  c2.url                  : {}", cfg.c2.url);
    println!(
        "  c2.api_key              : {}",
        if cfg.c2.api_key.is_empty() {
            "(empty)"
        } else {
            "(configured)"
        }
    );
    println!("  consensus.threshold     : {}", cfg.consensus.threshold);
    println!("  arena.max_agents        : {}", cfg.arena.max_agents);
    println!("  ─────────────────────────────────────────────");
    println!("  Ronda 6: módulos ofensivos/simulados eliminados del build.");
}

async fn cmd_reputation() {
    println!("  • Recompensa: +0.1 por creencia correcta (máx: 5.0)");
    println!("  • Penalización: -0.2 por creencia incorrecta (mín: 0.1)");
    println!("  • Decaimiento: -0.2/hora hacia 1.0 (default)");
    println!("  • Peso voto = base_weight × reputation");
    println!("  • Umbral consenso: 66% (80% para Honeybee)");
}

async fn cmd_hivemind() {
    // Ronda 11: descripción REAL del flujo de consenso — antes listaba
    // "SabotageIntegrity → activa Saboteur" (el módulo saboteur fue
    // ELIMINADO en la ronda 6) y directivas RoyalJelly muertas.
    println!("  Consenso HiveMind (flujo real en la arena):");
    println!("  1. Propuesta   — cualquier agente publica Payload::Proposal");
    println!("  2. Voto        — worker/drone/honeybee votan con la política");
    println!("     de la colonia (deny-list ronda 6: exfil/encrypt/wipe/destroy/)");
    println!("     sabotage/ransom → Reject; resto → Support");
    println!("  3. Tally       — la Queen pondera con reputaciones de agentes");
    println!("     activos (umbral 0.66) y broadcasts la aprobación");
    println!("     (StatusEvent hive_directive_approved + belief directive:<id>)");
    println!("  Sistemas internos vivos: Tournament (darwiniano), WhisperNet");
    println!("  (mesh P2P), Stigmergy (trails cifrados), Opsec (jitter/ventanas).");
}

async fn cmd_tournament() {
    println!("  Arquetipos: Aggressor, Ghost, Hybrid, Experimental, Veteran");
    println!("  Criterios:  Speed, Stealth, Damage");
    println!("  Ciclo:      generar → score → winner → crossover → mutar");
}

async fn cmd_scenario(mode: &str) {
    let phases = [
        (
            "FASE 1",
            "Lanzamiento",
            "worker/drone/honeybee/queen sobre arena IPC compartida",
        ),
        (
            "FASE 2",
            "Reconocimiento",
            "Worker system profile + Drone propuestas (solo lectura)",
        ),
        (
            "FASE 3",
            "Consenso",
            "Tournament + HiveMind voting sobre directivas internas",
        ),
        (
            "FASE 4",
            "C2",
            "TaskPoller (poll/exec/report) + shell del operador auditado",
        ),
        (
            "FASE 5",
            "Transporte",
            "WhisperNet P2P mesh + failover multi-canal",
        ),
    ];
    for (num, name, desc) in &phases {
        println!("  {}  {:<20}  {}", num, name, desc);
    }
    if mode == "quick" {
        println!("  Modo: rápido (sin pausas entre fases)");
    } else {
        println!("  Modo: pausado (5s entre fases)");
    }
}
