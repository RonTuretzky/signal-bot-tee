//! TEE-encrypted persistent subscription store.
//!
//! Follows the same pattern as x402-payments CreditStore:
//! AES-256-GCM encryption with Dstack-derived keys, atomic file writes.

use crate::error::StripeError;
use crate::types::{DailyUsage, SubscriptionStatus, UserSubscription};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use dstack_client::DstackClient;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Key derivation path for subscription store encryption.
const KEY_DERIVATION_PATH: &str = "stripe-payments/subscription-store";

/// Nonce size for AES-GCM (96 bits = 12 bytes).
const NONCE_SIZE: usize = 12;

/// Data version for schema migrations.
const DATA_VERSION: u32 = 1;

/// Persistent data structure for the subscription store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionStoreData {
    /// Schema version.
    pub version: u32,
    /// User subscriptions keyed by phone number.
    pub subscriptions: HashMap<String, UserSubscription>,
    /// Stripe customer_id -> phone number mapping (for webhook lookups).
    pub customer_id_map: HashMap<String, String>,
    /// Daily usage keyed by phone number.
    pub daily_usage: HashMap<String, DailyUsage>,
}

impl Default for SubscriptionStoreData {
    fn default() -> Self {
        Self {
            version: DATA_VERSION,
            subscriptions: HashMap::new(),
            customer_id_map: HashMap::new(),
            daily_usage: HashMap::new(),
        }
    }
}

/// TEE-encrypted subscription store.
pub struct SubscriptionStore {
    data: RwLock<SubscriptionStoreData>,
    dstack: DstackClient,
    storage_path: PathBuf,
    cached_key: RwLock<Option<[u8; 32]>>,
}

impl SubscriptionStore {
    /// Create a new subscription store and load existing data if available.
    pub async fn new(
        dstack: DstackClient,
        storage_path: PathBuf,
    ) -> Result<Arc<Self>, StripeError> {
        let store = Arc::new(Self {
            data: RwLock::new(SubscriptionStoreData::default()),
            dstack,
            storage_path,
            cached_key: RwLock::new(None),
        });

        store.load().await?;
        Ok(store)
    }

    /// Create a subscription store with a pre-derived key (for testing).
    pub async fn with_key(
        dstack: DstackClient,
        storage_path: PathBuf,
        key: [u8; 32],
    ) -> Result<Arc<Self>, StripeError> {
        let store = Arc::new(Self {
            data: RwLock::new(SubscriptionStoreData::default()),
            dstack,
            storage_path,
            cached_key: RwLock::new(Some(key)),
        });

        store.load().await?;
        Ok(store)
    }

    /// Derive encryption key from TEE root of trust.
    async fn derive_key(&self) -> Result<[u8; 32], StripeError> {
        {
            let cached = self.cached_key.read().await;
            if let Some(key) = *cached {
                return Ok(key);
            }
        }

        match self.dstack.derive_key(KEY_DERIVATION_PATH, None).await {
            Ok(key_bytes) => {
                if key_bytes.len() < 32 {
                    return Err(StripeError::Encryption(format!(
                        "Derived key too short: {} bytes",
                        key_bytes.len()
                    )));
                }
                let mut key = [0u8; 32];
                key.copy_from_slice(&key_bytes[..32]);
                *self.cached_key.write().await = Some(key);
                info!("Using DeriveKey endpoint for subscription store encryption");
                return Ok(key);
            }
            Err(e) => {
                warn!(
                    "DeriveKey not available, falling back to AppInfo: {}",
                    e
                );
            }
        }

        // Fallback to AppInfo-derived key
        let app_info = self.dstack.get_app_info().await.map_err(|e| {
            StripeError::Encryption(format!("Failed to get AppInfo: {}", e))
        })?;

        let compose_hash = app_info.compose_hash.as_deref().unwrap_or("unknown");
        let app_id = app_info.app_id.as_deref().unwrap_or("unknown");

        let mut hasher = Sha256::new();
        hasher.update(compose_hash.as_bytes());
        hasher.update(app_id.as_bytes());
        hasher.update(KEY_DERIVATION_PATH.as_bytes());
        let hash = hasher.finalize();

        let mut key = [0u8; 32];
        key.copy_from_slice(&hash);
        *self.cached_key.write().await = Some(key);

        info!(
            "Using AppInfo-derived key (compose_hash: {}, app_id: {})",
            compose_hash, app_id
        );

        Ok(key)
    }

