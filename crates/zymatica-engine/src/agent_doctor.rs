use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticCheck {
    pub name: String,
    pub passed: bool,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub overall_healthy: bool,
    pub checks: Vec<DiagnosticCheck>,
}

pub struct AgentDoctor;

impl AgentDoctor {
    pub fn run_diagnostics() -> DoctorReport {
        let mut checks = Vec::new();

        // Check 1: Git available
        let git_ok = Command::new("git").arg("--version").output().is_ok();
        checks.push(DiagnosticCheck {
            name: "Git Executable".to_string(),
            passed: git_ok,
            details: if git_ok {
                "Git CLI found on PATH"
            } else {
                "Git CLI missing"
            }
            .to_string(),
        });

        // Check 2: Cargo / Rust Compiler
        let rustc_ok = Command::new("rustc").arg("--version").output().is_ok();
        checks.push(DiagnosticCheck {
            name: "Rust Compiler (rustc)".to_string(),
            passed: rustc_ok,
            details: if rustc_ok {
                "Rust toolchain active"
            } else {
                "rustc not found"
            }
            .to_string(),
        });

        // Check 3: Workspace Directory
        let ws_ok = Path::new("src").exists() && Path::new("Cargo.toml").exists();
        checks.push(DiagnosticCheck {
            name: "Zymatica Workspace Root".to_string(),
            passed: ws_ok,
            details: if ws_ok {
                "Valid Cargo workspace root"
            } else {
                "Invalid workspace"
            }
            .to_string(),
        });

        let overall = checks.iter().all(|c| c.passed);
        DoctorReport {
            overall_healthy: overall,
            checks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doctor_diagnostics() {
        let report = AgentDoctor::run_diagnostics();
        assert!(!report.checks.is_empty());
        assert!(report.checks.iter().any(|c| c.name.contains("Rust")));
    }
}
