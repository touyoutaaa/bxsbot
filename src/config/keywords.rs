use serde::{Deserialize, Serialize};
use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Subscription {
    pub name: String,
    pub keywords: Vec<String>,
    pub sources: Vec<String>,
    pub categories: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeywordConfig {
    pub subscriptions: Vec<Subscription>,
}

impl KeywordConfig {
    pub fn load() -> Result<Self> {
        let config_path = PathBuf::from("config/keywords.toml");

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(config_path)?;
        let config: KeywordConfig = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn get_active_subscriptions(&self) -> Vec<&Subscription> {
        self.subscriptions.iter().filter(|s| s.enabled).collect()
    }
}

impl Default for KeywordConfig {
    fn default() -> Self {
        Self {
            subscriptions: vec![
                Subscription {
                    name: "机器学习".to_string(),
                    keywords: vec![
                        "machine learning".to_string(),
                        "deep learning".to_string(),
                        "neural network".to_string(),
                    ],
                    sources: vec!["arxiv".to_string(), "semantic_scholar".to_string()],
                    categories: vec!["cs.LG".to_string(), "cs.AI".to_string()],
                    enabled: true,
                },
            ],
        }
    }
}
