# MITRE ATT&CK Coverage

36 techniques across 10 tactics. Full catalog in `agent_base/src/attack.rs`.

## Defense Evasion (TA0005) — 10 techniques

| ID | Technique | Swarm Module |
|----|-----------|-------------|
| T1055.012 | Process Hollowing | `fileless::MemfdBinary` |
| T1562.001 | Disable/Modify Tools | `syscalls` |
| T1622 | Debugger Evasion | `anti_analysis` |
| T1497.001 | System Checks (Sandbox) | `anti_analysis` |
| T1497.003 | Time Based Evasion | `utils::safe_init` |
| T1027.002 | Software Packing | `crypto::decrypt_model` |
| T1027.005 | Indicator Removal | `shared_arena` |
| T1070.004 | File Deletion | `dropper` |
| T1564.004 | Hidden File System (Memory) | `fileless` |
| T1055 | Process Injection (Polymorphic) | `weaver` |

## Discovery (TA0007) — 5 techniques

| ID | Technique | Swarm Module |
|----|-----------|-------------|
| T1082 | System Information Discovery | `scout` |
| T1057 | Process Discovery | `scout` |
| T1046 | Network Service Discovery | `lateral::discover_hosts` |
| T1518.001 | Security Software Discovery | `scout` |
| T1614.001 | System Location Discovery | `anti_analysis::check_vm` |

## Credential Access (TA0006) — 3 techniques

| ID | Technique | Swarm Module |
|----|-----------|-------------|
| T1552.001 | Credentials from Files (SSH Keys) | `lateral::harvest_credentials` |
| T1552.004 | Credentials from Files (Cloud Keys) | `lateral::harvest_credentials` |
| T1552.002 | Credentials in Files (.bash_history) | `lateral::harvest_credentials` |

## Lateral Movement (TA0008) — 4 techniques

| ID | Technique | Swarm Module |
|----|-----------|-------------|
| T1021.004 | Remote Services: SSH | `lateral::exec_ssh` |
| T1570 | Lateral Tool Transfer (SCP) | `lateral::deploy_agent_ssh` |
| T1021.006 | Remote Services: WinRM (via SSH) | `lateral::exec_winrm` |
| T1047 | Remote Services: WMI (via SSH) | `lateral::exec_wmi` |

## Command & Control (TA0011) — 4 techniques

| ID | Technique | Swarm Module |
|----|-----------|-------------|
| T1573.002 | Encrypted Channel | `crypto` + `identity` |
| T1090.004 | Proxy: CDN Fronting | `exfil::http_beacon` |
| T1572 | Protocol Tunneling (DNS) | `exfil::dns_exfiltrate` |
| T1571 | Non-Standard Port | `shared_arena` |

## Exfiltration (TA0010) — 3 techniques

| ID | Technique | Swarm Module |
|----|-----------|-------------|
| T1048.003 | Exfiltration Over DNS | `exfil::dns_exfiltrate` |
| T1048.002 | Exfiltration Over HTTP | `exfil::http_beacon` |
| T1029 | Scheduled Transfer | `exfil::ExfilScheduler` |

## Execution (TA0002) — 2 techniques

| ID | Technique | Swarm Module |
|----|-----------|-------------|
| T1204.002 | User Execution (Dropper) | `dropper` |
| T1106 | Native API (Syscalls) | `syscalls` |

## Persistence (TA0003) — 2 techniques

| ID | Technique | Swarm Module |
|----|-----------|-------------|
| T1543.002 | System Process Creation | `shaper::regenerate_agent` |
| T1547.001 | Boot/Logon Autostart | `shaper` |

## Collection (TA0009) — 1 technique

| ID | Technique | Swarm Module |
|----|-----------|-------------|
| T1005 | Data from Local System | `hoarder` |

## Impact (TA0040) — 1 technique

| ID | Technique | Swarm Module |
|----|-----------|-------------|
| T1485 | Data Destruction | `hoarder` |

## Coverage Statistics

```
Total: 36 techniques across 10 tactics
Coverage: ~18% of enterprise matrix
Detection difficulty: 5 Very High, 12 High, 19 Medium
```