    /// Save data to encrypted storage.
    pub async fn persist(&self) -> Result<(), StripeError> {
        let key = self.derive_key().await?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));

        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let data = self.data.read().await;
        let plaintext = serde_json::to_vec(&*data)?;

        let ciphertext = cipher.encrypt(nonce, plaintext.as_ref())?;

        let mut encrypted = nonce_bytes.to_vec();
        encrypted.extend(ciphertext);

        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let temp_path = self.storage_path.with_extension("tmp");
        fs::write(&temp_path, &encrypted).await?;
        fs::rename(&temp_path, &self.storage_path).await?;

        debug!(
            "Saved subscription store ({} bytes) to {:?}",
            encrypted.len(),
            self.storage_path
        );

        Ok(())
    }

    /// Load data from encrypted storage.
    async fn load(&self) -> Result<(), StripeError> {
        if !self.storage_path.exists() {
            info!(
                "Subscription store not found at {:?}, starting fresh",
                self.storage_path
            );
            return Ok(());
        }

        let key = self.derive_key().await?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));

        let encrypted = fs::read(&self.storage_path).await?;

        if encrypted.len() < NONCE_SIZE {
            warn!("Subscription store file too short, starting fresh");
            return Ok(());
        }

        let nonce = Nonce::from_slice(&encrypted[..NONCE_SIZE]);
        let ciphertext = &encrypted[NONCE_SIZE..];

        let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|_| {
            StripeError::Encryption(
                "Failed to decrypt subscription store. TEE deployment may have changed.".to_string(),
            )
        })?;

        let data: SubscriptionStoreData = serde_json::from_slice(&plaintext)?;

        info!(
            "Loaded subscription store: {} subscriptions",
            data.subscriptions.len()
        );

        *self.data.write().await = data;
        Ok(())
    }

    /// Get a user's subscription (returns free tier if not found).
    pub async fn get_subscription(&self, user_id: &str) -> UserSubscription {
        let data = self.data.read().await;
        data.subscriptions
            .get(user_id)
            .cloned()
            .unwrap_or_else(|| UserSubscription::new_free(user_id.to_string()))
    }

    /// Look up a user by their Stripe customer ID.
    pub async fn get_user_by_customer_id(&self, customer_id: &str) -> Option<String> {
        let data = self.data.read().await;
        data.customer_id_map.get(customer_id).cloned()
    }

    /// Set or update a user's subscription.
    pub async fn set_subscription(
        &self,
        subscription: UserSubscription,
    ) -> Result<(), StripeError> {
        {
            let mut data = self.data.write().await;
            if let Some(ref cid) = subscription.stripe_customer_id {
                data.customer_id_map
                    .insert(cid.clone(), subscription.user_id.clone());
            }
            data.subscriptions
                .insert(subscription.user_id.clone(), subscription);
        }
        self.persist().await
    }

    /// Get today's usage for a user, creating a new record if needed.
    pub async fn get_daily_usage(&self, user_id: &str) -> DailyUsage {
        let data = self.data.read().await;
        match data.daily_usage.get(user_id) {
            Some(usage) if usage.is_today() => usage.clone(),
            _ => DailyUsage::new_today(),
        }
    }

    /// Increment the daily message count for a user.
    pub async fn increment_usage(
        &self,
        user_id: &str,
        prompt_tokens: u32,
        completion_tokens: u32,
    ) -> Result<DailyUsage, StripeError> {
        let usage = {
            let mut data = self.data.write().await;
            let usage = data
                .daily_usage
                .entry(user_id.to_string())
                .or_insert_with(DailyUsage::new_today);

            // Reset if it's a new day
            if !usage.is_today() {
                *usage = DailyUsage::new_today();
            }

            usage.message_count += 1;
            usage.prompt_tokens += prompt_tokens;
            usage.completion_tokens += completion_tokens;
            usage.clone()
        };

        self.persist().await?;
        Ok(usage)
    }

    /// Check if a user can send a message based on their subscription and daily usage.
    pub async fn can_send_message(
        &self,
        user_id: &str,
        free_daily_limit: u32,
    ) -> Result<(), String> {
        let subscription = self.get_subscription(user_id).await;

        if !subscription.status.can_send() {
            return Err(format!(
                "Your subscription is {}. Please renew to continue using the bot.",
                match subscription.status {
                    SubscriptionStatus::PastDue => "past due",
                    SubscriptionStatus::Canceled => "canceled",
                    _ => "inactive",
                }
            ));
        }

        // Unlimited for paid subscribers
        if subscription.status.is_unlimited() {
            return Ok(());
        }

        // Free tier: check daily limit
        let usage = self.get_daily_usage(user_id).await;
        if usage.message_count >= free_daily_limit {
            return Err(format!(
                "You've used all {} free messages for today. Upgrade to Premium for unlimited messages!\n\n\
                 Send `!subscribe` to see plans.",
                free_daily_limit
            ));
        }

        Ok(())
    }

    /// Get summary statistics.
    pub async fn get_stats(&self) -> StoreStats {
        let data = self.data.read().await;
        let active_count = data
            .subscriptions
            .values()
            .filter(|s| s.status == SubscriptionStatus::Active)
            .count();
        let free_count = data
            .subscriptions
            .values()
            .filter(|s| s.status == SubscriptionStatus::Free)
            .count();

        StoreStats {
            total_users: data.subscriptions.len(),
            active_subscribers: active_count,
            free_users: free_count,
        }
    }
}

