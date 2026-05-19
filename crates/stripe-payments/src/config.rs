//! Stripe payment configuration.

use serde::Deserialize;
use std::path::PathBuf;

/// Stripe payment configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct StripeConfig {
    /// Whether Stripe payments are enabled.
    #[serde(default)]
    pub enabled: bool,

    /// Stripe secret key (sk_live_... or sk_test_...).
    pub secret_key: String,

    /// Stripe webhook signing secret (whsec_...).
    pub webhook_secret: String,

    /// HTTP server port for webhook + API.
    #[serde(default = "default_server_port")]
    pub server_port: u16,

    /// Storage path for encrypted subscription store.
    #[serde(default = "default_storage_path")]
    pub storage_path: PathBuf,

    /// Number of free messages per day (for users without a subscription).
    #[serde(default = "default_free_daily_messages")]
    pub free_daily_messages: u32,

    /// Stripe Price ID for the Premium plan.
    pub premium_price_id: Option<String>,

    /// Stripe Price ID for the Pro plan.
    pub pro_price_id: Option<String>,

    /// Base URL for success/cancel redirects (web frontend URL).
    #[serde(default = "default_frontend_url")]
    pub frontend_url: String,
}

fn default_server_port() -> u16 {
    8082
}

fn default_storage_path() -> PathBuf {
    PathBuf::from("/data/subscriptions.enc")
}

fn default_free_daily_messages() -> u32 {
    15
}

fn default_frontend_url() -> String {
    "https://signal-tee-web.vercel.app".to_string()
}

impl Default for StripeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            secret_key: String::new(),
            webhook_secret: String::new(),
            server_port: default_server_port(),
            storage_path: default_storage_path(),
            free_daily_messages: default_free_daily_messages(),
            premium_price_id: None,
            pro_price_id: None,
            frontend_url: default_frontend_url(),
        }
    }
}

impl StripeConfig {
    /// Get all configured plan price IDs.
    pub fn plan_price_ids(&self) -> Vec<(&str, &str)> {
        let mut plans = Vec::new();
        if let Some(ref id) = self.premium_price_id {
            plans.push(("Premium", id.as_str()));
        }
        if let Some(ref id) = self.pro_price_id {
            plans.push(("Pro", id.as_str()));
        }
        plans
    }
}
