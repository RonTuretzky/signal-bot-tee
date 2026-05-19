//! Subscribe command - creates Stripe Checkout session for upgrading.

use crate::commands::CommandHandler;
use crate::error::AppResult;
use async_trait::async_trait;
use signal_client::BotMessage;
use std::sync::Arc;
use stripe_payments::client::StripeClient;
use stripe_payments::config::StripeConfig;
use stripe_payments::SubscriptionStore;
use tracing::info;

pub struct SubscribeHandler {
    stripe_client: StripeClient,
    store: Arc<SubscriptionStore>,
    config: StripeConfig,
}

impl SubscribeHandler {
    pub fn new(
        stripe_client: StripeClient,
        store: Arc<SubscriptionStore>,
        config: StripeConfig,
    ) -> Self {
        Self {
            stripe_client,
            store,
            config,
        }
    }
}

#[async_trait]
impl CommandHandler for SubscribeHandler {
    fn trigger(&self) -> Option<&str> {
        Some("!subscribe")
    }

    async fn execute(&self, message: &BotMessage) -> AppResult<String> {
        let user_id = &message.source;
        let sub = self.store.get_subscription(user_id).await;

        // Already subscribed?
        if sub.status.is_unlimited() {
            return Ok(format!(
                "You're already on the **{}** plan! Send `!manage` to manage your subscription.",
                sub.plan_name
            ));
        }

        // Build checkout URLs
        let success_url = format!("{}?subscribed=true", self.config.frontend_url);
        let cancel_url = format!("{}?canceled=true", self.config.frontend_url);

        let mut plans_text = String::from("**Upgrade Your Plan**\n\n");
        plans_text.push_str(&format!(
            "You're on the Free plan ({} messages/day).\n\n",
            self.config.free_daily_messages
        ));

        // Create checkout session for Premium plan
        if let Some(ref price_id) = self.config.premium_price_id {
            let customer_id = sub.stripe_customer_id.as_deref();
            match self
                .stripe_client
                .create_checkout_session(
                    price_id,
                    user_id,
                    &success_url,
                    &cancel_url,
                    customer_id,
                )
                .await
            {
                Ok(session) => {
                    if let Some(url) = session.url {
                        plans_text.push_str(&format!(
                            "**Premium** — $19/mo (unlimited messages)\n{}\n\n",
                            url
                        ));
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to create checkout session: {}", e);
                    return Ok(
                        "Sorry, there was an error creating the checkout session. Please try again."
                            .to_string(),
                    );
                }
            }
        }

        // Create checkout session for Pro plan
        if let Some(ref price_id) = self.config.pro_price_id {
            let customer_id = sub.stripe_customer_id.as_deref();
            match self
                .stripe_client
                .create_checkout_session(
                    price_id,
                    user_id,
                    &success_url,
                    &cancel_url,
                    customer_id,
                )
                .await
            {
                Ok(session) => {
                    if let Some(url) = session.url {
                        plans_text.push_str(&format!(
                            "**Pro** — $49/mo (unlimited + team features)\n{}\n\n",
                            url
                        ));
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to create Pro checkout session: {}", e);
                }
            }
        }

        if self.config.premium_price_id.is_none() && self.config.pro_price_id.is_none() {
            return Ok(
                "Subscriptions are not configured yet. Please try again later.".to_string(),
            );
        }

        plans_text.push_str("Click the link to complete your subscription in your browser.");

        info!("Sent subscription options to {}", user_id);
        Ok(plans_text)
    }
}

/// Subscription status command - shows current plan and usage.
pub struct SubscriptionHandler {
    store: Arc<SubscriptionStore>,
    free_daily_messages: u32,
}

impl SubscriptionHandler {
    pub fn new(store: Arc<SubscriptionStore>, free_daily_messages: u32) -> Self {
        Self {
            store,
            free_daily_messages,
        }
    }
}

#[async_trait]
impl CommandHandler for SubscriptionHandler {
    fn trigger(&self) -> Option<&str> {
        Some("!subscription")
    }

    async fn execute(&self, message: &BotMessage) -> AppResult<String> {
        let user_id = &message.source;
        let sub = self.store.get_subscription(user_id).await;
        let usage = self.store.get_daily_usage(user_id).await;

        let mut response = format!("**Your Subscription**\n\nPlan: {}\n", sub.plan_name);

        match sub.status {
            stripe_payments::SubscriptionStatus::Active => {
                response.push_str("Status: Active\n");
                response.push_str(&format!("Messages today: {}\n", usage.message_count));
                if let Some(end) = sub.current_period_end {
                    response.push_str(&format!("Renews: {}\n", end.format("%Y-%m-%d")));
                }
                response.push_str("\nSend `!manage` to change or cancel your plan.");
            }
            stripe_payments::SubscriptionStatus::Free => {
                response.push_str(&format!(
                    "Messages today: {}/{}\n\n",
                    usage.message_count, self.free_daily_messages
                ));
                response.push_str("Send `!subscribe` to upgrade for unlimited messages.");
            }
            stripe_payments::SubscriptionStatus::PastDue => {
                response.push_str("Status: Past Due (payment failed)\n\n");
                response.push_str("Send `!manage` to update your payment method.");
            }
            stripe_payments::SubscriptionStatus::Canceled => {
                response.push_str("Status: Canceled\n\n");
                response.push_str("Send `!subscribe` to resubscribe.");
            }
            stripe_payments::SubscriptionStatus::Trialing => {
                response.push_str("Status: Trial\n");
                response.push_str(&format!("Messages today: {}\n", usage.message_count));
            }
        }

        Ok(response)
    }
}

/// Manage command - returns Stripe Customer Portal URL.
pub struct ManageHandler {
    stripe_client: StripeClient,
    store: Arc<SubscriptionStore>,
    config: StripeConfig,
}

impl ManageHandler {
    pub fn new(
        stripe_client: StripeClient,
        store: Arc<SubscriptionStore>,
        config: StripeConfig,
    ) -> Self {
        Self {
            stripe_client,
            store,
            config,
        }
    }
}

#[async_trait]
impl CommandHandler for ManageHandler {
    fn trigger(&self) -> Option<&str> {
        Some("!manage")
    }

    async fn execute(&self, message: &BotMessage) -> AppResult<String> {
        let user_id = &message.source;
        let sub = self.store.get_subscription(user_id).await;

        let customer_id = match sub.stripe_customer_id {
            Some(ref cid) => cid.clone(),
            None => {
                return Ok(
                    "You don't have an active subscription. Send `!subscribe` to get started."
                        .to_string(),
                );
            }
        };

        let return_url = self.config.frontend_url.clone();
        match self
            .stripe_client
            .create_portal_session(&customer_id, &return_url)
            .await
        {
            Ok(session) => {
                info!("Created portal session for {}", user_id);
                Ok(format!(
                    "**Manage Subscription**\n\n\
                     Click to manage your plan, update payment, or cancel:\n{}",
                    session.url
                ))
            }
            Err(e) => {
                tracing::error!("Failed to create portal session: {}", e);
                Ok("Sorry, there was an error. Please try again.".to_string())
            }
        }
    }
}
