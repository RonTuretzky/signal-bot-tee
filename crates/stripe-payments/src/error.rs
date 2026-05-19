//! Stripe payment error types.

use thiserror::Error;

/// Errors that can occur in the Stripe payment system.
#[derive(Error, Debug)]
pub enum StripeError {
    /// User has exceeded their daily message limit.
    #[error("Daily message limit reached ({limit} messages). Upgrade your plan: {upgrade_url}")]
    DailyLimitReached { limit: u32, upgrade_url: String },

    /// Subscription is not active.
    #[error("Subscription inactive: {0}")]
    SubscriptionInactive(String),

    /// Stripe API error.
    #[error("Stripe API error: {0}")]
    StripeApi(String),

    /// Webhook signature verification failed.
    #[error("Invalid webhook signature")]
    InvalidSignature,

    /// Encryption/decryption error.
    #[error("Encryption error: {0}")]
    Encryption(String),

    /// Storage I/O error.
    #[error("Storage error: {0}")]
    Storage(String),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// User not found.
    #[error("User not found: {0}")]
    UserNotFound(String),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<std::io::Error> for StripeError {
    fn from(e: std::io::Error) -> Self {
        StripeError::Storage(e.to_string())
    }
}

impl From<aes_gcm::Error> for StripeError {
    fn from(_: aes_gcm::Error) -> Self {
        StripeError::Encryption("AES-GCM operation failed".to_string())
    }
}

impl From<reqwest::Error> for StripeError {
    fn from(e: reqwest::Error) -> Self {
        StripeError::StripeApi(e.to_string())
    }
}
