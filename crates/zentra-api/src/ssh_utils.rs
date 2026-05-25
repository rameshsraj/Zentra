//! Shared SSH utilities for collecting real data from remote hosts.
//! Extracted from targets.rs get_target_stats handler for reuse across handlers.

use std::collections::HashMap;
use tokio::process::Command;
use serde::Serialize;

/// Result of running an SSH command on a remote host
#[derive(Debug, Clone, Serialize)]
pub struct SshResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Convert raw SSH stderr into a clean, user-friendly error message.
/// Hides internal details like port numbers, protocol strings, and stack traces.
pub fn friendly_error(host: &str, raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("connection timed out") || lower.contains("timed out") {
        return format!("Server '{}' is not reachable — connection timed out. Please check that the server is running and network connectivity is available.", host);
    }
    if lower.contains("connection refused") {
        return format!("Server '{}' refused the connection. Please verify the SSH service is running on the target.", host);
    }
    if lower.contains("no route to host") {
        return format!("No route to server '{}'. Please check the network configuration and ensure the host address is correct.", host);
    }
    if lower.contains("host key verification failed") {
        return format!("Host key verification failed for '{}'. The server's identity may have changed.", host);
    }
    if lower.contains("permission denied") || lower.contains("authentication") {
        return format!("Authentication failed for '{}'. Please verify the SSH credentials and ensure key-based auth is configured.", host);
    }
    if lower.contains("name or service not known") || lower.contains("could not resolve") {
        return format!("Cannot resolve hostname '{}'. Please check the server address.", host);
    }
    if lower.contains("network is unreachable") {
        return format!("Network is unreachable for '{}'. Please check your network configuration.", host);
    }
    // Generic fallback — still clean
    format!("Unable to connect to '{}'. Please check the server status and network connectivity.", host)
}

/// Run a command on a remote host via SSH.
/// Uses StrictHostKeyChecking=accept-new (TOFU — Trust On First Use): accepts
/// the host key on first connection and stores it, but rejects if the key
/// changes (protects against MITM after initial trust).
///
/// By default UserKnownHostsFile=/dev/null so the API server doesn't accumulate
/// known_hosts state. To enable persistent host key verification, set:
///   ZENTRA_SSH_KNOWN_HOSTS=/root/.ssh/known_hosts
pub async fn ssh_exec(host: &str, user: &str, script: &str) -> SshResult {
    let known_hosts = std::env::var("ZENTRA_SSH_KNOWN_HOSTS")
        .unwrap_or_else(|_| "/dev/null".to_string());
    let known_hosts_opt = format!("UserKnownHostsFile={}", known_hosts);
    let output = Command::new("ssh")
        .args([
            "-o", "StrictHostKeyChecking=accept-new",
            "-o", &known_hosts_opt,
            "-o", "ConnectTimeout=5",
            "-o", "BatchMode=yes",
            "-o", "LogLevel=ERROR",
            &format!("{}@{}", user, host),
            script,
        ])
        .output()
        .await;

    match output {
        Ok(out) => SshResult {
            success: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        },
        Err(e) => SshResult {
            success: false,
            stdout: String::new(),
            stderr: format!("SSH exec failed: {}", e),
        },
    }
}

/// Ensure the Zentra server has an SSH key pair, generate one if missing
pub async fn ensure_ssh_key() -> SshResult {
    // Check if key exists, generate if not
    let script = r#"
if [ ! -f ~/.ssh/id_rsa ]; then
    mkdir -p ~/.ssh && chmod 700 ~/.ssh
    ssh-keygen -t rsa -b 4096 -f ~/.ssh/id_rsa -N '' -q
    echo "KEY_GENERATED"
else
    echo "KEY_EXISTS"
fi
cat ~/.ssh/id_rsa.pub
"#;
    let output = Command::new("bash")
        .args(["-c", script])
        .output()
        .await;

    match output {
        Ok(out) => SshResult {
            success: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        },
        Err(e) => SshResult {
            success: false,
            stdout: String::new(),
            stderr: format!("Key generation failed: {}", e),
        },
    }
}

