//! Rate limiting and other middleware.

use crate::error::ProxyError;
use axum::{
    extract::{ConnectInfo, Request, State},
    middleware::Next,
    response::Response,
};
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use std::{collections::HashMap, net::SocketAddr, num::NonZeroU32, sync::Arc, time::Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Global rate limiter (not keyed by IP).
pub type GlobalLimiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

/// Simple per-IP rate tracking.
struct IpBucket {
    count: u32,
    window_start: Instant,
}

/// Rate limiter state shared across requests.
#[derive(Clone)]
pub struct RateLimitState {
    /// Global rate limiter for all requests
    pub global: Arc<GlobalLimiter>,
    /// Per-IP rate tracking
    per_ip: Arc<RwLock<HashMap<String, IpBucket>>>,
    /// Max requests per IP per minute
    per_ip_limit: u32,
}

impl RateLimitState {
    /// Create a new rate limit state with the specified limits.
    pub fn new(requests_per_minute: u32) -> Self {
        let quota = Quota::per_minute(
            NonZeroU32::new(requests_per_minute).unwrap_or(NonZeroU32::new(10).unwrap()),
        );

        Self {
            global: Arc::new(RateLimiter::direct(quota)),
            per_ip: Arc::new(RwLock::new(HashMap::new())),
            per_ip_limit: requests_per_minute.max(5), // At least 5 per IP per minute
        }
    }

    /// Create a permissive rate limiter for testing.
    pub fn permissive() -> Self {
        let state = Self::new(1000);
        Self {
            per_ip_limit: 1000,
            ..state
        }
    }

    /// Check per-IP rate limit. Returns true if allowed.
    pub async fn check_ip(&self, ip: &str) -> bool {
        let now = Instant::now();
        let window = std::time::Duration::from_secs(60);

        let mut map = self.per_ip.write().await;
        let bucket = map.entry(ip.to_string()).or_insert(IpBucket {
            count: 0,
            window_start: now,
        });

        // Reset window if expired
        if now.duration_since(bucket.window_start) >= window {
            bucket.count = 0;
            bucket.window_start = now;
        }

        bucket.count += 1;
        bucket.count <= self.per_ip_limit
    }
}

/// Rate limiting middleware.
///
/// Checks both global and per-IP rate limits.
pub async fn rate_limit_middleware(
    State(rate_limit): State<RateLimitState>,
    request: Request,
    next: Next,
) -> Result<Response, ProxyError> {
    // Check global rate limit
    if rate_limit.global.check().is_err() {
        warn!("Global rate limit exceeded");
        return Err(ProxyError::RateLimitExceeded);
    }

    // Check per-IP rate limit (extract from X-Forwarded-For or connection info)
    let ip = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            request
                .extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|ci| ci.0.ip().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());

    if !rate_limit.check_ip(&ip).await {
        warn!(ip = %ip, "Per-IP rate limit exceeded");
        return Err(ProxyError::RateLimitExceeded);
    }

    debug!("Rate limit check passed");
    Ok(next.run(request).await)
}

/// Logging middleware for requests.
pub async fn logging_middleware(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = std::time::Instant::now();

    debug!(%method, %uri, "Request started");

    let response = next.run(request).await;

    let duration = start.elapsed();
    let status = response.status();

    if status.is_success() {
        debug!(%method, %uri, %status, ?duration, "Request completed");
    } else {
        warn!(%method, %uri, %status, ?duration, "Request failed");
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_state_creation() {
        let state = RateLimitState::new(10);
        // Should allow first request
        assert!(state.global.check().is_ok());
    }

    #[test]
    fn test_rate_limit_exhaustion() {
        // Very low limit for testing
        let state = RateLimitState::new(1);

        // First request should succeed
        assert!(state.global.check().is_ok());

        // Second request should fail (exceeded 1 per minute)
        assert!(state.global.check().is_err());
    }

    #[test]
    fn test_permissive_rate_limit() {
        let state = RateLimitState::permissive();
        // Should allow many requests
        for _ in 0..100 {
            assert!(state.global.check().is_ok());
        }
    }

    #[tokio::test]
    async fn test_per_ip_rate_limit() {
        let state = RateLimitState::new(5);
        // First 5 requests should pass
        for _ in 0..5 {
            assert!(state.check_ip("1.2.3.4").await);
        }
        // 6th should fail
        assert!(!state.check_ip("1.2.3.4").await);
        // Different IP should still pass
        assert!(state.check_ip("5.6.7.8").await);
    }
}
