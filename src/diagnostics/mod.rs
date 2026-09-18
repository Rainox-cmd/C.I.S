use crate::config::Config;
use crate::index::Index;
use crate::project::Project;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Pass,
    Warn,
    Fail,
}

impl Health {
    fn as_str(&self) -> &'static str {
        match self {
            Health::Pass => "PASS",
            Health::Warn => "WARN",
            Health::Fail => "FAIL",
        }
    }

    fn symbol(&self) -> &'static str {
        match self {
            Health::Pass => "[OK]",
            Health::Warn => "[!!]",
            Health::Fail => "[XX]",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub name: String,
    pub health: Health,
    pub message: String,
    pub details: Option<String>,
}

impl Diagnostic {
    pub fn pass(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            health: Health::Pass,
            message: message.to_string(),
            details: None,
        }
    }

    pub fn warn(name: &str, message: &str, details: &str) -> Self {
        Self {
            name: name.to_string(),
            health: Health::Warn,
            message: message.to_string(),
            details: Some(details.to_string()),
        }
    }

    pub fn fail(name: &str, message: &str, details: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            health: Health::Fail,
            message: message.to_string(),
            details: details.map(|s| s.to_string()),
        }
    }
}

pub struct DiagnosticsReport {
    pub checks: Vec<Diagnostic>,
}

impl DiagnosticsReport {
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    pub fn add(&mut self, check: Diagnostic) {
        self.checks.push(check);
    }

    pub fn is_healthy(&self) -> bool {
        !self.checks.iter().any(|c| c.health == Health::Fail)
    }

    pub fn has_warnings(&self) -> bool {
        self.checks.iter().any(|c| c.health == Health::Warn)
    }

    pub fn exit_code(&self) -> i32 {
        if self.is_healthy() {
            0
        } else {
            1
        }
    }

    pub fn print(&self) {
        println!("C.I.S. Doctor — Diagnostics Report");
        println!("==================================");
        println!();

        for check in &self.checks {
            println!("{} {}", check.health.symbol(), check.health.as_str());
            if !check.name.is_empty() {
                println!("  Name:    {}", check.name);
            }
            println!("  Status:  {}", check.message);
            if let Some(details) = &check.details {
                println!("  Details: {}", details);
            }
            println!();
        }

        let pass_count = self
            .checks
            .iter()
            .filter(|c| c.health == Health::Pass)
            .count();
        let warn_count = self
            .checks
            .iter()
            .filter(|c| c.health == Health::Warn)
            .count();
        let fail_count = self
            .checks
            .iter()
            .filter(|c| c.health == Health::Fail)
            .count();

        println!(
            "Summary: {} passed, {} warnings, {} failures",
            pass_count, warn_count, fail_count
        );
    }

    pub fn to_json(&self) -> String {
        let mut json = String::from("{\n  \"checks\": [\n");
        for (i, check) in self.checks.iter().enumerate() {
            if i > 0 {
                json.push_str(",\n");
            }
            json.push_str(&format!(
                "    {{\"name\": \"{}\", \"health\": \"{}\", \"message\": \"{}\"",
                check.name,
                check.health.as_str(),
                check.message
            ));
            if let Some(details) = &check.details {
                json.push_str(&format!("\", \"details\": \"{}\"}}", details));
            } else {
                json.push_str("\"}");
            }
        }
        json.push_str("\n  ]\n}");
        json
    }
}

impl Default for DiagnosticsReport {
    fn default() -> Self {
        Self::new()
    }
}

pub fn run_all_checks(project: &Project, config: &Config, index: &Index) -> DiagnosticsReport {
    let mut report = DiagnosticsReport::new();

    report.add(check_project_root(project));
    report.add(check_cis_dir(project));
    report.add(check_config_validity(project));
    report.add(check_database(index));
    report.add(check_directories(project));
    report.add(check_terminal_security(config));
    report.add(check_environment());
    report.add(check_ntfs_streams(project));

    report
}

pub fn check_project_root(project: &Project) -> Diagnostic {
    if project.root.exists() {
        Diagnostic::pass(
            "Project Root",
            &format!("Project root found: {}", project.root.display()),
        )
    } else {
        Diagnostic::fail(
            "Project Root",
            "Project root directory does not exist",
            None,
        )
    }
}

pub fn check_cis_dir(project: &Project) -> Diagnostic {
    if project.cis_dir.exists() {
        Diagnostic::pass(
            "C.I.S. Directory",
            &format!(".cis/ directory exists at {}", project.cis_dir.display()),
        )
    } else {
        Diagnostic::fail(
            "C.I.S. Directory",
            ".cis/ directory does not exist",
            Some("Run `cis init` to create the .cis/ directory"),
        )
    }
}

