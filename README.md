# Hive Colony v3.0

```
 __         .' '.
        _/__)        .   .       .
       (8|)_}}- .      .        .
jgs     `\__)    '. . ' ' .  . '
    H I V E   C O L O N Y
```

**Bee-inspired multi-agent autonomous Red Team framework.**

[Full Documentation](docs/README.md) | [Operator Guide](docs/OPERATOR_GUIDE.md) | [API Reference](docs/API.md) | [MITRE Coverage](docs/MITRE_MAPPING.md)

---

## Quick Start

```bash
source build_env.sh
./hive.sh brain     # Operator mode: C2 + dashboard only
./hive.sh colony    # Aggressive colony: attacks all reachable hosts
./hive.sh deploy IP # Deploy to victim
./hive.sh status    # See running agents
./hive.sh test      # 38 tests
```

**Dashboard:** `http://localhost:8080` | **C2 API:** `http://localhost:8443/health`

## 6 Agent Types

| Agent | Role | Capability |
|-------|------|-----------|
| Worker ◈ | Recon | EDR detection (8 vendors), process scanning, system profiling |
| Drone ◆ | Spread | SSH lateral movement, network discovery, agent regeneration |
| Honeybee ◉ | Action | AES-256-GCM encryption, 3-pass wipe, C2 exfiltration |
| Weaver ✦ | Obfuscation | 4 mutation techniques, polymorphic variants |
| Queen ◇ | Strategy | Ollama LLM, C2 bridge (Sliver/CS/HTTP), Royal Jelly directives |
| Swarm ⬡ | Autonomous | Self-spreading via SSH, MARL target selection |

## 10-Layer Evasion

1. Shared memory IPC (no TCP) · 2. Fileless `memfd_create` · 3. Direct ASM syscalls · 4. Call stack spoofing · 5. Encrypted ONNX models · 6. Anti-debug · 7. Anti-sandbox · 8. Anti-VM · 9. String obfuscation · 10. Honey detection

## MITRE ATT&CK

36 techniques across 10 tactics.

## Requirements

- Rust, OpenSSL dev, Python 3, Linux 3.17+
- Optional: Ollama, nmap

## License

Research & educational use only.