/// Summary statistics for the subscription store.
#[derive(Debug, Clone)]
pub struct StoreStats {
    pub total_users: usize,
    pub active_subscribers: usize,
    pub free_users: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_key() -> [u8; 32] {
        [0x42u8; 32]
    }

    async fn create_test_store() -> (Arc<SubscriptionStore>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("subscriptions.enc");
        let dstack = DstackClient::new("/var/run/dstack.sock");

        let store = SubscriptionStore::with_key(dstack, storage_path, create_test_key())
            .await
            .unwrap();

        (store, temp_dir)
    }

    #[tokio::test]
    async fn test_get_subscription_default() {
        let (store, _dir) = create_test_store().await;

        let sub = store.get_subscription("+14155551234").await;

        assert_eq!(sub.user_id, "+14155551234");
        assert_eq!(sub.status, SubscriptionStatus::Free);
        assert_eq!(sub.plan_name, "Free");
    }

    #[tokio::test]
    async fn test_set_and_get_subscription() {
        let (store, _dir) = create_test_store().await;

        let mut sub = UserSubscription::new_free("+14155551234".to_string());
        sub.status = SubscriptionStatus::Active;
        sub.plan_name = "Premium".to_string();
        sub.stripe_customer_id = Some("cus_test123".to_string());
        sub.subscription_id = Some("sub_test123".to_string());

        store.set_subscription(sub).await.unwrap();

        let loaded = store.get_subscription("+14155551234").await;
        assert_eq!(loaded.status, SubscriptionStatus::Active);
        assert_eq!(loaded.plan_name, "Premium");
    }

    #[tokio::test]
    async fn test_customer_id_lookup() {
        let (store, _dir) = create_test_store().await;

        let mut sub = UserSubscription::new_free("+14155551234".to_string());
        sub.stripe_customer_id = Some("cus_abc123".to_string());
        sub.status = SubscriptionStatus::Active;
        store.set_subscription(sub).await.unwrap();

        let user = store.get_user_by_customer_id("cus_abc123").await;
        assert_eq!(user, Some("+14155551234".to_string()));

        let missing = store.get_user_by_customer_id("cus_unknown").await;
        assert_eq!(missing, None);
    }

    #[tokio::test]
    async fn test_daily_usage_tracking() {
        let (store, _dir) = create_test_store().await;

        let usage = store.get_daily_usage("+14155551234").await;
        assert_eq!(usage.message_count, 0);

        store.increment_usage("+14155551234", 100, 50).await.unwrap();
        store.increment_usage("+14155551234", 200, 100).await.unwrap();

        let usage = store.get_daily_usage("+14155551234").await;
        assert_eq!(usage.message_count, 2);
        assert_eq!(usage.prompt_tokens, 300);
        assert_eq!(usage.completion_tokens, 150);
    }

