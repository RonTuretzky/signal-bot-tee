//! Group mode command — sets the bot's behavior in a group.

use crate::commands::CommandHandler;
use crate::error::AppResult;
use crate::group::{GroupConfigStore, GroupMode};
use async_trait::async_trait;
use signal_client::BotMessage;

pub struct ModeHandler {
    group_config: GroupConfigStore,
}

impl ModeHandler {
    pub fn new(group_config: GroupConfigStore) -> Self {
        Self { group_config }
    }
}

#[async_trait]
impl CommandHandler for ModeHandler {
    fn trigger(&self) -> Option<&str> {
        Some("!mode")
    }

    async fn execute(&self, message: &BotMessage) -> AppResult<String> {
        if !message.is_group {
            return Ok(
                "The `!mode` command only works in group chats. Add me to a group and try there!"
                    .to_string(),
            );
        }

        let group_id = message.reply_target();
        let args = message.text.strip_prefix("!mode").unwrap_or("").trim();

        if args.is_empty() {
            let current = self.group_config.get_mode(group_id).await;
            return Ok(format!(
                "**Current mode:** {}\n\n\
                 Available modes:\n\
                 `!mode assistant` — AI assistant (responds when mentioned)\n\
                 `!mode translate` — Translator (translates every message)",
                current.display_name()
            ));
        }

        match GroupMode::from_str(args) {
            Some(mode) => {
                let name = mode.display_name();
                let description = match &mode {
                    GroupMode::Assistant => "I'll respond when mentioned or when you use ! commands.",
                    GroupMode::Translate => "I'll translate every message in this group.",
                };
                self.group_config.set_mode(group_id, mode).await;
                Ok(format!("Mode set to **{}**. {}", name, description))
            }
            None => Ok(format!(
                "Unknown mode: `{}`\n\n\
                 Available modes:\n\
                 `!mode assistant` — AI assistant (responds when mentioned)\n\
                 `!mode translate` — Translator (translates every message)",
                args
            )),
        }
    }
}
