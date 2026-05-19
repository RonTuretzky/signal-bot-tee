//! Payment provider abstraction.
//!
//! Provides a common trait that both Stripe and x402 payment systems implement,
//! allowing the signal-bot to work with either provider without code changes.

use async_trait::async_trait;
use std::sync::Arc;
use stripe_payments::SubscriptionStore;
use x402_payments::{calculate_credits, estimate_credits, CreditStore, PricingConfig, TokenUsage, UsageRecord};

/// Payment gating trait — checks access and records usage.
#[async_trait]
pub trait PaymentGate: Send + Sync {
    /// Check if a user can send a message. Returns Ok(()) or a user-facing error.
    async fn check_access(&self, user_id: &str, message_len: usize) -> Result<(), String>;

    /// Check if a group can use the bot (rate-limited by group_id).
    /// Default implementation uses the same logic as check_access with group_id as the key.
    async fn check_group_access(&self, group_id: &str) -> Result<(), String> {
        self.check_access(group_id, 0).await
    }

    /// Record usage after a message completes.
    async fn record_usage(
        &self,
        user_id: &str,
        conversation_id: &str,
        prompt_tokens: u32,
        completion_tokens: u32,
    ) -> Option<String>;

    /// Get status text for the user (balance, subscription info, etc.).
    async fn status_text(&self, user_id: &str) -> String;
}

/// x402 credit-based payment gate.
pub struct X402Gate {
    pub credit_store: Arc<CreditStore>,
    pub pricing_config: PricingConfig,
}

#[async_trait]
impl PaymentGate for X402Gate {
    async fn check_access(&self, user_id: &str, message_len: usize) -> Result<(), String> {
        let estimated = estimate_credits(message_len, &self.pricing_config);
        if !self.credit_store.has_credits(user_id, estimated).await {
            let balance = self.credit_store.get_balance(user_id).await;
            let remaining = format_credits(balance.credits_remaining);
            return Err(format!(
                "Insufficient credits. You have {} remaining.\n\n\
                 Use `!deposit` to add USDC and get more credits.",
                remaining
            ));
        }
        Ok(())
    }

    async fn record_usage(
        &self,
        user_id: &str,
        conversation_id: &str,
        prompt_tokens: u32,
        completion_tokens: u32,
    ) -> Option<String> {
        let token_usage = TokenUsage::new(prompt_tokens, completion_tokens);
        let credits_used = calculate_credits(&token_usage, &self.pricing_config);

        let usage_record = UsageRecord::new(
            user_id.to_string(),
            conversation_id.to_string(),
            prompt_tokens,
            completion_tokens,
            credits_used,
        );

        match self.credit_store.deduct_credits(user_id, credits_used, usage_record).await {
            Ok(new_balance) => {
                let cost = format_credits(credits_used);
                let remaining = format_credits(new_balance.credits_remaining);
                Some(format!(
                    "\n\n_Cost: {} ({} tokens) | Balance: {}_",
                    cost,
                    prompt_tokens + completion_tokens,
                    remaining
                ))
            }
            Err(e) => {
                tracing::error!("Failed to deduct credits for {}: {}", user_id, e);
                None
            }
        }
    }

    async fn status_text(&self, user_id: &str) -> String {
        let balance = self.credit_store.get_balance(user_id).await;
        format!(
            "Credits remaining: {}\nTotal deposited: {}\nTotal consumed: {}",
            format_credits(balance.credits_remaining),
            format_credits(balance.total_deposited),
            format_credits(balance.total_consumed),
        )
    }
}

/// Stripe subscription-based payment gate.
pub struct StripeGate {
    pub store: Arc<SubscriptionStore>,
    pub free_daily_messages: u32,
}

#[async_trait]
impl PaymentGate for StripeGate {
    async fn check_access(&self, user_id: &str, _message_len: usize) -> Result<(), String> {
        self.store
            .can_send_message(user_id, self.free_daily_messages)
            .await
    }

    async fn check_group_access(&self, group_id: &str) -> Result<(), String> {
        // Groups get the same daily limit, keyed by group_id
        // Any paid member in the group could upgrade the group in the future,
        // but for now groups are always free-tier limited.
        let usage = self.store.get_daily_usage(group_id).await;
        if usage.message_count >= self.free_daily_messages {
            return Err(format!(
                "This group has used all {} free messages for today.\n\n\
                 In a DM with me, send `!subscribe` to get unlimited messages (works in all your groups too).",
                self.free_daily_messages
            ));
        }
        Ok(())
    }

    async fn record_usage(
        &self,
        user_id: &str,
        _conversation_id: &str,
        prompt_tokens: u32,
        completion_tokens: u32,
    ) -> Option<String> {
        match self.store.increment_usage(user_id, prompt_tokens, completion_tokens).await {
            Ok(usage) => {
                let sub = self.store.get_subscription(user_id).await;
                if sub.status.is_unlimited() {
                    None // Don't clutter paid users' messages
                } else {
                    Some(format!(
                        "\n\n_{}/{} free messages used today_",
                        usage.message_count, self.free_daily_messages
                    ))
                }
            }
            Err(e) => {
                tracing::error!("Failed to record usage for {}: {}", user_id, e);
                None
            }
        }
    }

    async fn status_text(&self, user_id: &str) -> String {
        let sub = self.store.get_subscription(user_id).await;
        let usage = self.store.get_daily_usage(user_id).await;

        let mut lines = vec![
            format!("Plan: {}", sub.plan_name),
            format!("Status: {:?}", sub.status),
        ];

        if sub.status.is_unlimited() {
            lines.push(format!("Messages today: {}", usage.message_count));
        } else {
            lines.push(format!(
                "Messages today: {}/{}",
                usage.message_count, self.free_daily_messages
            ));
        }

        if let Some(end) = sub.current_period_end {
            lines.push(format!("Renews: {}", end.format("%Y-%m-%d")));
        }

        lines.join("\n")
    }
}

/// Format credits as USD.
fn format_credits(credits: u64) -> String {
    let usdc = credits as f64 / 1_000_000.0;
    if usdc < 0.01 {
        format!("${:.4}", usdc)
    } else {
        format!("${:.2}", usdc)
    }
}
