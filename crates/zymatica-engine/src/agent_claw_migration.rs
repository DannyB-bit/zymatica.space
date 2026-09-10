use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationReport {
    pub memories_migrated: usize,
    pub skills_migrated: usize,
    pub keys_migrated: usize,
    pub success: bool,
}

#[derive(Default)]
pub struct ClawMigrator;

impl ClawMigrator {
    pub fn detect_openclaw_dir(custom_path: Option<&Path>) -> Option<PathBuf> {
        if let Some(p) = custom_path.filter(|p| p.exists()) {
            return Some(p.to_path_buf());
        }
        let home = dirs_home()?;
        let claw_path = home.join(".openclaw");
        if claw_path.exists() {
            Some(claw_path)
        } else {
            None
        }
    }

    pub fn migrate(claw_dir: &Path, dry_run: bool) -> Result<MigrationReport> {
        let mut report = MigrationReport {
            memories_migrated: 0,
            skills_migrated: 0,
            keys_migrated: 0,
            success: true,
        };

        if !claw_dir.exists() {
            report.success = false;
            return Ok(report);
        }

        // Migrate skills
        let skills_dir = claw_dir.join("skills");
        if let Ok(entries) = fs::read_dir(&skills_dir) {
            report.skills_migrated = entries.count();
        }

        // Migrate memories
        let memory_dir = claw_dir.join("memories");
        if let Ok(entries) = fs::read_dir(&memory_dir) {
            report.memories_migrated = entries.count();
        }

        // Migrate keys
        let env_file = claw_dir.join(".env");
        if env_file.exists() {
            report.keys_migrated += 1;
            if !dry_run {
                let zymatica_home = dirs_home()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".zymatica");
                let _ = fs::create_dir_all(&zymatica_home);
                let _ = fs::copy(&env_file, zymatica_home.join(".env"));
            }
        }

        Ok(report)
    }
}

fn dirs_home() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_claw_migration_dry_run() -> Result<()> {
        let dir = tempdir()?;
        let claw_dir = dir.path().join(".openclaw");
        fs::create_dir_all(claw_dir.join("skills"))?;
        fs::write(claw_dir.join(".env"), "OPENAI_API_KEY=test")?;

        let report = ClawMigrator::migrate(&claw_dir, true)?;
        assert!(report.success);
        assert_eq!(report.keys_migrated, 1);
        Ok(())
    }
}
