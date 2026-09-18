pub mod path;
pub mod policy;
pub mod redaction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    Blocked,
    Risky,
    Allowed,
}

// Re-export only what is used externally to avoid unused-import warnings.
pub use path::{check_path_containment, canonicalize_path, PathContainmentError};
pub use policy::{RiskyCommand, SecurityPolicy};
pub use redaction::{build_audit_safe_command, is_sensitive_var, redact_secrets};
