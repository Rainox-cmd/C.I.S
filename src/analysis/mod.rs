use crate::config::Config;
use crate::index::Index;
use crate::parser::ParserEngine;
use crate::project::Project;
use crate::scanner::Scanner;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum IssueCategory {
    ScanError,
    SyntaxError,
    DependencyCycle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Issue {
    pub category: IssueCategory,
    pub severity: Severity,
    pub message: String,
    pub file: Option<String>,
}

impl Issue {
    pub fn identity(&self) -> String {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentIssue {
    pub id: i64,
    pub identity: String,
    pub category: IssueCategory,
    pub severity: Severity,
    pub message: String,
    pub file: Option<String>,
    pub status: String,
    pub created_at: f64,
    pub updated_at: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisReport {
    pub project_path: String,
    pub issues: Vec<Issue>,
    pub informational: Vec<String>,
}

pub struct AnalysisEngine;

impl AnalysisEngine {
    pub fn run(project: &Project, config: &Config, index: &Index) -> Result<AnalysisReport> {
        let mut issues = Vec::new();
        let mut informational = Vec::new();

        // Step 1: Dependency cycles
        if let Ok(has_cycle) = index.has_cycle() {
            if has_cycle {
                issues.push(Issue {
                    category: IssueCategory::DependencyCycle,
                    severity: Severity::Warning,
                    message: "Dependency cycle detected in project dependency graph".to_string(),
                    file: None,
                });
            }
        }

        // Step 2: Entry points (Informational)
        if let Ok(entries) = index.get_entry_points() {
            let entry_points: Vec<String> = entries
                .into_iter()
                .filter(|e| e.incoming_count == 0)
                .map(|e| e.rel_path)
                .collect();
            if !entry_points.is_empty() {
                informational.push("Entry points:".to_string());
                for ep in entry_points {
                    informational.push(format!("- {}", ep));
                }
            }
        }

        // Step 3: Scanner
        let scanner = Scanner::new(
            project.root.clone(),
            config.scanner.respect_gitignore,
            config.scanner.respect_cisignore,
            config.general.max_file_size_bytes,
        );

        let scan_result = scanner.scan()?;
        for err in &scan_result.scan_errors {
            issues.push(Issue {
                category: IssueCategory::ScanError,
                severity: Severity::Error,
                message: err.clone(),
                file: None,
            });
        }

        // Step 4: Parser
        let supported = ["Python", "JavaScript", "TypeScript", "Rust", "Go"];
        for file in &scan_result.files {
            if supported.contains(&file.language.as_str()) {
                let file_path = project.root.join(&file.rel_path);
                if let Ok(content) = fs::read_to_string(&file_path) {
                    let parse_result =
                        ParserEngine::parse_file(&file_path, &content, &file.language);
                    if let Some(err) = parse_result.syntax_error {
                        issues.push(Issue {
                            category: IssueCategory::SyntaxError,
                            severity: Severity::Error,
                            message: err,
                            file: Some(file.rel_path.clone()),
                        });
                    }
                }
            }
        }

        Ok(AnalysisReport {
            project_path: project.root.to_string_lossy().to_string(),
            issues,
            informational,
        })
    }
}

#[cfg(test)]
mod tests;
