use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub author: Option<String>,
    pub version: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub metadata: SkillMetadata,
    pub path: PathBuf,
    pub instructions: String,
}

pub struct SkillStore {
    skills: HashMap<String, Skill>,
}

impl Default for SkillStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillStore {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    pub fn load_from_dir(&mut self, dir_path: &Path) -> Result<usize> {
        let mut count = 0;
        if !dir_path.exists() {
            return Ok(0);
        }

        for entry in fs::read_dir(dir_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let skill_md = path.join("SKILL.md");
                if let Ok(skill) = self.parse_skill_file(&skill_md) {
                    self.skills.insert(skill.metadata.name.clone(), skill);
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    pub fn get_skill(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    pub fn list_skills(&self) -> Vec<&SkillMetadata> {
        self.skills.values().map(|s| &s.metadata).collect()
    }

    pub fn format_prompt_injection(&self, skill_name: &str) -> Option<String> {
        self.get_skill(skill_name).map(|s| {
            format!(
                "### Skill: {}\nDescription: {}\n---\n{}",
                s.metadata.name, s.metadata.description, s.instructions
            )
        })
    }

    fn parse_skill_file(&self, path: &Path) -> Result<Skill> {
        let content = fs::read_to_string(path)?;
        let mut name = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed-skill".to_string());
        let mut description = "Custom skill".to_string();

        let mut body = content.clone();
        if content.starts_with("---") {
            let parts: Vec<&str> = content.splitn(3, "---").collect();
            if parts.len() >= 3 {
                for line in parts[1].lines() {
                    if let Some((k, v)) = line.split_once(':') {
                        let key = k.trim();
                        let val = v.trim().trim_matches('"').trim_matches('\'');
                        if key == "name" && !val.is_empty() {
                            name = val.to_string();
                        } else if key == "description" && !val.is_empty() {
                            description = val.to_string();
                        }
                    }
                }
                body = parts[2].trim().to_string();
            }
        }

        Ok(Skill {
            metadata: SkillMetadata {
                name,
                description,
                author: None,
                version: Some("1.0.0".to_string()),
                tags: vec![],
            },
            path: path.to_path_buf(),
            instructions: body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_skill_store_parsing() -> Result<()> {
        let dir = tempdir()?;
        let skill_dir = dir.path().join("my-skill");
        fs::create_dir(&skill_dir)?;
        let skill_md = skill_dir.join("SKILL.md");
        fs::write(
            &skill_md,
            "---\nname: test-skill\ndescription: Test skill description\n---\n# Instructions\nDo test tasks.",
        )?;

        let mut store = SkillStore::new();
        let count = store.load_from_dir(dir.path())?;
        assert_eq!(count, 1);

        let skill = store.get_skill("test-skill").unwrap();
        assert_eq!(skill.metadata.description, "Test skill description");
        assert!(skill.instructions.contains("Do test tasks."));
        Ok(())
    }
}
