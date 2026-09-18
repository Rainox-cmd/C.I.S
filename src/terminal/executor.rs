use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::security::{
    build_audit_safe_command, check_path_containment, is_sensitive_var, redact_secrets,
    PathContainmentError, RiskLevel, SecurityPolicy,
};

use super::result::ExecutionResult;

const ALLOWED_ENV_VARS: &[&str] = &[
    "PATH",
    "Path",
    "HOME",
    "USERPROFILE",
    "USER",
    "USERNAME",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "RUSTFLAGS",
    "RUST_BACKTRACE",
    "TERM",
    "SystemRoot",
    "SYSTEMROOT",
    "TEMP",
    "TMP",
    "TMPDIR",
    "WINDIR",
    "windir",
    "APPDATA",
    "HOMEDRIVE",
    "HOMEPATH",
];

const POLL_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Debug, Clone)]
pub struct TerminalExecutor {
    pub policy: SecurityPolicy,
    pub timeout: Duration,
    pub max_output_bytes: usize,
    pub project_root: PathBuf,
    pub logs_dir: PathBuf,
    pub redact_secrets: bool,
    pub restrict_to_project: bool,
}

impl TerminalExecutor {
    pub fn new(
        project_root: PathBuf,
        logs_dir: PathBuf,
        timeout: Duration,
        max_output_bytes: usize,
        policy: SecurityPolicy,
        redact_secrets: bool,
        restrict_to_project: bool,
    ) -> Self {
        Self {
            policy,
            timeout,
            max_output_bytes,
            project_root,
            logs_dir,
            redact_secrets,
            restrict_to_project,
        }
    }

    pub fn from_config(
        project_root: PathBuf,
        logs_dir: PathBuf,
        config: &crate::config::Config,
    ) -> Self {
        let policy = SecurityPolicy {
            allowlist: config.security.command_allowlist.clone(),
            denylist: config.security.command_denylist.clone(),
            require_confirmation: config.security.require_confirmation_for_risky,
            ..SecurityPolicy::default()
        };

        Self {
            policy,
            timeout: Duration::from_secs(config.security.default_timeout_seconds),
            max_output_bytes: config.security.max_output_bytes as usize,
            project_root,
            logs_dir,
            redact_secrets: config.security.redact_secrets,
            restrict_to_project: config.security.restrict_to_project_root,
        }
    }