/// Set up SSH key-based auth from the Zentra server to a remote host using password.
/// Uses sshpass + ssh-copy-id if available, falls back to manual key injection.
pub async fn setup_ssh_key_auth(host: &str, user: &str, password: &str) -> SshResult {
    // First ensure we have a key
    let key_result = ensure_ssh_key().await;
    if !key_result.success {
        return key_result;
    }

    // Try sshpass + ssh-copy-id first.
    // Password is passed via SSHPASS env var with -e so it does NOT appear
    // in /proc/<pid>/cmdline or shell history.
    let output = Command::new("sshpass")
        .env("SSHPASS", password)
        .args([
            "-e", "ssh-copy-id",
            "-o", "StrictHostKeyChecking=no",
            "-o", "UserKnownHostsFile=/dev/null",
            &format!("{}@{}", user, host),
        ])
        .output()
        .await;

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();

            if out.status.success() {
                return SshResult {
                    success: true,
                    stdout: "SSH key auth configured successfully via ssh-copy-id".into(),
                    stderr: String::new(),
                };
            }

            // If sshpass is not installed, fall back to expect-based approach
            if stderr.contains("command not found") || stderr.contains("sshpass") {
                return setup_ssh_key_manual(host, user, password).await;
            }

            SshResult {
                success: false,
                stdout,
                stderr,
            }
        }
        Err(_) => setup_ssh_key_manual(host, user, password).await,
    }
}

/// Manual SSH key setup: read public key, inject via expect or manual SSH
async fn setup_ssh_key_manual(host: &str, user: &str, password: &str) -> SshResult {
    // Read our public key
    let key_output = Command::new("cat")
        .arg(format!("{}/.ssh/id_rsa.pub", std::env::var("HOME").unwrap_or_else(|_| "/root".into())))
        .output()
        .await;

    let pub_key = match key_output {
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Err(e) => return SshResult {
            success: false,
            stdout: String::new(),
            stderr: format!("Cannot read public key: {}", e),
        },
    };

    if pub_key.is_empty() {
        return SshResult {
            success: false,
            stdout: String::new(),
            stderr: "No SSH public key found. Run ssh-keygen first.".into(),
        };
    }

    // Use expect to automate password-based SSH and inject the key.
    // The password is ONLY passed via the SSHPASS environment variable
    // (set below with .env("SSHPASS", password)) and read inside expect
    // as $env(SSHPASS). This prevents command injection via special
    // characters in passwords and keeps the secret out of /proc/<pid>/cmdline
    // and shell history. The password is NEVER interpolated into the
    // expect script string directly.
    let expect_script = format!(
        r#"
expect -c '
set timeout 15
set pass $env(SSHPASS)
spawn ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null {user}@{host} "mkdir -p ~/.ssh && chmod 700 ~/.ssh && echo \"{key}\" >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys && sort -u -o ~/.ssh/authorized_keys ~/.ssh/authorized_keys && echo SSH_KEY_INJECTED"
expect {{
    "password:" {{ send "$pass\r"; exp_continue }}
    "Password:" {{ send "$pass\r"; exp_continue }}
    "SSH_KEY_INJECTED" {{ }}
    timeout {{ puts "TIMEOUT"; exit 1 }}
    eof {{ }}
}}
expect eof
' 2>&1
"#,
        user = user,
        host = host,
        key = pub_key.replace('"', r#"\""#),
    );

    let output = Command::new("bash")
        .env("SSHPASS", password)
        .args(["-c", &expect_script])
        .output()
        .await;

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();

            if stdout.contains("SSH_KEY_INJECTED") {
                // Verify key-based auth works now
                let verify = ssh_exec(host, user, "echo VERIFY_OK").await;
                if verify.success && verify.stdout.contains("VERIFY_OK") {
                    return SshResult {
                        success: true,
                        stdout: "SSH key auth configured and verified successfully".into(),
                        stderr: String::new(),
                    };
                }
                return SshResult {
                    success: true,
                    stdout: "SSH key injected but verification pending — may need a moment to propagate".into(),
                    stderr: verify.stderr,
                };
            }

            // Check if expect isn't installed
            if stderr.contains("command not found") || stdout.contains("command not found") {
                return SshResult {
                    success: false,
                    stdout: String::new(),
                    stderr: "Neither sshpass nor expect is installed on the Zentra server. Install one: yum install sshpass expect".into(),
                };
            }

            SshResult {
                success: false,
                stdout,
                stderr,
            }
        }
        Err(e) => SshResult {
            success: false,
            stdout: String::new(),
            stderr: format!("Failed to run SSH key setup: {}", e),
        },
    }
}

