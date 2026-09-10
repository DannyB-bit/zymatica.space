use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZymaticaConfig {
    pub provider: String,
    pub model: String,
    pub portal_enabled: bool,
    pub active_tools: Vec<String>,
    pub zymatica_home: PathBuf,
}

impl Default for ZymaticaConfig {
    fn default() -> Self {
        Self {
            provider: "Zymatica Portal".to_string(),
            model: "gemma-4-e2b-q8".to_string(),
            portal_enabled: true,
            active_tools: vec![
                "read_file".to_string(),
                "write_to_file".to_string(),
                "terminal".to_string(),
                "grep_search".to_string(),
                "list_dir".to_string(),
            ],
            zymatica_home: get_default_zymatica_home(),
        }
    }
}

pub struct SetupWizard;

impl SetupWizard {
    pub fn run_setup(portal_flag: bool) -> Result<ZymaticaConfig> {
        let mut config = ZymaticaConfig::default();
        if portal_flag {
            config.provider = "Zymatica Portal".to_string();
            config.portal_enabled = true;
        }

        let target_dir = &config.zymatica_home;
        fs::create_dir_all(target_dir)
            .with_context(|| format!("Creating Zymatica home at {}", target_dir.display()))?;

        let config_file = target_dir.join("config.yaml");
        let yaml_content = format!(
            "# Zymatica Engine Configuration\nprovider: {}\nmodel: {}\nportal_enabled: {}\nactive_tools:\n  - {}\n",
            config.provider,
            config.model,
            config.portal_enabled,
            config.active_tools.join("\n  - ")
        );

        fs::write(&config_file, yaml_content)?;
        Ok(config)
    }
}

fn get_default_zymatica_home() -> PathBuf {
    if let Some(home) = dirs_home() {
        home.join(".zymatica")
    } else {
        PathBuf::from(".zymatica")
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
    fn test_setup_wizard_flow() -> Result<()> {
        let dir = tempdir()?;
        let config = ZymaticaConfig {
            zymatica_home: dir.path().to_path_buf(),
            ..Default::default()
        };
        assert_eq!(config.zymatica_home, dir.path());

        let cfg = SetupWizard::run_setup(true)?;
        assert!(cfg.portal_enabled);
        Ok(())
    }
}
