<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://capsule-render.vercel.app/api?type=waving&color=0:0d1117,50:00ff88,100:0d1117&height=200&section=header&text=HiveMind&fontSize=70&fontColor=00ff88&animation=fadeIn&fontAlignY=35">
    <source media="(prefers-color-scheme: light)" srcset="https://capsule-render.vercel.app/api?type=waving&color=0:ffffff,50:00cc77,100:ffffff&height=200&section=header&text=HiveMind&fontSize=70&fontColor=00cc77&animation=fadeIn&fontAlignY=35">
    <img src="https://capsule-render.vercel.app/api?type=waving&color=0:0d1117,50:00ff88,100:0d1117&height=200&section=header&text=HiveMind&fontSize=70&fontColor=00ff88&animation=fadeIn&fontAlignY=35" alt="HiveMind">
  </picture>
  <br>
  <img src="https://readme-typing-svg.demolab.com?font=Fira+Code&weight=600&size=22&duration=3000&pause=700&color=00FF88&center=true&vCenter=true&width=600&lines=Rust-based+post-exploitation+framework;Encrypted+beaconing+%7C+Modular+payloads;Cross-platform+persistence+agents" alt="Typing SVG">
  <br><br>
  <p><b>HiveMind</b> — <i>formerly <code>hive-colony</code></i> — is a Rust-powered post-exploitation framework designed for lightweight implants, encrypted C2 beaconing, modular payload delivery, and cross-platform persistence.</p>
  <br>
  <p>
    <a href="https://github.com/Ruby570bocadito/HiveMind/actions"><img src="https://img.shields.io/github/actions/workflow/status/Ruby570bocadito/HiveMind/ci.yml?branch=master&style=for-the-badge&logo=githubactions&logoColor=white&label=BUILD" alt="Build"></a>
    <a href="https://github.com/Ruby570bocadito/HiveMind/releases"><img src="https://img.shields.io/github/v/release/Ruby570bocadito/HiveMind?style=for-the-badge&logo=rust&logoColor=white&label=VERSION&color=00ff88" alt="Release"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/LICENSE-MIT%2FApache_2.0-00aa66?style=for-the-badge&logo=opensourceinitiative&logoColor=white" alt="License"></a>
    <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/RUST-1.70%2B-ff6600?style=for-the-badge&logo=rust&logoColor=white" alt="Rust"></a>
    <a href="https://github.com/Ruby570bocadito/HiveMind"><img src="https://img.shields.io/github/stars/Ruby570bocadito/HiveMind?style=for-the-badge&logo=github&logoColor=white&label=STARS&color=ffcc00" alt="Stars"></a>
    <a href="https://github.com/Ruby570bocadito/HiveMind/issues"><img src="https://img.shields.io/github/issues/Ruby570bocadito/HiveMind?style=for-the-badge&logo=github&logoColor=white&label=ISSUES&color=ff4444" alt="Issues"></a>
  </p>
  <br>
  <p>
    <a href="docs/DEPLOYMENT.md">📦 Deployment</a> •
    <a href="docs/OPERATOR_GUIDE.md">🎮 Operator Guide</a> •
    <a href="docs/AGENTS.md">🤖 Agents</a> •
    <a href="docs/MITRE_MAPPING.md">🛡️ MITRE ATT&CK</a> •
    <a href="docs/EVASION.md">👻 Evasion</a>
  </p>
  <br>
  <img src="https://capsule-render.vercel.app/api?type=rect&color=0:00ff88,100:00aa66&height=2&width=100%&section=separator">
</div>

<br>

## 📐 Architecture

