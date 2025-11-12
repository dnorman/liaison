use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub glob: GlobConfig,
}

#[derive(Debug, Deserialize)]
pub struct GlobConfig {
    #[serde(default)]
    pub include: Vec<String>,
    
    #[serde(default = "default_exclude")]
    pub exclude: Vec<String>,
}

fn default_exclude() -> Vec<String> {
    vec![
        "target/**".to_string(),
        "node_modules/**".to_string(),
    ]
}

impl Default for GlobConfig {
    fn default() -> Self {
        Self {
            include: Vec::new(),
            exclude: default_exclude(),
        }
    }
}

impl Config {
    pub fn load(repo_root: &Path) -> Result<Self> {
        let config_path = repo_root.join(".liaison.toml");
        
        if !config_path.exists() {
            return Ok(Config {
                glob: GlobConfig::default(),
            });
        }

        let content = std::fs::read_to_string(&config_path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}


