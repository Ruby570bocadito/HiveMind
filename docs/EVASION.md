# Evasion Stack

The swarm implements a 10-layer defense evasion stack designed to bypass modern EDRs (CrowdStrike, Defender ATP, SentinelOne, Elastic EDR, CarbonBlack).

---

## Layer 1: Network (No TCP)

**Module:** `shared_arena`, `arena_mgr`

All inter-agent communication uses a lock-free atomic ring buffer in shared memory (`memfd_create` + `mmap`). Zero TCP ports. Zero sockets. No `listen()`, no `bind()`, no `accept()`.

- **EDR surface eliminated:** Network monitoring, port scanning, socket inspection
- **Detection difficulty:** Very High — kernel only sees anonymous memory mappings

---

## Layer 2: Filesystem (Fileless)

**Module:** `fileless`

Agent binaries execute from `memfd_create` anonymous files. At no point does an agent binary touch the filesystem.

```rust
let fd = memfd_create("swarm_scout", MFD_CLOEXEC);
write(fd, binary_data);
Command::new(format!("/proc/self/fd/{}", fd)).spawn();
```

- **EDR surface eliminated:** File creation monitoring, binary signature scanning, quarantine
- **Detection difficulty:** Very High — no `execve` with a disk path

---

## Layer 3: Userland Hooks (Direct Syscalls)

**Module:** `syscalls` (Linux), `syscalls::windows` (Hell's Gate)

EDRs hook `ntdll.dll` (Windows) or `libc` (Linux) to intercept API calls. The swarm executes syscalls directly via inline assembly:

```rust
unsafe fn syscall3(nr: i64, a1: i64, a2: i64, a3: i64) -> i64 {
    asm!("syscall", in("rax") nr, in("rdi") a1, in("rsi") a2, in("rdx") a3, ...);
}
```

**Windows Hell's Gate:**
1. Reads clean `ntdll.dll` from disk
2. Parses PE export table
3. Extracts syscall numbers from `mov r10, rcx; mov eax, [SSN]` stubs
4. Executes `syscall` instruction directly

- **EDR surface eliminated:** Userland API hooking, ETW, AMSI (for injection calls)
- **Detection difficulty:** Very High

---

## Layer 4: Call Stack (Synthetic Frames)

**Module:** `stack_spoof`

Modern EDRs analyze the call stack to verify syscalls originate from legitimate modules. The swarm crafts synthetic RBP chains pointing to `libc.so` or `ntdll.dll` frames.

- **EDR surface eliminated:** Call stack analysis, ETW stack walking
- **Detection difficulty:** High

---

## Layer 5: ML Models (Encrypted at Rest)

**Modules:** `crypto`, `build.rs` (scout, shaper)

ONNX model files are XOR-encrypted at build time. The raw model never appears in the binary.

```rust
// build.rs
let encrypted = xor_encrypt(&model_bytes, seed);
fs::write("model.onnx.enc", &encrypted);

// runtime
const ENC: &[u8] = include_bytes!(...);
let model = decrypt_model(ENC, seed);
let session = OnnxModel::new(&model); // in RAM, then dropped
```

- **EDR surface eliminated:** Static ML model analysis, YARA rules targeting ONNX structures
- **Detection difficulty:** High — compiled binary shows no ONNX protobuf signatures

---

## Layer 6: Debugger Detection

**Module:** `anti_analysis`

Checks performed at agent startup:
- `/proc/self/status` TracerPid — if non-zero, a debugger is attached
- Self-ptrace: if `ptrace(PTRACE_TRACEME)` fails, already being traced
- Random 1-10 second delay to evade timing-based sandbox detection

- **EDR surface eliminated:** Debugger attachment, dynamic analysis
- **Detection difficulty:** Medium

---

## Layer 7: Sandbox Detection

**Module:** `anti_analysis`

Checks:
- **Uptime < 10 minutes** → likely sandbox
- **CPU cores < 2** → likely sandbox
- **RAM < 2GB** → likely sandbox
- **Username contains** "sandbox", "malware", "virus", "test"
- If 3+ indicators → agent operates in stealth mode

- **EDR surface eliminated:** Automated sandbox analysis
- **Detection difficulty:** Medium

---

## Layer 8: VM Detection

**Module:** `anti_analysis`

Checks:
- `/sys/class/dmi/id/product_name` for "VirtualBox", "VMware", "QEMU", "KVM", "Xen", "Hyper-V"
- `/proc/cpuinfo` for "hypervisor" flag
- `/proc/modules` for VM guest drivers (vboxguest, vmw_balloon, virtio, hv_vmbus)

- **EDR surface eliminated:** VM-based analysis environments
- **Detection difficulty:** Medium

---

## Layer 9: String Obfuscation

**Module:** `obfstr`

Sensitive strings (`127.0.0.1:4242`, `swarm_arena`, agent names, key seeds) are XOR-encrypted at compile time via the `obf!()` macro:

```rust
let addr: String = obf!("127.0.0.1:4242");
// Expands to: xor_decrypt(&ENCRYPTED_ARRAY)
```

`strings swarm_binary` reveals nothing useful.

- **EDR surface eliminated:** Static string analysis, YARA string rules
- **Detection difficulty:** Medium

---

## Layer 10: Honey Detection

**Module:** `honey`

Before any offensive action, the swarm checks for:
- **Honeyfiles:** bait filenames (passwords.txt, credentials.docx), suspicious sizes (0/1024/1337 bytes), world-writable permissions (777)
- **Honeypots:** known service ports (Cowrie SSH 2222, Dionaea 21/23), banner keywords ("cowrie", "honeypot", "dionaea")
- **Canary tokens:** AWS keys with "CANARY", URLs at canarytokens.org, HoneyDocs links

If detected → SKIP target, log warning.

- **EDR surface eliminated:** Honeypot triggering, canary token alerts
- **Detection difficulty:** Medium

---

## Summary

| Layer | Technique | EDR Bypassed |
|-------|-----------|-------------|
| 1 | Shared memory IPC | Network monitors |
| 2 | memfd_create | File system monitors |
| 3 | Direct syscalls | Userland API hooks |
| 4 | Call stack spoofing | ETW stack analysis |
| 5 | Encrypted models | Static ML analysis |
| 6 | Debugger detection | Debugger/sandbox |
| 7 | Sandbox detection | Automated analysis |
| 8 | VM detection | Analysis VMs |
| 9 | String obfuscation | YARA string rules |
| 10 | Honey detection | Honeypots/canaries |
