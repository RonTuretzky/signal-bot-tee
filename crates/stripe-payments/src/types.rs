//! Core types for Stripe subscription management.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Subscription status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    /// No subscription — using free tier.
    Free,
    /// Active paid subscription.
    Active,
    /// Payment failed, grace period.
    PastDue,
    /// Subscription canceled.
    Canceled,
    /// In trial period.
    Trialing,
}

impl Default for SubscriptionStatus {
    fn default() -> Self {
        Self::Free
    }
}

impl SubscriptionStatus {
    /// Whether the user can send messages.
    pub fn can_send(&self) -> bool {
        matches!(self, Self::Free | Self::Active | Self::Trialing)
    }

    /// Whether the user has unlimited messages.
    pub fn is_unlimited(&self) -> bool {
        matches!(self, Self::Active | Self::Trialing)
    }
}

/// A user's subscription record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSubscription {
    /// User identifier (Signal phone number).
    pub user_id: String,
    /// Stripe customer ID.
    pub stripe_customer_id: Option<String>,
    /// Stripe subscription ID.
    pub subscription_id: Option<String>,
    /// Current subscription status.
    pub status: SubscriptionStatus,
    /// Plan name ("Free", "Premium", "Pro").
    pub plan_name: String,
    /// When the current billing period ends.
    pub current_period_end: Option<DateTime<Utc>>,
    /// When the subscription was created.
    pub created_at: DateTime<Utc>,
    /// When the subscription was last updated.
    pub updated_at: DateTime<Utc>,
}

impl UserSubscription {
    /// Create a new free-tier subscription.
    pub fn new_free(user_id: String) -> Self {
        let now = Utc::now();
        Self {
            user_id,
            stripe_customer_id: None,
            subscription_id: None,
            status: SubscriptionStatus::Free,
            plan_name: "Free".to_string(),
            current_period_end: None,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Daily usage tracking for a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyUsage {
    /// The date (YYYY-MM-DD) this usage is for.
    pub date: String,
    /// Number of messages sent today.
    pub message_count: u32,
    /// Total prompt tokens consumed today.
    pub prompt_tokens: u32,
    /// Total completion tokens consumed today.
    pub completion_tokens: u32,
}

impl DailyUsage {
    /// Create a new daily usage record for today.
    pub fn new_today() -> Self {
        Self {
            date: Utc::now().format("%Y-%m-%d").to_string(),
            message_count: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
        }
    }

    /// Check if this usage record is for the current day.
    pub fn is_today(&self) -> bool {
        self.date == Utc::now().format("%Y-%m-%d").to_string()
    }
}
