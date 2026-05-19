//! Stripe webhook handler.

use crate::client::{StripeClient, WebhookEvent};
use crate::config::StripeConfig;
use crate::store::SubscriptionStore;
use crate::types::SubscriptionStatus;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tracing::{error, info, warn};

/// Shared state for webhook handler.
pub struct WebhookState {
    pub stripe_client: StripeClient,
    pub store: Arc<SubscriptionStore>,
    pub config: StripeConfig,
}

/// Stripe webhook endpoint handler.
pub async fn handle_webhook(
    State(state): State<Arc<WebhookState>>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let signature = match headers.get("stripe-signature") {
        Some(sig) => match sig.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return StatusCode::BAD_REQUEST,
        },
        None => {
            warn!("Missing Stripe-Signature header");
            return StatusCode::BAD_REQUEST;
        }
    };

    let event = match state.stripe_client.verify_webhook(&body, &signature) {
        Ok(e) => e,
        Err(e) => {
            warn!("Webhook verification failed: {}", e);
            return StatusCode::BAD_REQUEST;
        }
    };

    info!("Received webhook event: {} ({})", event.id, event.event_type);

    match event.event_type.as_str() {
        "checkout.session.completed" => handle_checkout_completed(&state, &event).await,
        "customer.subscription.updated" => handle_subscription_updated(&state, &event).await,
        "customer.subscription.deleted" => handle_subscription_deleted(&state, &event).await,
        "invoice.payment_failed" => handle_payment_failed(&state, &event).await,
        _ => {
            // Acknowledge unknown events (Stripe expects 200)
            StatusCode::OK
        }
    }
}

async fn handle_checkout_completed(state: &WebhookState, event: &WebhookEvent) -> StatusCode {
    let checkout = match StripeClient::parse_checkout_completed(event) {
        Some(c) => c,
        None => {
            error!("Failed to parse checkout.session.completed");
            return StatusCode::OK; // Still return 200 to avoid retries
        }
    };

    let user_id = match checkout.client_reference_id {
        Some(ref id) => id.clone(),
        None => {
            // Fall back to customer_id lookup
            match state.store.get_user_by_customer_id(&checkout.customer_id).await {
                Some(id) => id,
                None => {
                    error!("Cannot determine user for checkout: {:?}", checkout);
                    return StatusCode::OK;
                }
            }
        }
    };

    // Determine plan name from price
    let plan_name = determine_plan_name(&state.config, None);

    let mut subscription = state.store.get_subscription(&user_id).await;
    subscription.status = SubscriptionStatus::Active;
    subscription.plan_name = plan_name;
    subscription.stripe_customer_id = Some(checkout.customer_id);
    subscription.subscription_id = checkout.subscription_id;
    subscription.updated_at = Utc::now();

    if let Err(e) = state.store.set_subscription(subscription).await {
        error!("Failed to save subscription for {}: {}", user_id, e);
    } else {
        info!("Activated subscription for {}", user_id);
    }

    StatusCode::OK
}

async fn handle_subscription_updated(state: &WebhookState, event: &WebhookEvent) -> StatusCode {
    let sub_event = match StripeClient::parse_subscription_event(event) {
        Some(s) => s,
        None => {
            error!("Failed to parse subscription updated event");
            return StatusCode::OK;
        }
    };

    let user_id = match state
        .store
        .get_user_by_customer_id(&sub_event.customer_id)
        .await
    {
        Some(id) => id,
        None => {
            warn!(
                "Unknown customer in subscription update: {}",
                sub_event.customer_id
            );
            return StatusCode::OK;
        }
    };

    let new_status = match sub_event.status.as_str() {
        "active" => SubscriptionStatus::Active,
        "past_due" => SubscriptionStatus::PastDue,
        "canceled" | "unpaid" => SubscriptionStatus::Canceled,
        "trialing" => SubscriptionStatus::Trialing,
        _ => {
            warn!("Unknown subscription status: {}", sub_event.status);
            return StatusCode::OK;
        }
    };

    let plan_name = determine_plan_name(&state.config, sub_event.price_id.as_deref());

    let mut subscription = state.store.get_subscription(&user_id).await;
    subscription.status = new_status;
    subscription.plan_name = plan_name;
    subscription.subscription_id = Some(sub_event.subscription_id);
    subscription.current_period_end = sub_event
        .current_period_end
        .and_then(|ts| DateTime::from_timestamp(ts, 0));
    subscription.updated_at = Utc::now();

    if let Err(e) = state.store.set_subscription(subscription).await {
        error!("Failed to update subscription for {}: {}", user_id, e);
    } else {
        info!("Updated subscription for {}: {}", user_id, sub_event.status);
    }

    StatusCode::OK
}

async fn handle_subscription_deleted(state: &WebhookState, event: &WebhookEvent) -> StatusCode {
    let sub_event = match StripeClient::parse_subscription_event(event) {
        Some(s) => s,
        None => return StatusCode::OK,
    };

    let user_id = match state
        .store
        .get_user_by_customer_id(&sub_event.customer_id)
        .await
    {
        Some(id) => id,
        None => return StatusCode::OK,
    };

    let mut subscription = state.store.get_subscription(&user_id).await;
    subscription.status = SubscriptionStatus::Canceled;
    subscription.updated_at = Utc::now();

    if let Err(e) = state.store.set_subscription(subscription).await {
        error!("Failed to cancel subscription for {}: {}", user_id, e);
    } else {
        info!("Canceled subscription for {}", user_id);
    }

    StatusCode::OK
}

async fn handle_payment_failed(state: &WebhookState, event: &WebhookEvent) -> StatusCode {
    // invoice.payment_failed — mark as past_due
    let customer_id = event
        .data
        .object
        .get("customer")
        .and_then(|v| v.as_str());

    if let Some(customer_id) = customer_id {
        if let Some(user_id) = state.store.get_user_by_customer_id(customer_id).await {
            let mut subscription = state.store.get_subscription(&user_id).await;
            subscription.status = SubscriptionStatus::PastDue;
            subscription.updated_at = Utc::now();

            if let Err(e) = state.store.set_subscription(subscription).await {
                error!("Failed to mark past_due for {}: {}", user_id, e);
            } else {
                info!("Marked subscription past_due for {}", user_id);
            }
        }
    }

    StatusCode::OK
}

/// Determine plan name from price ID.
fn determine_plan_name(config: &StripeConfig, price_id: Option<&str>) -> String {
    match price_id {
        Some(id) if config.pro_price_id.as_deref() == Some(id) => "Pro".to_string(),
        Some(id) if config.premium_price_id.as_deref() == Some(id) => "Premium".to_string(),
        Some(_) => "Premium".to_string(), // Default for unknown price IDs
        None => "Premium".to_string(),
    }
}