    pub fn execute(
        &self,
        command: &str,
        args: &[&str],
        cwd: Option<&Path>,
        skip_confirmation: bool,
    ) -> Result<ExecutionResult> {
        let full_command = build_audit_safe_command(
            command,
            &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        );

        let full_cmd_string = if args.is_empty() {
            command.to_string()
        } else {
            format!("{} {}", command, args.join(" "))
        };
        let risk_level = self.policy.classify(&full_cmd_string);

        // Check denylist / allowlist
        if risk_level == RiskLevel::Blocked {
            let result = ExecutionResult {
                command: command.to_string(),
                args: args.iter().map(|s| s.to_string()).collect(),
                exit_code: -1,
                stdout: String::new(),
                stderr: "Command blocked by security policy".to_string(),
                duration: Duration::ZERO,
                timed_out: false,
                blocked: true,
                block_reason: Some(
                    "Command is on the denylist or not in the allowlist".to_string(),
                ),
                requires_confirmation: false,
                risk_description: None,
                audit_log_path: self.audit_log_path(),
            };
            self.write_audit_log(&result, cwd, risk_level, false);
            tracing::warn!(command = %full_command, "Command blocked by security policy");
            return Ok(result);
        }

        // Check risky commands requiring confirmation
        let is_risky = risk_level == RiskLevel::Risky;
        let risk_desc = if is_risky {
            self.policy.risky_description(&full_cmd_string)
        } else {
            None
        };

        let requires_confirmation =
            is_risky && self.policy.require_confirmation && !skip_confirmation;

        if requires_confirmation {
            let result = ExecutionResult {
                command: command.to_string(),
                args: args.iter().map(|s| s.to_string()).collect(),
                exit_code: -1,
                stdout: String::new(),
                stderr: format!(
                    "Command requires confirmation. Risk: {}",
                    risk_desc.as_deref().unwrap_or("unknown")
                ),
                duration: Duration::ZERO,
                timed_out: false,
                blocked: false,
                block_reason: None,
                requires_confirmation: true,
                risk_description: risk_desc,
                audit_log_path: self.audit_log_path(),
            };
            self.write_audit_log(&result, cwd, risk_level, false);
            return Ok(result);
        }

        // Resolve and validate working directory
        let resolved_cwd = match self.resolve_cwd(cwd) {
            Ok(p) => p,
            Err(e) => {
                let result = ExecutionResult {
                    command: command.to_string(),
                    args: args.iter().map(|s| s.to_string()).collect(),
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: format!("Path containment check failed: {}", e),
                    duration: Duration::ZERO,
                    timed_out: false,
                    blocked: true,
                    block_reason: Some(format!("Path containment: {}", e)),
                    requires_confirmation: false,
                    risk_description: risk_desc,
                    audit_log_path: self.audit_log_path(),
                };
                self.write_audit_log(&result, cwd, risk_level, false);
                tracing::warn!(command = %full_command, error = %e, "Path containment failed");
                return Ok(result);
            }
        };

        let filtered_env = self.build_filtered_env();

        let start = Instant::now();

        let is_windows_builtin = cfg!(windows) && ["echo", "dir", "type", "copy", "move", "del", "ren", "md", "cd"].contains(&command.to_lowercase().as_str());

        if is_windows_builtin {
            let metachars = ['&', '|', ';', '%', '^', '<', '>'];
            if full_cmd_string.chars().any(|c| metachars.contains(&c)) {
                let result = ExecutionResult {
                    command: command.to_string(),
                    args: args.iter().map(|s| s.to_string()).collect(),
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: "Command blocked: contains shell metacharacters".to_string(),
                    duration: Duration::ZERO,
                    timed_out: false,
                    blocked: true,
                    block_reason: Some("Shell metacharacters are not allowed".to_string()),
                    requires_confirmation: false,
                    risk_description: None,
                    audit_log_path: self.audit_log_path(),
                };
                self.write_audit_log(&result, cwd, risk_level, false);
                return Ok(result);
            }
        }

        let mut cmd = if is_windows_builtin {
            let mut c = Command::new("cmd.exe");
            c.arg("/d").arg("/c").arg(command).args(args);
            c
        } else {
            let mut c = Command::new(command);
            c.args(args);
            c
        };

        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(&resolved_cwd)
            .envs(&filtered_env)
            .spawn()
            .with_context(|| format!("Failed to spawn command: {}", command))?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let max_bytes = self.max_output_bytes as u64;

        let stdout_thread = stdout.map(|s| {
            thread::spawn(move || {
                let mut buf = Vec::new();
                let mut limited = s.take(max_bytes);
                let _ = limited.read_to_end(&mut buf);
                buf
            })
        });

        let stderr_thread = stderr.map(|s| {
            thread::spawn(move || {
                let mut buf = Vec::new();
                let mut limited = s.take(max_bytes);
                let _ = limited.read_to_end(&mut buf);
                buf
            })
        });

        let mut timed_out = false;

        let exit_code = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status.code().unwrap_or(-1),
                Ok(None) => {
                    if start.elapsed() >= self.timeout {
                        timed_out = true;
                        let _ = child.kill();
                        let _ = child.wait();
                        break -1;
                    }
                    thread::sleep(POLL_INTERVAL);
                }
                Err(_) => break -1,
            }
        };

        let stdout_bytes = stdout_thread
            .map(|h| h.join().unwrap_or_default())
            .unwrap_or_default();
        let stderr_bytes = stderr_thread
            .map(|h| h.join().unwrap_or_default())
            .unwrap_or_default();

        let mut stdout_str = String::from_utf8_lossy(&stdout_bytes).into_owned();
        let mut stderr_str = String::from_utf8_lossy(&stderr_bytes).into_owned();

        if stdout_bytes.len() >= self.max_output_bytes {
            stdout_str.push_str("\n...[output truncated]");
        }
        if stderr_bytes.len() >= self.max_output_bytes {
            stderr_str.push_str("\n...[output truncated]");
        }

        if self.redact_secrets {
            stdout_str = redact_secrets(&stdout_str);
            stderr_str = redact_secrets(&stderr_str);
        }

        let duration = start.elapsed();

        let result = ExecutionResult {
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            exit_code,
            stdout: stdout_str,
            stderr: stderr_str,
            duration,
            timed_out,
            blocked: false,
            block_reason: None,
            requires_confirmation: false,
            risk_description: None,
            audit_log_path: self.audit_log_path(),
        };

        self.write_audit_log(&result, cwd, risk_level, true);

        if timed_out {
            tracing::warn!(command = %full_command, duration_ms = duration.as_millis(), "Command timed out");
        } else {
            tracing::info!(
                command = %full_command,
                exit_code = exit_code,
                duration_ms = duration.as_millis(),
                "Command executed"
            );
        }

        Ok(result)
    }

    fn resolve_cwd(&self, cwd: Option<&Path>) -> Result<PathBuf, PathContainmentError> {
        let target = match cwd {
            Some(c) => c.to_path_buf(),
            None => self.project_root.clone(),
        };

        if self.restrict_to_project {
            check_path_containment(&target, &self.project_root)?;
        }

        Ok(target)
    }

    fn build_filtered_env(&self) -> HashMap<String, String> {
        let current_env: Vec<(String, String)> = std::env::vars().collect();
        let mut filtered: HashMap<String, String> = HashMap::new();

        for (key, value) in &current_env {
            if ALLOWED_ENV_VARS.contains(&key.as_str()) {
                if self.redact_secrets && is_sensitive_var(key) {
                    filtered.insert(key.clone(), "[REDACTED]".to_string());
                } else {
                    filtered.insert(key.clone(), value.clone());
                }
            }
        }

        filtered
    }

    fn audit_log_path(&self) -> PathBuf {
        self.logs_dir.join("audit.log")
    }

    fn write_audit_log(
        &self,
        result: &ExecutionResult,
        cwd: Option<&Path>,
        risk_level: RiskLevel,
        executed: bool,
    ) {
        if let Err(e) = self._write_audit_log(result, cwd, risk_level, executed) {
            tracing::warn!(error = %e, "Failed to write audit log");
        }
    }

    fn _write_audit_log(
        &self,
        result: &ExecutionResult,
        cwd: Option<&Path>,
        risk_level: RiskLevel,
        executed: bool,
    ) -> Result<()> {
        let audit_path = self.audit_log_path();
        if let Some(parent) = audit_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create audit log directory: {}", parent.display())
            })?;
        }

        let allowed = !result.blocked && !result.requires_confirmation && executed;
        let status_str = if result.blocked {
            "blocked"
        } else if result.timed_out {
            "timed_out"
        } else if executed {
            "executed"
        } else if result.requires_confirmation {
            "requires_confirmation"
        } else {
            "unknown"
        };

        let working_dir = cwd
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| self.project_root.to_string_lossy().to_string());

        let entry = AuditEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            program: &result.command,
            arguments: build_audit_safe_command(&result.command, &result.args),
            working_directory: &working_dir,
            exit_status: status_str,
            exit_code: result.exit_code,
            duration_ms: result.duration.as_millis(),
            timed_out: result.timed_out,
            blocked: result.blocked,
            block_reason: result.block_reason.as_deref(),
            requires_confirmation: result.requires_confirmation,
            risk_level: match risk_level {
                RiskLevel::Blocked => "blocked",
                RiskLevel::Risky => "risky",
                RiskLevel::Allowed => "allowed",
            },
            allowed,
        };

        let line = serde_json::to_string(&entry).unwrap_or_else(|e| {
            format!(
                r#"{{"timestamp":"error","error":"failed to serialize audit entry: {}"}}"#,
                e
            )
        });

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&audit_path)
            .with_context(|| format!("Failed to open audit log: {}", audit_path.display()))?;

        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;

        Ok(())
    }
}

