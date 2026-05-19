//! Signal Registration Proxy - Entry point.

use dstack_client::DstackClient;
use signal_registration_proxy::{
    api::{create_router_with_rate_limit, AppState, RateLimitState},
    config::Config,
    registry::Store,
    signal::SignalRegistrationClient,
};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() {
    // Load configuration
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize logging
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.log.level));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Signal Registration Proxy");

    // Initialize Dstack client for TEE operations
    let dstack = DstackClient::new(&config.dstack.socket_path);

    // Initialize storage
    let store = if config.registry.persist {
        Store::new(dstack, config.registry.path.clone()).await
    } else {
        info!("Persistence disabled, using in-memory storage");
        Store::memory()
    };

    // Load existing registry
    let registry = match store.load().await {
        Ok(r) => {
            info!("Loaded registry with {} records", r.count());
            r
        }
        Err(e) => {
            error!("Failed to load registry: {}", e);
            info!("Starting with empty registry");
            signal_registration_proxy::Registry::new()
        }
    };

    // Initialize Signal client
    let signal_client = match SignalRegistrationClient::new(&config.signal.api_url) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create Signal client: {}", e);
            std::process::exit(1);
        }
    };

    // Derive JWT signing key from TEE (or use fallback for local dev)
    let auth_dstack = DstackClient::new(&config.dstack.socket_path);
    let signing_key = match auth_dstack
        .derive_key("signal-registration-proxy/jwt-signing", None)
        .await
    {
        Ok(key_bytes) if key_bytes.len() >= 32 => {
            let mut key = [0u8; 32];
            key.copy_from_slice(&key_bytes[..32]);
            info!("Using TEE-derived JWT signing key");
            key
        }
        _ => {
            // Fallback: derive from a fixed seed (for local dev without TEE)
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(b"signal-registration-proxy/jwt-signing/dev-fallback");
            let hash = hasher.finalize();
            let mut key = [0u8; 32];
            key.copy_from_slice(&hash);
            info!("Using fallback JWT signing key (no TEE available)");
            key
        }
    };
    let token_manager = signal_registration_proxy::auth::TokenManager::new(signing_key);

    // Create application state
    let state = AppState::new(registry, store, signal_client, token_manager);

    // Create rate limiter from config
    let rate_limit = RateLimitState::new(config.rate_limit.global_per_minute);

    // Create router with rate limiting
    let app = create_router_with_rate_limit(state, rate_limit);

    // Bind to address
    let addr = SocketAddr::new(
        config.server.listen_addr.parse().unwrap_or([0, 0, 0, 0].into()),
        config.server.port,
    );

    info!("Listening on {}", addr);

    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to bind to {}: {}", addr, e);
            std::process::exit(1);
        }
    };

    // Run server
    if let Err(e) = axum::serve(listener, app).await {
        error!("Server error: {}", e);
        std::process::exit(1);
    }
}