    #[tokio::test]
    async fn test_free_tier_limit() {
        let (store, _dir) = create_test_store().await;
        let free_limit = 3;

        // First 3 messages should be allowed
        for _ in 0..3 {
            assert!(store.can_send_message("+14155551234", free_limit).await.is_ok());
            store.increment_usage("+14155551234", 100, 50).await.unwrap();
        }

        // 4th message should be blocked
        let result = store.can_send_message("+14155551234", free_limit).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("free messages"));
    }

    #[tokio::test]
    async fn test_paid_user_unlimited() {
        let (store, _dir) = create_test_store().await;

        let mut sub = UserSubscription::new_free("+14155551234".to_string());
        sub.status = SubscriptionStatus::Active;
        sub.plan_name = "Premium".to_string();
        store.set_subscription(sub).await.unwrap();

        // Paid users should always be allowed regardless of usage
        for _ in 0..100 {
            assert!(store.can_send_message("+14155551234", 3).await.is_ok());
            store.increment_usage("+14155551234", 100, 50).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_subscription_lifecycle() {
        let (store, _dir) = create_test_store().await;
        let user = "+14155551234";
        let free_limit = 15;

        // 1. User starts as Free
        let sub = store.get_subscription(user).await;
        assert_eq!(sub.status, SubscriptionStatus::Free);
        assert!(store.can_send_message(user, free_limit).await.is_ok());

        // 2. User subscribes (checkout completed webhook)
        let mut sub = store.get_subscription(user).await;
        sub.status = SubscriptionStatus::Active;
        sub.plan_name = "Premium".to_string();
        sub.stripe_customer_id = Some("cus_test".to_string());
        sub.subscription_id = Some("sub_test".to_string());
        store.set_subscription(sub).await.unwrap();

        // Verify customer lookup works
        assert_eq!(
            store.get_user_by_customer_id("cus_test").await,
            Some(user.to_string())
        );

        // 3. Payment fails (invoice.payment_failed webhook)
        let mut sub = store.get_subscription(user).await;
        sub.status = SubscriptionStatus::PastDue;
        store.set_subscription(sub).await.unwrap();

        // PastDue users cannot send
        assert!(store.can_send_message(user, free_limit).await.is_err());

        // 4. Payment recovered (subscription.updated webhook)
        let mut sub = store.get_subscription(user).await;
        sub.status = SubscriptionStatus::Active;
        store.set_subscription(sub).await.unwrap();
        assert!(store.can_send_message(user, free_limit).await.is_ok());

        // 5. User cancels (subscription.deleted webhook)
        let mut sub = store.get_subscription(user).await;
        sub.status = SubscriptionStatus::Canceled;
        store.set_subscription(sub).await.unwrap();
        assert!(store.can_send_message(user, free_limit).await.is_err());
    }

    #[tokio::test]
    async fn test_stats() {
        let (store, _dir) = create_test_store().await;

        // Add a free user (implicit via get)
        store.increment_usage("+11111111111", 10, 5).await.unwrap();

        // Add an active subscriber
        let mut sub = UserSubscription::new_free("+12222222222".to_string());
        sub.status = SubscriptionStatus::Active;
        sub.plan_name = "Premium".to_string();
        store.set_subscription(sub).await.unwrap();

        let stats = store.get_stats().await;
        assert_eq!(stats.total_users, 1); // only explicitly set subscriptions
        assert_eq!(stats.active_subscribers, 1);
    }

    #[tokio::test]
    async fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("subscriptions.enc");
        let key = create_test_key();

        // Create store and add data
        {
            let dstack = DstackClient::new("/var/run/dstack.sock");
            let store = SubscriptionStore::with_key(dstack, storage_path.clone(), key)
                .await
                .unwrap();

            let mut sub = UserSubscription::new_free("+14155551234".to_string());
            sub.status = SubscriptionStatus::Active;
            sub.plan_name = "Premium".to_string();
            store.set_subscription(sub).await.unwrap();
        }

        // Create new store instance and verify data loaded
        {
            let dstack = DstackClient::new("/var/run/dstack.sock");
            let store = SubscriptionStore::with_key(dstack, storage_path, key)
                .await
                .unwrap();

            let sub = store.get_subscription("+14155551234").await;
            assert_eq!(sub.status, SubscriptionStatus::Active);
            assert_eq!(sub.plan_name, "Premium");
        }
    }
}
