use crate::error::AppResult;
use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub telegram: TelegramConfig,
    pub postgres: PostgresConfig,
    #[serde(default)]
    pub redis: RedisConfig,
    #[serde(default)]
    pub keywords: KeywordsConfig,
    #[serde(default)]
    pub scraping: ScrapingConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TelegramConfig {
    pub bot_token: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PostgresConfig {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    #[serde(default = "default_redis_url")]
    pub url: String,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: default_redis_url(),
        }
    }
}

fn default_redis_url() -> String {
    "redis://127.0.0.1:6379".to_string()
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct KeywordsConfig {
    #[serde(default = "default_false")]
    pub track_rust: bool,
    #[serde(default = "default_false")]
    pub track_go: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ScrapingConfig {
    #[serde(default = "default_interval")]
    pub interval_seconds: u64,
}

impl Default for ScrapingConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 600,
        }
    }
}

fn default_false() -> bool {
    false
}

fn default_interval() -> u64 {
    600
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> AppResult<Self> {
        let mut file = File::open(path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_deserialize() {
        let yaml = r#"
telegram:
  bot_token: "test_token"
postgres:
  url: "postgres://job_notifier:password@127.0.0.1:5432/job_notifier"
redis:
  url: "redis://127.0.0.1:6379"
keywords:
  track_rust: true
  track_go: false
scraping:
  interval_seconds: 300
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap_or_else(|e| panic!("{}", e));
        assert_eq!(config.telegram.bot_token, "test_token");
        assert_eq!(
            config.postgres.url,
            "postgres://job_notifier:password@127.0.0.1:5432/job_notifier"
        );
        assert_eq!(config.redis.url, "redis://127.0.0.1:6379");
        assert!(config.keywords.track_rust);
        assert!(!config.keywords.track_go);
        assert_eq!(config.scraping.interval_seconds, 300);
    }
}
