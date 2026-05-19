//! Help command - displays available commands.

use crate::commands::CommandHandler;
use crate::error::AppResult;
use async_trait::async_trait;
use signal_client::BotMessage;

pub struct HelpHandler {
    payment_provider: String,
}

impl HelpHandler {
    pub fn new() -> Self {
        Self {
            payment_provider: "none".to_string(),
        }
    }

    pub fn with_payment_provider(provider: &str) -> Self {
        Self {
            payment_provider: provider.to_string(),
        }
    }
}

impl Default for HelpHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CommandHandler for HelpHandler {
    fn trigger(&self) -> Option<&str> {
        Some("!help")
    }

    async fn execute(&self, _message: &BotMessage) -> AppResult<String> {
        let payment_section = match self.payment_provider.as_str() {
            "stripe" => r#"
**Subscription:**
- !subscribe - Upgrade to Premium for unlimited messages
- !subscription - Check your plan and usage
- !manage - Manage billing and payment

Free tier includes 15 messages/day. Upgrade for unlimited access."#,
            "x402" => r#"
**Payments:**
This bot uses prepaid credits. Deposit USDC on Base, NEAR, or Solana to add credits.
- !balance - Check your credit balance
- !deposit - Get deposit addresses for USDC"#,
            _ => "",
        };

        Ok(format!(
            r#"**Signal AI** (Private & Verifiable)

Just send a message to chat with AI.

**Commands:**
- !verify <challenge> - Get TEE attestation with your challenge
- !clear - Clear conversation history
- !models - List available AI models
- !help - Show this message
{payment_section}

**Group Chat:**
Add me to any group! In groups I respond when @mentioned.
- !mode assistant - AI assistant (default, responds when mentioned)
- !mode translate - Auto-translate every message
Groups get 15 free messages/day.

**Verification:**
Use `!verify my-random-text` to get cryptographic proof this bot runs in a TEE.

**Privacy:**
Messages are end-to-end encrypted via Signal, processed in a TEE (Intel TDX), and sent to NEAR AI's private inference.
Neither the bot operator nor NEAR AI can read your messages."#
        ))
    }
}
