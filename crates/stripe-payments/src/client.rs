//! Stripe API client for checkout sessions, customer portal, and webhook verification.
//!
//! Uses raw HTTP calls to Stripe API rather than a full SDK crate,
//! keeping dependencies minimal and avoiding version churn.

use crate::config::StripeConfig;
use crate::error::StripeError;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::Deserialize;
use sha2::Sha256;
use tracing::{debug, warn};

type HmacSha256 = Hmac<Sha256>;

const STRIPE_API_BASE: &str = "https://api.stripe.com/v1";

/// Stripe API client.
#[derive(Clone)]
pub struct StripeClient {
    http: Client,
    secret_key: String,
    webhook_secret: String,
}

/// Response from creating a Checkout Session.
#[derive(Debug, Deserialize)]
pub struct CheckoutSession {
    pub id: String,
    pub url: Option<String>,
}

/// Response from creating a Customer Portal session.
#[derive(Debug, Deserialize)]
pub struct PortalSession {
    pub id: String,
    pub url: String,
}

/// Stripe webhook event.
#[derive(Debug, Deserialize)]
pub struct WebhookEvent {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub data: EventData,
}

#[derive(Debug, Deserialize)]
pub struct EventData {
    pub object: serde_json::Value,
}

/// Parsed subscription data from webhook events.
#[derive(Debug, Clone)]
pub struct SubscriptionEvent {
    pub customer_id: String,
    pub subscription_id: String,
    pub status: String,
    pub price_id: Option<String>,
    pub current_period_end: Option<i64>,
}

/// Parsed checkout session data.
#[derive(Debug, Clone)]
pub struct CheckoutCompleted {
    pub customer_id: String,
    pub subscription_id: Option<String>,
    /// The client_reference_id we set (user's phone number).
    pub client_reference_id: Option<String>,
}

impl StripeClient {
    /// Create a new Stripe client.
    pub fn new(config: &StripeConfig) -> Self {
        Self {
            http: Client::new(),
            secret_key: config.secret_key.clone(),
            webhook_secret: config.webhook_secret.clone(),
        }
    }

    /// Create a Checkout Session for a subscription.
    pub async fn create_checkout_session(
        &self,
        price_id: &str,
        user_id: &str,
        success_url: &str,
        cancel_url: &str,
        customer_id: Option<&str>,
    ) -> Result<CheckoutSession, StripeError> {
        let mut params = vec![
            ("mode", "subscription".to_string()),
            ("line_items[0][price]", price_id.to_string()),
            ("line_items[0][quantity]", "1".to_string()),
            ("success_url", success_url.to_string()),
            ("cancel_url", cancel_url.to_string()),
            ("client_reference_id", user_id.to_string()),
        ];

        if let Some(cid) = customer_id {
            params.push(("customer", cid.to_string()));
        }

        let response = self
            .http
            .post(format!("{}/checkout/sessions", STRIPE_API_BASE))
            .basic_auth(&self.secret_key, None::<&str>)
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            warn!("Stripe checkout session creation failed: {}", body);
            return Err(StripeError::StripeApi(format!(
                "Failed to create checkout session: {}",
                body
            )));
        }

        let session: CheckoutSession = response.json().await?;
        debug!("Created checkout session: {}", session.id);
        Ok(session)
    }

    /// Create a Customer Portal session for self-service management.
    pub async fn create_portal_session(
        &self,
        customer_id: &str,
        return_url: &str,
    ) -> Result<PortalSession, StripeError> {
        let params = [
            ("customer", customer_id),
            ("return_url", return_url),
        ];

        let response = self
            .http
            .post(format!("{}/billing_portal/sessions", STRIPE_API_BASE))
            .basic_auth(&self.secret_key, None::<&str>)
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(StripeError::StripeApi(format!(
                "Failed to create portal session: {}",
                body
            )));
        }

        let session: PortalSession = response.json().await?;
        debug!("Created portal session for customer {}", customer_id);
        Ok(session)
    }

    /// Verify a webhook signature and parse the event.
    pub fn verify_webhook(
        &self,
        payload: &[u8],
        signature_header: &str,
    ) -> Result<WebhookEvent, StripeError> {
        // Parse the Stripe-Signature header
        let mut timestamp = None;
        let mut signatures = Vec::new();

        for part in signature_header.split(',') {
            let part = part.trim();
            if let Some(t) = part.strip_prefix("t=") {
                timestamp = Some(t.to_string());
            } else if let Some(v1) = part.strip_prefix("v1=") {
                signatures.push(v1.to_string());
            }
        }

        let timestamp = timestamp.ok_or(StripeError::InvalidSignature)?;
        if signatures.is_empty() {
            return Err(StripeError::InvalidSignature);
        }

        // Compute expected signature
        let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(payload));
        let mut mac = HmacSha256::new_from_slice(self.webhook_secret.as_bytes())
            .map_err(|_| StripeError::InvalidSignature)?;
        mac.update(signed_payload.as_bytes());
        let expected = hex::encode(mac.finalize().into_bytes());

        // Check if any signature matches
        if !signatures.iter().any(|sig| sig == &expected) {
            return Err(StripeError::InvalidSignature);
        }

        // Parse the event
        let event: WebhookEvent = serde_json::from_slice(payload)?;
        debug!("Verified webhook event: {} ({})", event.id, event.event_type);
        Ok(event)
    }

    /// Parse a checkout.session.completed event.
    pub fn parse_checkout_completed(event: &WebhookEvent) -> Option<CheckoutCompleted> {
        let obj = &event.data.object;
        Some(CheckoutCompleted {
            customer_id: obj.get("customer")?.as_str()?.to_string(),
            subscription_id: obj
                .get("subscription")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            client_reference_id: obj
                .get("client_reference_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        })
    }

    /// Parse a subscription event (updated, deleted).
    pub fn parse_subscription_event(event: &WebhookEvent) -> Option<SubscriptionEvent> {
        let obj = &event.data.object;
        Some(SubscriptionEvent {
            customer_id: obj.get("customer")?.as_str()?.to_string(),
            subscription_id: obj.get("id")?.as_str()?.to_string(),
            status: obj.get("status")?.as_str()?.to_string(),
            price_id: obj
                .get("items")
                .and_then(|i| i.get("data"))
                .and_then(|d| d.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("price"))
                .and_then(|p| p.get("id"))
                .and_then(|id| id.as_str())
                .map(|s| s.to_string()),
            current_period_end: obj
                .get("current_period_end")
                .and_then(|v| v.as_i64()),
        })
    }
}
