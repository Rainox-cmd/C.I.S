pub const SECRET_PATTERNS: &[(&str, &str)] = &[
    // API keys and tokens
    ("OpenAI API key", r"sk-[a-zA-Z0-9]{20,}"),
    ("GitHub token", r"gh[pousr]_[A-Za-z0-9]{36,}"),
    ("Generic bearer token", r"(?i)bearer\s+[a-zA-Z0-9._\-]{10,}"),
    ("AWS access key", r"AKIA[0-9A-Z]{16}"),
    ("AWS secret key", r"(?i)aws_secret_access_key\s*[=:]\s*[A-Za-z0-9/+=]{40}"),
    // Passwords in command lines
    ("password=", r"(?i)password=[^\s]+"),
    ("passwd=", r"(?i)passwd=[^\s]+"),
    ("api_key=", r"(?i)api[_-]?key=[^\s]+"),
    ("token=", r"(?i)token=[^\s]+"),
    ("secret=", r"(?i)secret=[^\s]+"),
    // Common env var patterns
    ("API_KEY env", r"(?i)\b[A-Z_]*API[_-]?[A-Z_]*KEY[A-Z_]*\b\s*[=:]\s*\S+"),
    ("SECRET env", r"(?i)\b[A-Z_]*SECRET[A-Z_]*\b\s*[=:]\s*\S+"),
    ("PASSWORD env", r"(?i)\b[A-Z_]*PASSWORD[A-Z_]*\b\s*[=:]\s*\S+"),
    ("TOKEN env", r"(?i)\b[A-Z_]*TOKEN[A-Z_]*\b\s*[=:]\s*\S+"),
];

pub fn redact_secrets(text: &str) -> String {
    let mut result = text.to_string();
    for (label, pattern) in SECRET_PATTERNS {
        if let Ok(re) = regex::Regex::new(pattern) {
            result = re.replace_all(&result, |_caps: &regex::Captures| {
                format!("[REDACTED:{}]", label)
            }).to_string();
        }
    }
    result
}

const SENSITIVE_VAR_NAMES: &[&str] = &[
    "PASSWORD",
    "PASSPHRASE",
    "SECRET",
    "SECRET_KEY",
    "API_KEY",
    "API_SECRET",
    "TOKEN",
    "ACCESS_TOKEN",
    "REFRESH_TOKEN",
    "PRIVATE_KEY",
    "CREDENTIALS",
    "CLIENT_SECRET",
    "NVIDIA_API_KEY",
    "OPENAI_API_KEY",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "DATABASE_URL",
    "DB_PASSWORD",
    "NIM_API_KEY",
];

pub fn filter_env_vars(
    current_env: &[(String, String)],
) -> Vec<(String, String)> {
    current_env
        .iter()
        .filter(|(k, _)| !is_sensitive_var(k))
        .cloned()
        .collect()
}

pub fn is_sensitive_var(name: &str) -> bool {
    let upper = name.to_uppercase();
    for sensitive in SENSITIVE_VAR_NAMES {
        if upper == *sensitive || upper.contains(sensitive) {
            return true;
        }
    }
    false
}

pub fn redact_env_value(name: &str, value: &str, enabled: bool) -> String {
    if enabled && is_sensitive_var(name) {
        "[REDACTED]".to_string()
    } else {
        value.to_string()
    }
}

pub fn build_audit_safe_command(cmd: &str, args: &[String]) -> String {
    let mut parts = vec![cmd.to_string()];
    for arg in args {
        if is_sensitive_arg(arg) {
            parts.push("[REDACTED]".to_string());
        } else {
            parts.push(arg.clone());
        }
    }
    parts.join(" ")
}

fn is_sensitive_arg(arg: &str) -> bool {
    let lower = arg.to_lowercase();
    lower.contains("password")
        || lower.contains("token")
        || lower.contains("secret")
        || lower.contains("api_key")
        || lower.contains("apikey")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_redact_openai_key() {
        let input = "key=sk-abc123def456ghi789jkl012mno345pqr";
        let result = redact_secrets(input);
        assert!(result.contains("[REDACTED:"));
        assert!(!result.contains("sk-abc123"));
    }

    #[test]
    fn test_redact_github_token() {
        let input = "token=ghp_abcdefghijklmnopqrstuvwxyz0123456789AB";
        let result = redact_secrets(input);
        assert!(result.contains("[REDACTED:"));
        assert!(!result.contains("ghp_abcdef"));
    }

    #[test]
    fn test_redact_password_in_text() {
        let input = "Connecting with password=hunter2 to server";
        let result = redact_secrets(input);
        assert!(!result.contains("hunter2"));
        assert!(result.contains("[REDACTED:"));
    }

    #[test]
    fn test_redact_aws_key() {
        let input = "AWS key: AKIAIOSFODNN7EXAMPLE";
        let result = redact_secrets(input);
        assert!(result.contains("[REDACTED:"));
        assert!(!result.contains("AKIAIOSFODNN7EXAMPLE"));
    }

    #[test]
    fn test_redact_no_false_positive() {
        let input = "cargo build --release";
        let result = redact_secrets(input);
        assert_eq!(result, "cargo build --release");
    }

    #[test]
    fn test_is_sensitive_var() {
        assert!(is_sensitive_var("PASSWORD"));
        assert!(is_sensitive_var("API_KEY"));
        assert!(is_sensitive_var("nvidia_api_key"));
        assert!(is_sensitive_var("AWS_SECRET_ACCESS_KEY"));
        assert!(!is_sensitive_var("PATH"));
        assert!(!is_sensitive_var("HOME"));
        assert!(!is_sensitive_var("CARGO_HOME"));
    }

    #[test]
    fn test_filter_env_vars() {
        let env = vec![
            ("PATH".to_string(), "/usr/bin".to_string()),
            ("API_KEY".to_string(), "sk-abc123".to_string()),
            ("HOME".to_string(), "/home/user".to_string()),
            ("SECRET".to_string(), "topsecret".to_string()),
            ("NVIDIA_API_KEY".to_string(), "key123".to_string()),
        ];
        let result = filter_env_vars(&env);
        let map: HashMap<_, _> = result.into_iter().collect();
        assert!(map.contains_key("PATH"));
        assert!(map.contains_key("HOME"));
        assert!(!map.contains_key("API_KEY"));
        assert!(!map.contains_key("SECRET"));
        assert!(!map.contains_key("NVIDIA_API_KEY"));
    }

    #[test]
    fn test_redact_env_value() {
        assert_eq!(
            redact_env_value("API_KEY", "secret123", true),
            "[REDACTED]"
        );
        assert_eq!(
            redact_env_value("API_KEY", "secret123", false),
            "secret123"
        );
        assert_eq!(
            redact_env_value("PATH", "/usr/bin", true),
            "/usr/bin"
        );
    }

    #[test]
    fn test_build_audit_safe_command() {
        let args = vec!["--password=hunter2".to_string(), "build".to_string()];
        let result = build_audit_safe_command("curl", &args);
        assert!(!result.contains("hunter2"));
        assert!(result.contains("[REDACTED]"));
    }

    #[test]
    fn test_build_audit_safe_command_no_secrets() {
        let args = vec!["--release".to_string(), "build".to_string()];
        let result = build_audit_safe_command("cargo", &args);
        assert_eq!(result, "cargo --release build");
    }
}
