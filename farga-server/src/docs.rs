use std::path::PathBuf;
use anyhow::Result;

pub struct DocsTree {
    root: PathBuf,
}

impl DocsTree {
    pub fn new(root: PathBuf) -> Self { Self { root } }

    pub fn read_org(&self) -> Result<String> {
        let p = self.root.join("org.md");
        Ok(if p.exists() { std::fs::read_to_string(p)? } else { String::new() })
    }

    pub fn read_initiatives(&self) -> Result<Vec<String>> {
        let dir = self.root.join("initiatives");
        if !dir.exists() { return Ok(vec![]); }
        let mut items = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().map_or(false, |e| e == "md") {
                items.push(std::fs::read_to_string(path)?);
            }
        }
        Ok(items)
    }

    pub fn read_project(&self, project: &str) -> Result<String> {
        let p = self.root.join("projects").join(project).join("project.md");
        Ok(if p.exists() { std::fs::read_to_string(p)? } else { String::new() })
    }

    pub fn read_component(&self, project: &str, component_path: &str) -> Result<String> {
        let p = self.root.join("projects").join(project).join(component_path).join("component.md");
        Ok(if p.exists() { std::fs::read_to_string(p)? } else { String::new() })
    }

    pub fn read_governance_config(&self) -> Result<String> {
        let p = self.root.join("governance.yaml");
        Ok(if p.exists() { std::fs::read_to_string(p)? } else { String::new() })
    }
}