```mermaid
%%{init: {'theme': 'dark', 'themeVariables': { 'primaryColor': '#0d1117', 'primaryBorderColor': '#00ff88', 'secondaryColor': '#161b22', 'lineColor': '#00ff88', 'tertiaryColor': '#0d1117'}}}%%
graph TB
    subgraph C2["🐝 C2 Server"]
        HTTP["🌐 HTTP(S) Listener"]
        DNS["📡 DNS Tunnel"]
        ICMP["📶 ICMP Tunnel"]
        DROP["📤 Dead Drop (Gist/Pastebin/S3)"]
    end

    subgraph IMPLANT["🧬 Implant"]
        IM_CORE["Core Beacon"]
        IM_ENCRYPT["🔐 AES-256-GCM / XChaCha20"]
        IM_FINGER["Fingerprinting"]
        IM_MOD["Module Loader"]
    end

    subgraph MODULES["🧩 Modules"]
        M1["syscalls<br/><i>Hell's Gate / Halos Gate</i>"]
        M2["fileless<br/><i>memfd_create / NtCreateSection</i>"]
        M3["stack_spoof<br/><i>ret-spoofing + ROP chain</i>"]
        M4["antianalysis<br/><i>PEB BeingDebugged / ptrace</i>"]
        M5["antisandbox<br/><i>CPU/USER/tempo checks</i>"]
        M6["edr<br/><i>30+ EDR signatures</i>"]
        M7["obfuscation<br/><i>string + API hashing</i>"]
        M8["persistence<br/><i>Registry / crontab / .service</i>"]
    end

    subgraph PAYLOAD["📦 Payload Vectors"]
        P1["Network<br/>stager+bash+payload.b64"]
        P2["USB<br/>install.sh + manifest.dat"]
        P3["Phishing<br/>HTML + VBA macro"]
        P4["EXE<br/>C# loader + encrypted"]
    end

    C2 -->|beacon / task| IMPLANT
    IMPLANT -->|loader| MODULES
    IMPLANT -->|delivery| PAYLOAD
    PAYLOAD -->|callback| C2

    classDef highlight fill:#0d1117,stroke:#00ff88,stroke-width:2px,color:#fff
    class C2,IMPLANT,MODULES highlight
```

<br>

## ⚡ Quick Start

```bash
# 1. Build the framework
cargo build --release --workspace

# 2. Generate all payloads
./scripts/deploy.sh all

# 3. Launch local colony
./scripts/launch_colony.sh

# 4. Build a monolithic payload (auto-extract)
./scripts/build_payload.sh
```

<br>

## 🧩 Module Matrix

| Module | Technique | Status |
|--------|-----------|--------|
| `syscalls` | Hell's Gate / Halos Gate / indirect syscall | ✅ Linux + Windows |
| `fileless` | `memfd_create` / `NtCreateSection` + `NtMapViewOfSection` | ✅ Linux + Windows |
| `stack_spoof` | ret-spoofing + ROP chain | ✅ Linux + Windows |
| `antianalysis` | PEB BeingDebugged / `ptrace` / `TracerPid` | ✅ Linux + Windows |
| `antisandbox` | USER / CPU / uptime checks | ✅ Linux + Windows |
| `edr` | 30+ EDR signatures (Defender, CrowdStrike, SentinelOne…) | ✅ Windows |
| `obfuscation` | String obfuscation + API hashing | ✅ |
| `persistence` | Registry Run / Startup / SchTasks / WMI / crontab / systemd | ✅ |
| `phoenix` | Adaptive hibernation + channel rotation | ✅ |
| `sleepmask` | Sleep mask with heap encryption + stack permutation | ✅ |
| `indirect_syscall` | Dynamic SSN resolution + syscall stub | ✅ |
| `etw_patch` | ETW / ETW-TI patching (3 techniques) | ✅ |
| `ppid_spoof` | PPID spoofing via `NtCreateUserProcess` | ✅ |
| `clr_hijack` | CLR (.NET) hijacking for in-memory C# execution | ✅ |
| `sgn_encode` | Shikata-Ga-Nai encoder | ✅ |

<br>

## 🌐 Communications

### Internal (Mesh)
```
Worker  ──┬───┐
Drone   ──┼───┼──── Shared Arena (shm_open / mmap) ────┐  sin red, sin puertos
Honeybee ─┘   │   16 slots lock-free, MessagePack, 8KB msgs
Weaver  ──────┘
```

### External (C2)
```
Queen ─── HTTP(S) ─── C2 Server
         ├── DNS Tunnel ─── (txt records)
         ├── ICMP Tunnel ─── (raw socket)
         └── Dead Drop ─── Gist / Pastebin / S3
Failover: Priority → Race → RoundRobin with exponential backoff
```

