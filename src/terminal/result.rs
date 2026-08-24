use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub command: String,
    pub args: Vec<String>,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
    pub timed_out: bool,
    pub blocked: bool,
    pub block_reason: Option<String>,
    pub requires_confirmation: bool,
    pub risk_description: Option<String>,
    pub audit_log_path: std::path::PathBuf,
}

impl ExecutionResult {
    pub fn success(&self) -> bool {
        !self.blocked && !self.timed_out && self.exit_code == 0
    }

    pub fn blocked_or_failed(&self) -> bool {
        self.blocked || self.timed_out || self.exit_code != 0
    }

    pub fn is_blocked(&self) -> bool {
        self.blocked
    }

    pub fn is_timed_out(&self) -> bool {
        self.timed_out
    }

    pub fn formatted_output(&self) -> String {
        let mut out = String::new();
        if !self.stdout.is_empty() {
            out.push_str(&self.stdout);
        }
        if !self.stderr.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&self.stderr);
        }
        if out.is_empty() {
            out.push_str(&format!("[exit code: {}]", self.exit_code));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_result_success() {
        let result = ExecutionResult {
            command: "cargo".to_string(),
            args: vec!["build".to_string()],
            exit_code: 0,
            stdout: "Compiling cis...".to_string(),
            stderr: String::new(),
            duration: Duration::from_millis(500),
            timed_out: false,
            blocked: false,
            block_reason: None,
            requires_confirmation: false,
            risk_description: None,
            audit_log_path: std::path::PathBuf::new(),
        };

        assert!(result.success());
        assert!(!result.blocked_or_failed());
        assert!(!result.is_blocked());
        assert!(!result.is_timed_out());
    }

    #[test]
    fn test_execution_result_blocked() {
        let result = ExecutionResult {
            command: "rm".to_string(),
            args: vec!["-rf".to_string(), "/".to_string()],
            exit_code: -1,
            stdout: String::new(),
            stderr: String::new(),
            duration: Duration::from_millis(0),
            timed_out: false,
            blocked: true,
            block_reason: Some("Command is on the denylist".to_string()),
            requires_confirmation: false,
            risk_description: None,
            audit_log_path: std::path::PathBuf::new(),
        };

        assert!(!result.success());
        assert!(result.is_blocked());
        assert!(result.blocked_or_failed());
        assert_eq!(
            result.block_reason.as_deref(),
            Some("Command is on the denylist")
        );
    }

    #[test]
    fn test_execution_result_timed_out() {
        let result = ExecutionResult {
            command: "sleep".to_string(),
            args: vec!["60".to_string()],
            exit_code: -1,
            stdout: String::new(),
            stderr: String::new(),
            duration: Duration::from_secs(30),
            timed_out: true,
            blocked: false,
            block_reason: None,
            requires_confirmation: false,
            risk_description: None,
            audit_log_path: std::path::PathBuf::new(),
        };

        assert!(!result.success());
        assert!(result.is_timed_out());
        assert!(result.blocked_or_failed());
    }

    #[test]
    fn test_execution_result_formatted_output() {
        let result = ExecutionResult {
            command: "cargo".to_string(),
            args: vec!["build".to_string()],
            exit_code: 0,
            stdout: "Finished".to_string(),
            stderr: "warning".to_string(),
            duration: Duration::from_millis(100),
            timed_out: false,
            blocked: false,
            block_reason: None,
            requires_confirmation: false,
            risk_description: None,
            audit_log_path: std::path::PathBuf::new(),
        };

        let output = result.formatted_output();
        assert!(output.contains("Finished"));
        assert!(output.contains("warning"));
    }

    #[test]
    fn test_execution_result_empty_output() {
        let result = ExecutionResult {
            command: "cargo".to_string(),
            args: vec!["clean".to_string()],
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            duration: Duration::from_millis(50),
            timed_out: false,
            blocked: false,
            block_reason: None,
            requires_confirmation: false,
            risk_description: None,
            audit_log_path: std::path::PathBuf::new(),
        };

        let output = result.formatted_output();
        assert!(output.contains("[exit code: 0]"));
    }
}
