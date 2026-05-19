//! JWT-based authentication for the registration proxy.
//!
//! Users authenticate by providing their phone number and ownership secret.
//! The server returns a JWT signed with a TEE-derived key.
//! Protected endpoints accept the JWT via Authorization: Bearer header.

use crate::error::ProxyError;
use crate::registry::Registry;
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tokio::sync::RwLock;
use std::sync::Arc;

type HmacSha256 = Hmac<Sha256>;

/// JWT claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject: phone number.
    pub sub: String,
    /// Expiration time (unix timestamp).
    pub exp: i64,
    /// Issued at (unix timestamp).
    pub iat: i64,
}

/// JWT token manager.
pub struct TokenManager {
    /// HMAC signing key (derived from TEE or fallback).
    signing_key: [u8; 32],
    /// Token lifetime.
    token_lifetime: Duration,
}

impl TokenManager {
    /// Create a new token manager with a signing key.
    pub fn new(signing_key: [u8; 32]) -> Self {
        Self {
            signing_key,
            token_lifetime: Duration::hours(24),
        }
    }

    /// Create a token for a user.
    pub fn create_token(&self, phone_number: &str) -> Result<String, ProxyError> {
        let now = Utc::now();
        let claims = Claims {
            sub: phone_number.to_string(),
            exp: (now + self.token_lifetime).timestamp(),
            iat: now.timestamp(),
        };

        let header = base64url_encode(b"{\"alg\":\"HS256\",\"typ\":\"JWT\"}");
        let payload = base64url_encode(
            &serde_json::to_vec(&claims)
                .map_err(|e| ProxyError::Internal(format!("Failed to serialize claims: {}", e)))?,
        );

        let signing_input = format!("{}.{}", header, payload);
        let signature = self.sign(signing_input.as_bytes())?;
        let signature_b64 = base64url_encode(&signature);

        Ok(format!("{}.{}.{}", header, payload, signature_b64))
    }

    /// Verify a token and return the claims.
    pub fn verify_token(&self, token: &str) -> Result<Claims, ProxyError> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(ProxyError::Unauthorized("Invalid token format".to_string()));
        }

        // Verify signature
        let signing_input = format!("{}.{}", parts[0], parts[1]);
        let signature = base64url_decode(parts[2])
            .map_err(|_| ProxyError::Unauthorized("Invalid token signature encoding".to_string()))?;
        let expected = self.sign(signing_input.as_bytes())?;

        if signature != expected {
            return Err(ProxyError::Unauthorized("Invalid token signature".to_string()));
        }

        // Decode claims
        let payload_bytes = base64url_decode(parts[1])
            .map_err(|_| ProxyError::Unauthorized("Invalid token payload encoding".to_string()))?;
        let claims: Claims = serde_json::from_slice(&payload_bytes)
            .map_err(|_| ProxyError::Unauthorized("Invalid token claims".to_string()))?;

        // Check expiration
        let now = Utc::now().timestamp();
        if claims.exp < now {
            return Err(ProxyError::Unauthorized("Token expired".to_string()));
        }

        Ok(claims)
    }

    fn sign(&self, data: &[u8]) -> Result<Vec<u8>, ProxyError> {
        let mut mac = HmacSha256::new_from_slice(&self.signing_key)
            .map_err(|_| ProxyError::Internal("HMAC key error".to_string()))?;
        mac.update(data);
        Ok(mac.finalize().into_bytes().to_vec())
    }
}

/// Authenticate a user and return a JWT.
pub async fn authenticate(
    phone_number: &str,
    ownership_secret: &str,
    registry: &Arc<RwLock<Registry>>,
    token_manager: &TokenManager,
) -> Result<(String, i64), ProxyError> {
    let reg = registry.read().await;
    let record = reg
        .get(phone_number)
        .ok_or_else(|| ProxyError::NotFound(phone_number.to_string()))?;

    // Verify ownership secret
    if !record.verify_ownership(Some(ownership_secret)) {
        return Err(ProxyError::OwnershipProofMismatch);
    }

    let token = token_manager.create_token(phone_number)?;
    let expires_at = (Utc::now() + chrono::Duration::hours(24)).timestamp();

    Ok((token, expires_at))
}

/// Extract and verify a JWT from an Authorization header value.
pub fn extract_bearer_token(auth_header: &str, token_manager: &TokenManager) -> Result<Claims, ProxyError> {
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| ProxyError::Unauthorized("Invalid Authorization header".to_string()))?;

    token_manager.verify_token(token)
}

// Base64url encoding/decoding (no padding, URL-safe)
fn base64url_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

fn base64url_decode(data: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        [0x42u8; 32]
    }

    #[test]
    fn test_create_and_verify_token() {
        let tm = TokenManager::new(test_key());
        let token = tm.create_token("+14155551234").unwrap();

        let claims = tm.verify_token(&token).unwrap();
        assert_eq!(claims.sub, "+14155551234");
        assert!(claims.exp > Utc::now().timestamp());
    }

    #[test]
    fn test_invalid_token() {
        let tm = TokenManager::new(test_key());
        let result = tm.verify_token("invalid.token.here");
        assert!(result.is_err());
    }

    #[test]
    fn test_tampered_token() {
        let tm = TokenManager::new(test_key());
        let token = tm.create_token("+14155551234").unwrap();

        // Tamper with the token
        let tampered = format!("{}x", token);
        let result = tm.verify_token(&tampered);
        assert!(result.is_err());
    }

    #[test]
    fn test_different_key_rejects() {
        let tm1 = TokenManager::new([0x42u8; 32]);
        let tm2 = TokenManager::new([0x43u8; 32]);

        let token = tm1.create_token("+14155551234").unwrap();
        let result = tm2.verify_token(&token);
        assert!(result.is_err());
    }
}
