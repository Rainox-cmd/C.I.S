use std::path::Path;

use super::RiskLevel;

#[derive(Debug, Clone)]
pub struct RiskyCommand {
    pub pattern: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    pub allowlist: Vec<String>,
    pub denylist: Vec<String>,
    pub risky_patterns: Vec<RiskyCommand>,
    pub require_confirmation: bool,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            allowlist: vec![
                "python".into(),
                "python3".into(),
                "node".into(),
                "cargo".into(),
                "go".into(),
                "java".into(),
                "rustc".into(),
                "npm".into(),
            ],
            denylist: vec![
                "rm -rf".into(),
                "del /s".into(),
                "format".into(),
                "shutdown".into(),
                "shutdown.exe".into(),
                "diskpart".into(),
                "fdisk".into(),
            ],
            risky_patterns: vec![
                RiskyCommand {
                    pattern: "rm".into(),
                    description: "File or directory deletion".into(),
                },
                RiskyCommand {
                    pattern: "del".into(),
                    description: "Windows file deletion".into(),
                },
                RiskyCommand {
                    pattern: "rd".into(),
                    description: "Windows directory removal".into(),
                },
                RiskyCommand {
                    pattern: "rclone".into(),
                    description: "Remote sync tool — may delete remote data".into(),
                },
                RiskyCommand {
                    pattern: "curl".into(),
                    description: "Network data transfer".into(),
                },
                RiskyCommand {
                    pattern: "wget".into(),
                    description: "Network data transfer".into(),
                },
                RiskyCommand {
                    pattern: "git push".into(),
                    description: "Pushes to remote".into(),
                },
                RiskyCommand {
                    pattern: "git commit".into(),
                    description: "Commits changes".into(),
                },
            ],
            require_confirmation: true,
        }
    }
}

impl SecurityPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_allowlist(allowlist: Vec<String>) -> Self {
        Self {
            allowlist,
            ..Self::default()
        }
    }

    pub fn with_risky_confirmation(mut self, enabled: bool) -> Self {
        self.require_confirmation = enabled;
        self
    }

    pub fn allow(&mut self, cmd: &str) {
        self.allowlist.push(cmd.to_string());
    }

    pub fn deny(&mut self, cmd: &str) {
        self.denylist.push(cmd.to_string());
    }

    pub fn classify(&self, command: &str) -> RiskLevel {
        for deny in &self.denylist {
            if matches_pattern(command, deny) {
                return RiskLevel::Blocked;
            }
        }

        for risky in &self.risky_patterns {
            if matches_pattern(command, &risky.pattern) {
                return RiskLevel::Risky;
            }
        }

        if !self.allowlist.is_empty() {
            for allow in &self.allowlist {
                if matches_pattern(command, allow) {
                    return RiskLevel::Allowed;
                }
            }
            return RiskLevel::Blocked;
        }

        RiskLevel::Allowed
    }

    pub fn is_blocked(&self, command: &str) -> bool {
        self.classify(command) == RiskLevel::Blocked
    }

    pub fn is_risky(&self, command: &str) -> bool {
        self.classify(command) == RiskLevel::Risky
    }

    pub fn is_allowed(&self, command: &str) -> bool {
        matches!(
            self.classify(command),
            RiskLevel::Allowed | RiskLevel::Risky
        )
    }

    pub fn risky_description(&self, command: &str) -> Option<String> {
        for risky in &self.risky_patterns {
            if matches_pattern(command, &risky.pattern) {
                return Some(risky.description.clone());
            }
        }
        None
    }
}

fn extract_command_name(cmd: &str) -> String {
    let first = cmd.split_whitespace().next().unwrap_or("");
    Path::new(first)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(first)
        .to_lowercase()
}