<br>

## 🛡️ MITRE ATT&CK Coverage

36+ techniques across 10+ tactics. [View full mapping](docs/MITRE_MAPPING.md).

| Tactic | Techniques |
|--------|-----------|
| **Execution** | T1059, T1204, T1106, T1569 |
| **Persistence** | T1547, T1053, T1546 |
| **Defense Evasion** | T1564, T1055, T1140, T1027, T1620, T1562, T1070 |
| **Credential Access** | T1056, T1555 |
| **Discovery** | T1082, T1083, T1057, T1012, T1069, T1046 |
| **Collection** | T1115, T1056, T1074 |
| **Command & Control** | T1071, T1573, T1572, T1008, T1571, T1095 |
| **Exfiltration** | T1041, T1567, T1029 |

<br>

## 📂 Project Structure

```
src/
├── queen/             # C2 server (HTTPS / DNS / ICMP / Dead Drop)
├── worker/            # Reconnaissance & EDR detection agent
├── drone/             # Lateral movement & SSH agent
├── honeybee/          # Final execution: encryption, wipe, exfil
├── weaver/            # Morphing / process hollowing agent
├── swarm/             # Auto-propagation & MARL target selection
├── common/            # Shared crypto, IPC, config
scripts/
├── deploy.sh          # Payload generator (4 vectors)
├── build_payload.sh   # Monolithic auto-extract payload
├── launch_colony.sh   # Local colony launcher (Docker)
└── obfuscate_pe.py    # PE obfuscator v2.2
```

<br>

## 🐳 Docker

```bash
docker compose up -d
docker compose logs -f
docker compose down
```

<br>

## 📋 Requirements

| Dependency | Version | Notes |
|------------|---------|-------|
| **Rust** | 1.70+ | Stable channel |
| **OpenSSL** | dev | `apt install libssl-dev pkg-config` |
| **Python** | 3.10+ | See `requirements.txt` |
| **Linux** | 3.17+ | Kernel with `shm_open` |
| **Docker** | latest | Optional |
| **Optional** | — | Ollama, nmap, mingw-w64 |

<br>

## ⚖️ License

Dual-licensed under [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE).

---

<div align="center">
  <br>
  <img src="https://capsule-render.vercel.app/api?type=shrek&color=gradient&customColorList=2,3,6&height=120&section=footer&text=HiveMind%20%E2%80%93%20Research%20Only&fontSize=24&fontColor=00ff88&animation=twinkling">
  <br><br>
  <sub>
    <b>⚠️ HiveMind is intended exclusively for <b>authorized security research</b> and <b>defensive education</b>.</b><br>
    Unauthorized use on systems without explicit written permission is prohibited.<br><br>
    <a href="https://github.com/Ruby570bocadito"><img src="https://img.shields.io/badge/%40Ruby570bocadito-0d1117?style=flat-square&logo=github"></a>
    &nbsp;&nbsp;
    <a href="https://github.com/Ruby570bocadito/HiveMind/blob/master/docs/SECURITY.md"><img src="https://img.shields.io/badge/Security-00ff88?style=flat-square&logo=shield"></a>
    &nbsp;&nbsp;
    <a href="https://github.com/Ruby570bocadito/HiveMind/blob/master/CONTRIBUTING.md"><img src="https://img.shields.io/badge/Contributing-00aa66?style=flat-square&logo=git"></a>
    <br><br>
    <img src="https://img.shields.io/github/last-commit/Ruby570bocadito/HiveMind?style=flat-square&label=last%20commit&color=555">
    &nbsp;
    <img src="https://img.shields.io/github/repo-size/Ruby570bocadito/HiveMind?style=flat-square&label=size&color=555">
    &nbsp;
    <img src="https://img.shields.io/github/languages/count/Ruby570bocadito/HiveMind?style=flat-square&label=languages&color=555">
    <br><br>
    <code>🐝 Built with Rust · HiveMind © 2024</code>
  </sub>
  <br>
  <img src="https://capsule-render.vercel.app/api?type=waving&color=0:0d1117,50:00ff88,100:0d1117&height=100&section=footer&animation=fadeIn">
</div>
