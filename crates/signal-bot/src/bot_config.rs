//! Per-bot configuration provider.
//!
//! Fetches per-account config (model, system_prompt) from the registration proxy
//! and caches results with a TTL. Falls back to global defaults when the proxy
//! is unreachable or has no config for a given number.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Per-bot configuration.
#[derive(Clone, Debug)]
pub struct BotConfig {
    pub model: Option<String>,
    pub system_prompt: Option<String>,
}

/// Wire format from `GET /v1/bots/:number`.
#[derive(Deserialize)]
struct BotConfigResponse {
    #[allow(dead_code)]
    phone_number: String,
    model: Option<String>,
    system_prompt: Option<String>,
    #[allow(dead_code)]
    message: String,
}

struct CachedConfig {
    config: BotConfig,
    fetched_at: Instant,
}

pub struct BotConfigProvider {
    client: reqwest::Client,
    proxy_url: String,
    cache: Arc<RwLock<HashMap<String, CachedConfig>>>,
    cache_ttl: Duration,
    default_model: String,
    default_system_prompt: String,
}

impl BotConfigProvider {
    pub fn new(proxy_url: &str, default_model: &str, default_system_prompt: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
            proxy_url: proxy_url.trim_end_matches('/').to_string(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(300), // 5 minutes
            default_model: default_model.to_string(),
            default_system_prompt: default_system_prompt.to_string(),
        }
    }

    /// Get config for a specific phone number, with caching and fallback.
    pub async fn get_config(&self, phone_number: &str) -> BotConfig {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(phone_number) {
                if cached.fetched_at.elapsed() < self.cache_ttl {
                    debug!("Using cached config for {}", &phone_number[..phone_number.len().min(8)]);
                    return cached.config.clone();
                }
            }
        }

        // Fetch from proxy
        let config = self.fetch_config(phone_number).await;

        // Cache the result
        {
            let mut cache = self.cache.write().await;
            cache.insert(
                phone_number.to_string(),
                CachedConfig {
                    config: config.clone(),
                    fetched_at: Instant::now(),
                },
            );
        }

        config
    }

    /// Convenience: get the system prompt for a phone number.
    pub async fn get_system_prompt(&self, phone_number: &str) -> String {
        self.get_config(phone_number)
            .await
            .system_prompt
            .unwrap_or_else(|| self.default_system_prompt.clone())
    }

    /// Convenience: get the model for a phone number.
    pub async fn get_model(&self, phone_number: &str) -> String {
        self.get_config(phone_number)
            .await
            .model
            .unwrap_or_else(|| self.default_model.clone())
    }

    async fn fetch_config(&self, phone_number: &str) -> BotConfig {
        let encoded = urlencoding::encode(phone_number);
        let url = format!("{}/v1/bots/{}", self.proxy_url, encoded);

        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<BotConfigResponse>().await {
                    Ok(data) => {
                        debug!("Fetched config for {}", &phone_number[..phone_number.len().min(8)]);
                        BotConfig {
                            model: data.model,
                            system_prompt: data.system_prompt,
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse config for {}: {}", &phone_number[..phone_number.len().min(8)], e);
                        self.default_config()
                    }
                }
            }
            Ok(resp) => {
                debug!("No config found for {} ({})", &phone_number[..phone_number.len().min(8)], resp.status());
                self.default_config()
            }
            Err(e) => {
                warn!("Failed to fetch config: {}", e);
                self.default_config()
            }
        }
    }

    fn default_config(&self) -> BotConfig {
        BotConfig {
            model: Some(self.default_model.clone()),
            system_prompt: Some(self.default_system_prompt.clone()),
        }
    }
}
