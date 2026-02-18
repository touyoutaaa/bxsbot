pub mod keywords;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::Result;

pub use keywords::KeywordConfig;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub crawler: CrawlerConfig,
    pub translator: TranslatorConfig,
    pub generator: GeneratorConfig,
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CrawlerConfig {
    pub max_papers_per_day: usize,
    pub request_delay_ms: u64,
    pub user_agent: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TranslatorConfig {
    pub api_provider: String,
    pub api_key: String,
    pub api_url: String,
    pub model: String,
    pub target_language: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GeneratorConfig {
    pub ppt_template: String,
    pub max_papers_per_report: usize,
    pub include_images: bool,
    pub include_formulas: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    pub database_path: String,
    pub cache_ttl_days: u32,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let config_path = PathBuf::from("config/settings.toml");

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(config_path)?;
        let config: AppConfig = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, path: &str) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            crawler: CrawlerConfig {
                max_papers_per_day: 50,
                request_delay_ms: 1000,
                user_agent: "ResearchBot/1.0".to_string(),
            },
            translator: TranslatorConfig {
                api_provider: "minimax".to_string(),
                api_key: "your-minimax-api-key".to_string(),
                api_url: "https://api.minimax.chat/v1/text/chatcompletion_v2".to_string(),
                model: "abab6.5-chat".to_string(),
                target_language: "zh-CN".to_string(),
            },
            generator: GeneratorConfig {
                ppt_template: "academic".to_string(),
                max_papers_per_report: 20,
                include_images: true,
                include_formulas: true,
            },
            storage: StorageConfig {
                database_path: "./data/papers.db".to_string(),
                cache_ttl_days: 30,
            },
        }
    }
}
