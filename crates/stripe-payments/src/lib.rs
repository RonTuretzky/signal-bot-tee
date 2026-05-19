//! Stripe Subscription Billing for Signal Bot TEE
//!
//! Provides Stripe-based subscription management with TEE-encrypted storage.
//! Users get a free tier (limited daily messages) and can upgrade via Stripe Checkout.
//!
//! # Architecture
//!
//! ```text
//! User sends !subscribe → Bot creates Checkout URL → User pays in browser
//! Stripe webhook → Update subscription store → User gets unlimited messages
//! ```
//!
//! # Modules
//!
//! - [`config`] - Stripe configuration (keys, price IDs, limits)
//! - [`types`] - Subscription status, daily usage types
//! - [`store`] - TEE-encrypted persistent subscription store
//! - [`client`] - Stripe API client (checkout, portal, webhook verification)
//! - [`webhooks`] - Axum webhook handler
//! - [`error`] - Error types

pub mod client;
pub mod config;
pub mod error;
pub mod store;
pub mod types;
pub mod webhooks;

// Re-exports
pub use config::StripeConfig;
pub use error::StripeError;
pub use store::SubscriptionStore;
pub use types::{DailyUsage, SubscriptionStatus, UserSubscription};

use client::StripeClient;
use dstack_client::DstackClient;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;
use webhooks::WebhookState;

/// Start the Stripe payment/webhook HTTP server.
pub async fn spawn_stripe_server(
    config: StripeConfig,
    dstack: DstackClient,
) -> Result<Option<(tokio::task::JoinHandle<()>, Arc<SubscriptionStore>)>, StripeError> {
    if !config.enabled {
        info!("Stripe payments disabled");
        return Ok(None);
    }

    let store = SubscriptionStore::new(dstack, config.storage_path.clone()).await?;
    let stripe_client = StripeClient::new(&config);

    let webhook_state = Arc::new(WebhookState {
        stripe_client: stripe_client.clone(),
        store: store.clone(),
        config: config.clone(),
    });

    let router = axum::Router::new()
        .route(
            "/v1/stripe/webhook",
            axum::routing::post(webhooks::handle_webhook),
        )
        .route("/health", axum::routing::get(health_handler))
        .with_state(webhook_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.server_port));
    let listener = TcpListener::bind(addr).await.map_err(|e| {
        StripeError::Internal(format!("Failed to bind to {}: {}", addr, e))
    })?;

    info!("Stripe webhook server listening on {}", addr);

    let handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, router).await {
            tracing::error!("Stripe server error: {}", e);
        }
    });

    Ok(Some((handle, store)))
}

async fn health_handler() -> &'static str {
    "ok"
}
