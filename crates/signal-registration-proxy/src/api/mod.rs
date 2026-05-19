//! HTTP API for the registration proxy.

mod handlers;
mod middleware;
mod types;

pub use handlers::*;
pub use middleware::{logging_middleware, rate_limit_middleware, RateLimitState};
pub use types::*;

use crate::auth::TokenManager;
use crate::registry::{Registry, Store};
use crate::signal::SignalRegistrationClient;
use axum::{
    http::{header, Method},
    middleware as axum_middleware,
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    /// Phone number registry
    pub registry: Arc<RwLock<Registry>>,
    /// Persistent storage backend
    pub store: Arc<Store>,
    /// Signal CLI client
    pub signal_client: Arc<SignalRegistrationClient>,
    /// JWT token manager
    pub token_manager: Arc<TokenManager>,
}

impl AppState {
    /// Create new application state.
    pub fn new(
        registry: Registry,
        store: Store,
        signal_client: SignalRegistrationClient,
        token_manager: TokenManager,
    ) -> Self {
        Self {
            registry: Arc::new(RwLock::new(registry)),
            store: Arc::new(store),
            signal_client: Arc::new(signal_client),
            token_manager: Arc::new(token_manager),
        }
    }
}

/// Create the API router with rate limiting.
pub fn create_router(state: AppState) -> Router {
    create_router_with_rate_limit(state, RateLimitState::new(10))
}

/// Create the API router with custom rate limiting.
pub fn create_router_with_rate_limit(state: AppState, rate_limit: RateLimitState) -> Router {
    Router::new()
        // Health check (no rate limiting)
        .route("/health", get(handlers::health))
        // Auth endpoint
        .route("/v1/auth/login", post(handlers::login))
        // Registration endpoints (with rate limiting)
        .route("/v1/register/:number", post(handlers::register_number))
        .route(
            "/v1/register/:number/verify/:code",
            post(handlers::verify_registration),
        )
        .route("/v1/status/:number", get(handlers::get_status))
        .route("/v1/accounts", get(handlers::list_accounts))
        .route("/v1/unregister/:number", delete(handlers::unregister))
        // Profile and username management (requires ownership_secret or JWT)
        .route("/v1/profiles/:number", put(handlers::update_profile))
        .route("/v1/accounts/:number/username", post(handlers::set_username))
        .route("/v1/accounts/:number/username", delete(handlers::delete_username))
        // Adopt existing Signal CLI account into registry
        .route("/v1/accounts/:number/adopt", post(handlers::adopt_account))
        // Bot configuration management
        .route("/v1/bots", get(handlers::list_bots))
        .route("/v1/bots/:number", get(handlers::get_bot_config))
        .route("/v1/bots/:number", put(handlers::update_bot_config))
        // Dashboard endpoints (requires JWT)
        .route("/v1/dashboard/:number", get(handlers::get_dashboard))
        .route("/v1/dashboard/:number/config", put(handlers::update_bot_config_jwt))
        // Webhook alert endpoints (requires JWT)
        .route("/v1/webhooks/:number/alert", post(handlers::webhook_alert))
        .route("/v1/webhooks/:number/alert/:recipient", post(handlers::webhook_alert_to))
        .route("/v1/webhooks/:number/grafana", post(handlers::webhook_grafana))
        // Debug endpoints
        .route("/v1/debug/signal-accounts", get(handlers::debug_signal_accounts))
        .route("/v1/debug/force-unregister/:number", post(handlers::debug_force_unregister))
        .layer(axum_middleware::from_fn_with_state(
            rate_limit.clone(),
            rate_limit_middleware,
        ))
        .layer(axum_middleware::from_fn(logging_middleware))
        .layer(TraceLayer::new_for_http())
        // CORS layer to allow requests from web frontends
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
                .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        )
        .with_state(state)
}