fn matches_pattern(cmd: &str, pattern: &str) -> bool {
    let cmd_lower = cmd.trim().to_lowercase();
    let pat_lower = pattern.trim().to_lowercase();

    let cmd_name = extract_command_name(&cmd_lower);
    let pat_name = extract_command_name(&pat_lower);

    if pat_lower.contains(' ') {
        cmd_lower == pat_lower || cmd_lower.starts_with(&format!("{} ", pat_lower))
    } else {
        cmd_name == pat_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_blocks_denylisted() {
        let policy = SecurityPolicy::default();
        assert_eq!(policy.classify("rm -rf /"), RiskLevel::Blocked);
        assert_eq!(policy.classify("format C:"), RiskLevel::Blocked);
        assert_eq!(policy.classify("shutdown /s /t 0"), RiskLevel::Blocked);
        assert_eq!(policy.classify("diskpart"), RiskLevel::Blocked);
    }

    #[test]
    fn test_policy_blocks_non_allowlisted() {
        let policy = SecurityPolicy::default();
        assert_eq!(policy.classify("evil_command"), RiskLevel::Blocked);
        assert_eq!(policy.classify("ls -la"), RiskLevel::Blocked);
    }

    #[test]
    fn test_policy_flags_risky() {
        let policy = SecurityPolicy::default();
        assert_eq!(policy.classify("rm file.txt"), RiskLevel::Risky);
        assert_eq!(policy.classify("del file.txt"), RiskLevel::Risky);
        assert_eq!(policy.classify("curl https://example.com"), RiskLevel::Risky);
    }

    #[test]
    fn test_policy_allows_safe() {
        let policy = SecurityPolicy::default();
        assert_eq!(policy.classify("cargo build"), RiskLevel::Allowed);
        assert_eq!(policy.classify("python script.py"), RiskLevel::Allowed);
        assert_eq!(policy.classify("node app.js"), RiskLevel::Allowed);
        assert_eq!(policy.classify("go test ./..."), RiskLevel::Allowed);
    }

    #[test]
    fn test_policy_deny_takes_priority_over_allow() {
        let mut policy = SecurityPolicy::default();
        policy.allowlist.push("rm".into());
        assert_eq!(policy.classify("rm -rf /"), RiskLevel::Blocked);
    }

    #[test]
    fn test_policy_empty_allowlist_allows_all() {
        let mut policy = SecurityPolicy::default();
        policy.allowlist.clear();
        assert_eq!(policy.classify("cargo build"), RiskLevel::Allowed);
        assert_eq!(policy.classify("echo hello"), RiskLevel::Allowed);
        assert_eq!(policy.classify("rm -rf /"), RiskLevel::Blocked);
    }

    #[test]
    fn test_risky_description() {
        let policy = SecurityPolicy::default();
        let desc = policy.risky_description("rm file.txt");
        assert!(desc.is_some());
        assert!(desc.unwrap().contains("deletion"));
    }

    #[test]
    fn test_risky_description_none_for_safe() {
        let policy = SecurityPolicy::default();
        assert!(policy.risky_description("cargo build").is_none());
    }

    #[test]
    fn test_policy_with_confirmation_disabled() {
        let policy = SecurityPolicy::default().with_risky_confirmation(false);
        assert!(!policy.require_confirmation);
    }

    #[test]
    fn test_extract_command_name_basic() {
        assert_eq!(extract_command_name("Cargo Build"), "cargo");
        assert_eq!(extract_command_name("  python3  script.py"), "python3");
    }

    #[test]
    fn test_extract_command_name_path_stem() {
        assert_eq!(extract_command_name("C:\\Windows\\System32\\format.com"), "format");
        assert_eq!(extract_command_name("/usr/bin/rm"), "rm");
    }

    #[test]
    fn test_matches_pattern_single_word() {
        assert!(matches_pattern("format C:", "format"));
        assert!(!matches_pattern("cargo build", "format"));
    }

    #[test]
    fn test_matches_pattern_multi_word() {
        assert!(matches_pattern("rm -rf /home", "rm -rf"));
        assert!(!matches_pattern("rm file.txt", "rm -rf"));
        assert!(matches_pattern("git push origin main", "git push"));
        assert!(!matches_pattern("git status", "git push"));
    }

    #[test]
    fn test_policy_classify_case_insensitive() {
        let policy = SecurityPolicy::default();
        assert_eq!(policy.classify("FORMAT c:"), RiskLevel::Blocked);
        assert_eq!(policy.classify("Cargo Build"), RiskLevel::Allowed);
    }

    #[test]
    fn test_policy_classify_path_with_spaces() {
        let policy = SecurityPolicy::default();
        assert_eq!(policy.classify("cargo build"), RiskLevel::Allowed);
        assert_eq!(policy.classify("git push"), RiskLevel::Risky);
    }
}
