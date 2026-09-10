use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDiagnosis {
    pub error_type: String,
    pub original_command: String,
    pub root_cause: String,
    pub recommended_fix_command: Option<String>,
}

pub struct SelfHealingEngine;

impl SelfHealingEngine {
    pub fn diagnose_and_repair(
        command: &str,
        stderr: &str,
        exit_code: i32,
    ) -> Option<ErrorDiagnosis> {
        if exit_code == 0 {
            return None;
        }

        if stderr.contains("CommandNotFound")
            || stderr.contains("is not recognized")
            || stderr.contains("not found")
        {
            let pkg = command.split_whitespace().next().unwrap_or(command);
            return Some(ErrorDiagnosis {
                error_type: "MissingDependency".to_string(),
                original_command: command.to_string(),
                root_cause: format!("Executable '{}' is missing on PATH", pkg),
                recommended_fix_command: Some(format!("winget install --id {}", pkg)),
            });
        }

        if stderr.contains("PermissionDenied") || stderr.contains("Access is denied") {
            return Some(ErrorDiagnosis {
                error_type: "PermissionDenied".to_string(),
                original_command: command.to_string(),
                root_cause: "Operation requires elevated permissions or path access grant"
                    .to_string(),
                recommended_fix_command: Some(format!("sudo {}", command)),
            });
        }

        Some(ErrorDiagnosis {
            error_type: "ExecutionFailure".to_string(),
            original_command: command.to_string(),
            root_cause: format!("Command returned exit code {}", exit_code),
            recommended_fix_command: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_self_healing_diagnosis() {
        let stderr = "'git' is not recognized as an internal or external command";
        let diag = SelfHealingEngine::diagnose_and_repair("git status", stderr, 1);
        assert!(diag.is_some());
        let d = diag.unwrap();
        assert_eq!(d.error_type, "MissingDependency");
        assert!(
            d.recommended_fix_command
                .unwrap()
                .contains("winget install")
        );
    }
}