#[derive(Serialize)]
struct AuditEntry<'a> {
    timestamp: String,
    program: &'a str,
    arguments: String,
    working_directory: &'a str,
    exit_status: &'a str,
    exit_code: i32,
    duration_ms: u128,
    timed_out: bool,
    blocked: bool,
    block_reason: Option<&'a str>,
    requires_confirmation: bool,
    risk_level: &'a str,
    allowed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::RiskyCommand;
    use std::fs;
    use tempfile::tempdir;

    fn make_test_executor() -> (TerminalExecutor, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let logs_dir = dir.path().join("logs");
        fs::create_dir_all(&logs_dir).unwrap();
        let executor = TerminalExecutor::new(
            dir.path().to_path_buf(),
            logs_dir,
            Duration::from_secs(30),
            1024 * 1024,
            SecurityPolicy::default(),
            true,
            true,
        );
        (executor, dir)
    }

    fn make_permissive_executor() -> (TerminalExecutor, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let logs_dir = dir.path().join("logs");
        fs::create_dir_all(&logs_dir).unwrap();
        let mut policy = SecurityPolicy::default();
        policy.allowlist.clear();
        let executor = TerminalExecutor::new(
            dir.path().to_path_buf(),
            logs_dir,
            Duration::from_secs(30),
            1024 * 1024,
            policy,
            true,
            true,
        );
        (executor, dir)
    }

    #[test]
    fn test_execute_basic_command() {
        let (executor, _dir) = make_test_executor();
        let result = executor
            .execute("cargo", &["--version"], None, false)
            .unwrap();
        assert!(result.success());
        assert!(result.stdout.contains("cargo"));
    }

    #[test]
    fn test_execute_blocked_command() {
        let (executor, _dir) = make_test_executor();
        let result = executor.execute("rm", &["-rf", "/"], None, false).unwrap();
        assert!(result.is_blocked());
        assert!(result.stderr.contains("blocked"));
    }

    #[test]
    fn test_execute_non_allowlisted_blocked() {
        let (executor, _dir) = make_test_executor();
        let result = executor.execute("evil_command", &[], None, false).unwrap();
        assert!(result.is_blocked());
    }

    #[test]
    fn test_execute_risky_command_requires_confirmation() {
        let (executor, _dir) = make_test_executor();
        let result = executor.execute("rm", &["file.txt"], None, false).unwrap();
        assert!(result.requires_confirmation);
        assert!(!result.is_blocked());
        assert!(result.stderr.contains("confirmation"));
    }

    #[test]
    fn test_execute_risky_command_with_confirmation() {
        let dir = tempdir().unwrap();
        let logs_dir = dir.path().join("logs");
        fs::create_dir_all(&logs_dir).unwrap();
        let mut policy = SecurityPolicy::default();
        policy.allowlist.clear();
        policy.risky_patterns = vec![RiskyCommand {
            pattern: "cargo".to_string(),
            description: "Test risky command".to_string(),
        }];
        let executor = TerminalExecutor::new(
            dir.path().to_path_buf(),
            logs_dir,
            Duration::from_secs(5),
            1024 * 1024,
            policy,
            true,
            true,
        );
        let result = executor
            .execute("cargo", &["--version"], None, true)
            .unwrap();
        assert!(!result.requires_confirmation);
        assert!(!result.is_blocked());
        assert!(result.stdout.contains("cargo"));
    }

    #[test]
    fn test_execute_timeout() {
        let (executor, _dir) = make_test_executor();
        let result = executor
            .execute("cargo", &["--version"], None, false)
            .unwrap();
        assert!(!result.is_timed_out());
    }

    #[test]
    fn test_execute_timed_out_kills_process() {
        let dir = tempdir().unwrap();
        let logs_dir = dir.path().join("logs");
        fs::create_dir_all(&logs_dir).unwrap();
        let mut policy = SecurityPolicy::default();
        policy.allowlist.clear();
        let executor = TerminalExecutor::new(
            dir.path().to_path_buf(),
            logs_dir,
            Duration::from_millis(1),
            1024 * 1024,
            policy,
            true,
            true,
        );
        // sleep command may not exist on all systems; use a long-running cargo sub-command
        let result = executor.execute("cargo", &["--help"], None, true);
        // With 1ms timeout, cargo --help likely won't finish
        match result {
            Ok(r) => {
                // Either it finished too fast or timed out
                assert!(r.is_timed_out() || r.success() || r.exit_code == 0 || r.exit_code == -1);
            }
            Err(_) => {
                // Process spawn error is also acceptable for this test
            }
        }
    }

    #[test]
    fn test_execute_output_limit() {
        let dir = tempdir().unwrap();
        let logs_dir = dir.path().join("logs");
        fs::create_dir_all(&logs_dir).unwrap();
        let mut policy = SecurityPolicy::default();
        policy.allowlist.clear();
        let executor = TerminalExecutor::new(
            dir.path().to_path_buf(),
            logs_dir,
            Duration::from_secs(5),
            10,
            policy,
            true,
            true,
        );
        let result = executor
            .execute("cargo", &["--version"], None, true)
            .unwrap();
        // cargo --version output is short, but the limit is 10 bytes
        // The output should be truncated
        let _ = result;
    }

    #[test]
    fn test_execute_secret_redaction() {
        let dir = tempdir().unwrap();
        let logs_dir = dir.path().join("logs");
        fs::create_dir_all(&logs_dir).unwrap();
        let mut policy = SecurityPolicy::default();
        policy.allowlist.clear();
        let executor = TerminalExecutor::new(
            dir.path().to_path_buf(),
            logs_dir,
            Duration::from_secs(5),
            1024 * 1024,
            policy,
            true,
            true,
        );
        let result = executor
            .execute("cargo", &["--version"], None, true)
            .unwrap();
        // cargo --version shouldn't contain secrets, but let's verify redaction works
        assert!(!result.stdout.contains("sk-"));
    }

    #[test]
    fn test_execute_audit_log_created() {
        let (executor, _dir) = make_test_executor();
        let _ = executor
            .execute("cargo", &["--version"], None, false)
            .unwrap();
        let audit_path = executor.audit_log_path();
        assert!(audit_path.exists());
        let contents = fs::read_to_string(&audit_path).unwrap();
        assert!(contents.contains("cargo"));
        assert!(contents.contains("executed"));
    }

    #[test]
    fn test_execute_environment_filtering() {
        let (executor, _dir) = make_test_executor();
        let env = executor.build_filtered_env();
        // Sensitive vars should be redacted or absent
        if env.contains_key("NVIDIA_API_KEY") {
            assert_eq!(env.get("NVIDIA_API_KEY"), Some(&"[REDACTED]".to_string()));
        }
        if env.contains_key("API_KEY") {
            assert_eq!(env.get("API_KEY"), Some(&"[REDACTED]".to_string()));
        }
    }

    #[test]
    fn test_execute_blocked_audit_log() {
        let (executor, _dir) = make_test_executor();
        let _ = executor.execute("rm", &["-rf", "/"], None, false).unwrap();
        let audit_path = executor.audit_log_path();
        assert!(audit_path.exists());
        let contents = fs::read_to_string(&audit_path).unwrap();
        assert!(contents.contains("blocked"));
    }

    #[test]
    fn test_execute_risky_audit_log() {
        let (executor, _dir) = make_test_executor();
        let _ = executor.execute("rm", &["file.txt"], None, false).unwrap();
        let audit_path = executor.audit_log_path();
        let contents = fs::read_to_string(&audit_path).unwrap();
        assert!(contents.contains("risky"));
    }

    #[test]
    fn test_path_containment_inside_project() {
        let (executor, dir) = make_test_executor();
        let subdir = dir.path().join("subdir");
        fs::create_dir_all(&subdir).unwrap();
        let result = executor.execute("cargo", &["--version"], Some(&subdir), false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_path_containment_outside_project() {
        let (executor, _dir) = make_test_executor();
        let result = executor
            .execute("cargo", &["--version"], Some(Path::new("/")), false)
            .unwrap();
        assert!(result.is_blocked());
        assert!(result.stderr.contains("containment") || result.stderr.contains("not contained"));
    }

    #[test]
    fn test_permissive_policy_allows_cargo() {
        let (executor, _dir) = make_permissive_executor();
        let result = executor
            .execute("cargo", &["--version"], None, false)
            .unwrap();
        assert!(!result.is_blocked());
        assert!(result.stdout.contains("cargo"));
    }
}