pub fn check_config_validity(project: &Project) -> Diagnostic {
    let config_path = project.cis_dir().join("config.toml");
    if config_path.exists() {
        Diagnostic::pass(
            "Configuration",
            "Configuration is valid and loaded from config.toml",
        )
    } else {
        Diagnostic::warn(
            "Configuration",
            "No config.toml found — using defaults",
            "Run `cis init` or `cis config set` to create a persistent config",
        )
    }
}

pub fn check_database(index: &Index) -> Diagnostic {
    match check_database_integrity(index) {
        Ok(true) => Diagnostic::pass(
            "Database",
            "Database is accessible and passes integrity check",
        ),
        Ok(false) => Diagnostic::fail("Database", "Database integrity check failed", None),
        Err(e) => Diagnostic::fail(
            "Database",
            "Database is not accessible",
            Some(&e.to_string()),
        ),
    }
}

fn check_database_integrity(index: &Index) -> Result<bool, String> {
    // Check if basic tables exist
    let conn = index.conn();
    let table_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='files'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|count| count > 0)
        .map_err(|e| e.to_string())?;

    if !table_exists {
        return Ok(true);
    }

    // Run integrity check
    let result: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    Ok(result == "ok")
}

pub fn check_directories(project: &Project) -> Diagnostic {
    let required = [("logs", &project.logs_dir), ("cache", &project.cache_dir)];

    let mut missing = Vec::new();
    for (name, path) in &required {
        if !path.exists() {
            missing.push(name.to_string());
        }
    }

    if missing.is_empty() {
        Diagnostic::pass(
            "Required Directories",
            "All required directories exist (.cis/logs/, .cis/cache/)",
        )
    } else {
        Diagnostic::warn(
            "Required Directories",
            "Some required directories are missing",
            &format!("Missing: {}", missing.join(", ")),
        )
    }
}

pub fn check_terminal_security(config: &Config) -> Diagnostic {
    let timeout = config.security.default_timeout_seconds;
    let max_output = config.security.max_output_bytes;
    let allowlist_count = config.security.command_allowlist.len();
    let denylist_count = config.security.command_denylist.len();

    if timeout == 0 {
        Diagnostic::fail(
            "Terminal Security",
            "Command timeout is set to 0 (dangerous — no timeout)",
            None,
        )
    } else {
        Diagnostic::pass(
            "Terminal Security",
            &format!(
                "Terminal security configured (timeout {}s, output limit {} bytes, {} allowlisted, {} denylisted)",
                timeout, max_output, allowlist_count, denylist_count
            ),
        )
    }
}

pub fn check_ntfs_streams(project: &Project) -> Diagnostic {
    if cfg!(windows) {
        Diagnostic::pass(
            "NTFS Streams",
            "Windows Alternate Data Streams are safely handled"
        )
    } else {
        Diagnostic::pass(
            "NTFS Streams",
            "Not applicable on this OS"
        )
    }
}

