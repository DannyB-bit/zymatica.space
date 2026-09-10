use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub plugin_type: String,
    pub entry_point: Option<String>,
}

pub struct PluginLoader {
    plugins: HashMap<String, PluginManifest>,
}

impl Default for PluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginLoader {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    pub fn discover_plugins(&mut self, plugins_dir: &Path) -> Result<usize> {
        let mut count = 0;
        if !plugins_dir.exists() {
            return Ok(0);
        }

        for entry in fs::read_dir(plugins_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let manifest_path = path.join("plugin.json");
                if let Ok(content) = fs::read_to_string(&manifest_path)
                    && let Ok(manifest) = serde_json::from_str::<PluginManifest>(&content)
                {
                    self.plugins.insert(manifest.name.clone(), manifest);
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    pub fn get_plugin(&self, name: &str) -> Option<&PluginManifest> {
        self.plugins.get(name)
    }

    pub fn list_plugins(&self) -> Vec<&PluginManifest> {
        self.plugins.values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_plugin_discovery() -> Result<()> {
        let dir = tempdir()?;
        let plugin_dir = dir.path().join("my-plugin");
        fs::create_dir_all(&plugin_dir)?;
        let manifest = r#"{
            "name": "kanban-dispatcher",
            "version": "1.0.0",
            "description": "Multi-agent kanban board worker",
            "plugin_type": "worker"
        }"#;
        fs::write(plugin_dir.join("plugin.json"), manifest)?;

        let mut loader = PluginLoader::new();
        let count = loader.discover_plugins(dir.path())?;
        assert_eq!(count, 1);
        assert!(loader.get_plugin("kanban-dispatcher").is_some());
        Ok(())
    }
}
