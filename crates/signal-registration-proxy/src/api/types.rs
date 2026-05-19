//! API request and response types.

use crate::registry::RegistrationStatus;
use serde::{Deserialize, Serialize};

/// Request to initiate phone number registration.
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    /// Optional CAPTCHA token if required by Signal
    pub captcha: Option<String>,

    /// Use voice call instead of SMS for verification code
    #[serde(default)]
    pub use_voice: bool,

    /// Optional ownership proof secret (will be hashed and stored)
    /// Required for later unregistration or re-registration
    pub ownership_secret: Option<String>,

    /// AI model to use for this bot
    pub model: Option<String>,

    /// System prompt for the AI assistant
    pub system_prompt: Option<String>,
}

/// Response after initiating registration.
#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub phone_number: String,
    pub status: String,
    pub message: String,
}

/// Request to verify registration with code.
#[derive(Debug, Deserialize)]
pub struct VerifyRequest {
    /// Optional Signal PIN to set
    pub pin: Option<String>,

    /// Ownership secret (must match what was provided during registration)
    pub ownership_secret: Option<String>,
}

/// Response after verification.
#[derive(Debug, Serialize)]
pub struct VerifyResponse {
    pub phone_number: String,
    pub status: String,
    pub message: String,
}

/// Phone number status response.
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub phone_number: String,
    pub status: RegistrationStatus,
    pub registered_at: Option<String>,
}

/// List of registered accounts.
#[derive(Debug, Serialize)]
pub struct AccountsResponse {
    pub accounts: Vec<AccountInfo>,
    pub total: usize,
}

/// Account info for listing.
#[derive(Debug, Serialize)]
pub struct AccountInfo {
    pub phone_number: String,
    pub status: RegistrationStatus,
    pub registered_at: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub username: Option<String>,
}

/// Request to unregister a number.
#[derive(Debug, Deserialize)]
pub struct UnregisterRequest {
    /// Ownership secret (must match what was provided during registration)
    pub ownership_secret: Option<String>,
}

/// Health check response.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub registry_count: usize,
    pub signal_api_healthy: bool,
}

/// Request to update profile.
#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    /// Display name
    pub name: Option<String>,

    /// About/status text
    pub about: Option<String>,

    /// Ownership secret (must match what was provided during registration)
    pub ownership_secret: Option<String>,
}

/// Response after updating profile.
#[derive(Debug, Serialize)]
pub struct ProfileResponse {
    pub phone_number: String,
    pub message: String,
}

/// Request to set username.
#[derive(Debug, Deserialize)]
pub struct SetUsernameRequest {
    /// Desired username (without discriminator)
    pub username: String,

    /// Ownership secret (must match what was provided during registration)
    pub ownership_secret: Option<String>,
}

/// Response after setting username.
#[derive(Debug, Serialize)]
pub struct UsernameResponse {
    pub phone_number: String,
    pub username: Option<String>,
    pub username_link: Option<String>,
    pub message: String,
}

/// Request to delete username.
#[derive(Debug, Deserialize)]
pub struct DeleteUsernameRequest {
    /// Ownership secret (must match what was provided during registration)
    pub ownership_secret: Option<String>,
}

/// Request to adopt an existing Signal CLI account into the registry.
#[derive(Debug, Deserialize)]
pub struct AdoptAccountRequest {
    /// Optional ownership proof secret (will be hashed and stored)
    /// Required for later unregistration or configuration changes
    pub ownership_secret: Option<String>,

    /// AI model to use for this bot
    pub model: Option<String>,

    /// System prompt for the AI assistant
    pub system_prompt: Option<String>,
}

/// Request to update bot configuration.
#[derive(Debug, Deserialize)]
pub struct UpdateBotConfigRequest {
    /// AI model to use for this bot
    pub model: Option<String>,

    /// System prompt for the AI assistant
    pub system_prompt: Option<String>,

    /// Ownership secret (must match what was provided during registration)
    pub ownership_secret: Option<String>,
}

/// Response after updating bot config.
#[derive(Debug, Serialize)]
pub struct BotConfigResponse {
    pub phone_number: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub message: String,
}

/// Bot info for public listing.
#[derive(Debug, Serialize)]
pub struct BotInfo {
    /// Signal username (if set)
    pub username: String,
    /// Phone number in E.164 format
    pub phone_number: String,
    /// Signal.me link
    pub signal_link: String,
    /// When registered
    pub registered_at: String,
    /// AI model
    pub model: Option<String>,
    /// Bot description (derived from system prompt)
    pub description: Option<String>,
    /// Full system prompt
    pub system_prompt: Option<String>,
    /// Signal identity key (safety number) for verification
    pub identity_key: Option<String>,
}

/// List of bots response.
#[derive(Debug, Serialize)]
pub struct BotsResponse {
    pub bots: Vec<BotInfo>,
    pub total: usize,
}

/// Request to update bot config via JWT (no ownership_secret needed).
#[derive(Debug, Deserialize)]
pub struct UpdateBotConfigJwtRequest {
    /// AI model to use for this bot
    pub model: Option<String>,
    /// System prompt for the AI assistant
    pub system_prompt: Option<String>,
}

/// Webhook alert request — accepts alerts from external monitoring systems.
#[derive(Debug, Deserialize)]
pub struct WebhookAlertRequest {
    /// Alert title/summary.
    pub title: String,
    /// Alert body/description (optional).
    pub body: Option<String>,
    /// Severity level: "critical", "warning", "info", "ok" (optional, defaults to "info").
    pub severity: Option<String>,
    /// Source system (e.g., "Grafana", "Prometheus", "UptimeRobot").
    pub source: Option<String>,
    /// URL for more details (optional).
    pub url: Option<String>,
}

/// Webhook alert response.
#[derive(Debug, Serialize)]
pub struct WebhookAlertResponse {
    pub status: String,
    pub message: String,
}

/// Grafana webhook payload (simplified).
#[derive(Debug, Deserialize)]
pub struct GrafanaWebhookPayload {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(rename = "ruleUrl")]
    #[serde(default)]
    pub rule_url: Option<String>,
    #[serde(rename = "ruleName")]
    #[serde(default)]
    pub rule_name: Option<String>,
}

/// Login request.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// Phone number in E.164 format.
    pub phone_number: String,
    /// Ownership secret.
    pub ownership_secret: String,
}

/// Login response with JWT.
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub phone_number: String,
    pub expires_at: i64,
}

/// Dashboard response with combined bot status and usage.
#[derive(Debug, Serialize)]
pub struct DashboardResponse {
    pub phone_number: String,
    pub status: RegistrationStatus,
    pub registered_at: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub username: Option<String>,
    pub signal_link: Option<String>,
}