/// Parse section-delimited SSH output (===KEY=== format)
pub fn parse_sections(output: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut current_key = String::new();
    let mut current_val = String::new();

    for line in output.lines() {
        if line.starts_with("===") && line.ends_with("===") {
            if !current_key.is_empty() {
                map.insert(current_key.clone(), current_val.trim().to_string());
            }
            current_key = line.trim_matches('=').to_string();
            current_val = String::new();
        } else if !current_key.is_empty() {
            if !current_val.is_empty() {
                current_val.push('\n');
            }
            current_val.push_str(line);
        }
    }
    if !current_key.is_empty() {
        map.insert(current_key, current_val.trim().to_string());
    }
    map
}

/// Full server stats collection script
pub const SERVER_STATS_SCRIPT: &str = r#"
echo "===HOSTNAME==="
hostname 2>/dev/null || echo "unknown"
echo "===OS==="
(grep '^PRETTY_NAME=' /etc/os-release 2>/dev/null || grep '^ID=' /etc/os-release 2>/dev/null || echo "NAME=$(uname -s)") | head -1 | cut -d= -f2 | tr -d '"'
echo "===KERNEL==="
uname -r 2>/dev/null || echo "unknown"
echo "===CPU_CORES==="
nproc 2>/dev/null || grep -c ^processor /proc/cpuinfo 2>/dev/null || echo "0"
echo "===CPU_MODEL==="
grep 'model name' /proc/cpuinfo 2>/dev/null | head -1 | cut -d: -f2 | sed 's/^ //' || echo "unknown"
echo "===CPU_USAGE==="
top -bn1 2>/dev/null | grep 'Cpu(s)' | awk '{print 100 - $8}' || mpstat 1 1 2>/dev/null | tail -1 | awk '{print 100 - $NF}' || echo "0"
echo "===LOAD_AVG==="
cat /proc/loadavg 2>/dev/null | awk '{print $1, $2, $3}' || echo "0 0 0"
echo "===MEMORY==="
free -m 2>/dev/null | grep Mem | awk '{print $2, $3}' || echo "0 0"
echo "===SWAP==="
free -m 2>/dev/null | grep Swap | awk '{print $2, $3}' || echo "0 0"
echo "===DISK==="
df -BG / 2>/dev/null | tail -1 | awk '{gsub("G",""); print $2, $3, $5}' || echo "0 0 0%"
echo "===UPTIME==="
cat /proc/uptime 2>/dev/null | awk '{print int($1)}' || echo "0"
echo "===PROCS==="
ps aux 2>/dev/null | wc -l || echo "0"
echo "===USERS==="
who 2>/dev/null | wc -l || echo "0"
echo "===END==="
"#;

/// Network IO collection script (per-interface)
pub const NETWORK_IO_SCRIPT: &str = r#"
echo "===INTERFACES==="
cat /proc/net/dev 2>/dev/null | tail -n +3 | awk '{gsub(":", " "); print $1, $2, $3, $4, $5, $10, $11, $12, $13}'
echo "===CONNECTIONS==="
cat /proc/net/tcp 2>/dev/null | tail -n +2 | wc -l
echo "===UDP==="
cat /proc/net/udp 2>/dev/null | tail -n +2 | wc -l
echo "===END==="
"#;

/// Disk detail collection script
pub const DISK_DETAIL_SCRIPT: &str = r#"
echo "===MOUNTS==="
df -BG 2>/dev/null | tail -n +2 | awk '{gsub("G",""); print $1, $6, $2, $3, $5}'
echo "===IO==="
cat /proc/diskstats 2>/dev/null | awk '{if($4>0) print $3, $4, $6, $8, $10}'
echo "===END==="
"#;

/// Process list script (top 10 by CPU)
pub const PROCESS_LIST_SCRIPT: &str = r#"
echo "===PROCS==="
ps aux --sort=-%cpu 2>/dev/null | head -11 | tail -10 | awk '{print $2, $1, $3, $6, $8, $11}'
echo "===TOTAL==="
ps aux 2>/dev/null | wc -l
echo "===END==="
"#;

/// Collect real server stats from a remote host via SSH and return parsed data
pub async fn collect_server_stats(host: &str, user: &str) -> HashMap<String, String> {
    let result = ssh_exec(host, user, SERVER_STATS_SCRIPT).await;
    if result.success {
        parse_sections(&result.stdout)
    } else {
        let mut m = HashMap::new();
        m.insert("ERROR".into(), friendly_error(host, &result.stderr));
        m
    }
}
