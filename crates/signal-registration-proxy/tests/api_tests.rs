//! Integration tests for the registration proxy API.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use signal_registration_proxy::{
    api::{create_router_with_rate_limit, AppState, RateLimitState},
    auth::TokenManager,
    registry::{Registry, Store},
    SignalRegistrationClient,
};
use tower::ServiceExt;

/// Create a test app state with memory-only storage.
fn create_test_state() -> AppState {
    let registry = Registry::new();
    let store = Store::memory();
    // Use a non-existent URL since we won't actually call Signal in tests
    let signal_client = SignalRegistrationClient::new("http://localhost:9999").unwrap();
    let token_manager = TokenManager::new([0x42u8; 32]);
    AppState::new(registry, store, signal_client, token_manager)
}

#[tokio::test]
async fn test_health_endpoint() {
    let state = create_test_state();
    let app = create_router_with_rate_limit(state, RateLimitState::permissive());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "ok");
    assert_eq!(json["registry_count"], 0);
}

#[tokio::test]
async fn test_status_not_found() {
    let state = create_test_state();
    let app = create_router_with_rate_limit(state, RateLimitState::permissive());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/status/+14155551234")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_list_accounts_empty() {
    let state = create_test_state();
    let app = create_router_with_rate_limit(state, RateLimitState::permissive());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/accounts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["total"], 0);
    assert!(json["accounts"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_invalid_phone_number() {
    let state = create_test_state();
    let app = create_router_with_rate_limit(state, RateLimitState::permissive());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/status/invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should fail with bad request due to invalid phone number format
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_rate_limiting() {
    let state = create_test_state();
    // Very restrictive rate limit: 1 request per minute
    let rate_limit = RateLimitState::new(1);
    let app = create_router_with_rate_limit(state, rate_limit);

    // First request should succeed
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/accounts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Second request should be rate limited
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/accounts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}
