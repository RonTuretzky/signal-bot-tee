//! Error types for the registration proxy.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;

/// Proxy error types.
#[derive(Debug, Error)]
pub enum ProxyError {
    #[error("Phone number already registered: {0}")]
    AlreadyRegistered(String),

    #[error("Phone number not found: {0}")]
    NotFound(String),

    #[error("Invalid phone number format: {0}")]
    InvalidPhoneNumber(String),

    #[error("Ownership proof mismatch")]
    OwnershipProofMismatch,

    #[error("Registration pending verification")]
    PendingVerification,

    #[error("Signal API error: {0}")]
    SignalApi(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("TEE not available: {0}")]
    TeeNotAvailable(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Error response body.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            ProxyError::AlreadyRegistered(_) => (StatusCode::CONFLICT, "ALREADY_REGISTERED"),
            ProxyError::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            ProxyError::InvalidPhoneNumber(_) => (StatusCode::BAD_REQUEST, "INVALID_PHONE_NUMBER"),
            ProxyError::OwnershipProofMismatch => (StatusCode::FORBIDDEN, "OWNERSHIP_MISMATCH"),
            ProxyError::PendingVerification => (StatusCode::CONFLICT, "PENDING_VERIFICATION"),
            ProxyError::SignalApi(_) => (StatusCode::BAD_GATEWAY, "SIGNAL_API_ERROR"),
            ProxyError::Storage(_) => (StatusCode::INTERNAL_SERVER_ERROR, "STORAGE_ERROR"),
            ProxyError::Encryption(_) => (StatusCode::INTERNAL_SERVER_ERROR, "ENCRYPTION_ERROR"),
            ProxyError::TeeNotAvailable(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, "TEE_NOT_AVAILABLE")
            }
            ProxyError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
            ProxyError::RateLimitExceeded => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMIT_EXCEEDED"),
            ProxyError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
        };

        let body = ErrorResponse {
            error: self.to_string(),
            code: code.to_string(),
        };

        (status, Json(body)).into_response()
    }
}

impl From<std::io::Error> for ProxyError {
    fn from(e: std::io::Error) -> Self {
        ProxyError::Storage(e.to_string())
    }
}

impl From<serde_json::Error> for ProxyError {
    fn from(e: serde_json::Error) -> Self {
        ProxyError::Storage(format!("JSON serialization error: {}", e))
    }
}

impl From<aes_gcm::Error> for ProxyError {
    fn from(_: aes_gcm::Error) -> Self {
        ProxyError::Encryption("AES-GCM encryption/decryption failed".to_string())
    }
}

impl From<dstack_client::DstackError> for ProxyError {
    fn from(e: dstack_client::DstackError) -> Self {
        ProxyError::TeeNotAvailable(e.to_string())
    }
}

impl From<reqwest::Error> for ProxyError {
    fn from(e: reqwest::Error) -> Self {
        ProxyError::SignalApi(e.to_string())
    }
}