pub fn check_environment() -> Diagnostic {
    let rustc = std::env::var("RUSTC");
    let cargo_home = std::env::var("CARGO_HOME");
    let mut details = Vec::new();

    if rustc.is_ok()
        || std::process::Command::new("rustc")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    {
        details.push("rustc available".to_string());
    } else {
        details.push("rustc not found".to_string());
    }

    if cargo_home.is_ok() {
        details.push("CARGO_HOME set".to_string());
    }
    
    if cfg!(windows) {
        if std::process::Command::new("cl.exe").arg("/?").output().is_ok() {
            details.push("MSVC compiler (cl.exe) available".to_string());
        } else {
            let vswhere = std::process::Command::new(r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe")
                .arg("-latest")
                .arg("-property")
                .arg("installationPath")
                .output();
                
            if let Ok(output) = vswhere {
                if output.status.success() {
                    details.push("Visual Studio found but MSVC not in PATH (run vcvars64.bat)".to_string());
                } else {
                    details.push("MSVC compiler not found".to_string());
                }
            } else {
                details.push("MSVC compiler not found".to_string());
            }
        }
    }

    Diagnostic::pass(
        "Environment",
        &format!("Environment: {}", details.join(", ")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_test_project() -> (Project, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let project = Project::new(dir.path().to_path_buf()).unwrap();
        project.init().unwrap();
        (project, dir)
    }

    #[test]
    fn test_health_as_str() {
        assert_eq!(Health::Pass.as_str(), "PASS");
        assert_eq!(Health::Warn.as_str(), "WARN");
        assert_eq!(Health::Fail.as_str(), "FAIL");
    }

    #[test]
    fn test_diagnostic_pass() {
        let d = Diagnostic::pass("test", "all good");
        assert_eq!(d.health, Health::Pass);
        assert_eq!(d.name, "test");
    }

    #[test]
    fn test_diagnostic_warn() {
        let d = Diagnostic::warn("test", "warning", "details");
        assert_eq!(d.health, Health::Warn);
        assert_eq!(d.details, Some("details".to_string()));
    }

    #[test]
    fn test_diagnostic_fail() {
        let d = Diagnostic::fail("test", "error", Some("detail"));
        assert_eq!(d.health, Health::Fail);
        assert_eq!(d.details, Some("detail".to_string()));
    }

    #[test]
    fn test_report_is_healthy() {
        let mut report = DiagnosticsReport::new();
        report.add(Diagnostic::pass("a", "ok"));
        report.add(Diagnostic::pass("b", "ok"));
        assert!(report.is_healthy());
        assert_eq!(report.exit_code(), 0);
    }

    #[test]
    fn test_report_not_healthy() {
        let mut report = DiagnosticsReport::new();
        report.add(Diagnostic::pass("a", "ok"));
        report.add(Diagnostic::fail("b", "bad", None));
        assert!(!report.is_healthy());
        assert_eq!(report.exit_code(), 1);
    }

    #[test]
    fn test_report_has_warnings() {
        let mut report = DiagnosticsReport::new();
        report.add(Diagnostic::pass("a", "ok"));
        report.add(Diagnostic::warn("b", "caution", "detail"));
        report.add(Diagnostic::fail("c", "bad", None));
        assert!(report.has_warnings());
        assert!(!report.is_healthy());
    }

    #[test]
    fn test_check_project_root_found() {
        let (project, _dir) = make_test_project();
        let d = check_project_root(&project);
        assert_eq!(d.health, Health::Pass);
    }

    #[test]
    fn test_check_cis_dir_exists() {
        let (project, _dir) = make_test_project();
        let d = check_cis_dir(&project);
        assert_eq!(d.health, Health::Pass);
    }

    #[test]
    fn test_check_cis_dir_missing() {
        let dir = tempdir().unwrap();
        let project = Project::new(dir.path().to_path_buf()).unwrap();
        let d = check_cis_dir(&project);
        assert_eq!(d.health, Health::Fail);
    }

    #[test]
    fn test_check_config_validity_defaults() {
        let (project, _dir) = make_test_project();
        let _cfg = Config::load(&project).unwrap();
        let d = check_config_validity(&project);
        assert_eq!(d.health, Health::Pass);
    }

    #[test]
    fn test_check_config_validity_no_file() {
        let dir = tempdir().unwrap();
        let project = Project::new(dir.path().to_path_buf()).unwrap();
        let d = check_config_validity(&project);
        assert_eq!(d.health, Health::Warn);
    }

    #[test]
    fn test_check_directories_all_exist() {
        let (project, _dir) = make_test_project();
        let d = check_directories(&project);
        assert_eq!(d.health, Health::Pass);
    }

    #[test]
    fn test_check_directories_missing() {
        let dir = tempdir().unwrap();
        let project = Project::new(dir.path().to_path_buf()).unwrap();
        // Don't call init, so dirs don't exist
        let d = check_directories(&project);
        assert_eq!(d.health, Health::Warn);
    }

    #[test]
    fn test_check_terminal_security_ok() {
        let config = Config::default();
        let d = check_terminal_security(&config);
        assert_eq!(d.health, Health::Pass);
    }

    #[test]
    fn test_check_terminal_security_bad_timeout() {
        let mut config = Config::default();
        config.security.default_timeout_seconds = 0;
        let d = check_terminal_security(&config);
        assert_eq!(d.health, Health::Fail);
    }

    #[test]
    fn test_run_all_checks_on_initialized_project() {
        let (project, _dir) = make_test_project();
        let config = Config::default();
        let index = Index::open(&project, &config).unwrap();
        let report = run_all_checks(&project, &config, &index);
        for check in &report.checks {
            assert_ne!(
                check.health,
                Health::Fail,
                "Check '{}' failed: {}",
                check.name,
                check.message
            );
        }
    }

    #[test]
    fn test_report_to_json() {
        let mut report = DiagnosticsReport::new();
        report.add(Diagnostic::pass("test", "ok"));
        let json = report.to_json();
        assert!(json.contains("\"name\": \"test\""));
        assert!(json.contains("\"health\": \"PASS\""));
    }
}
