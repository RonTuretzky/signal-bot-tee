//! Per-group configuration (mode, rate limits).

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Group operating mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupMode {
    /// General AI assistant — responds when mentioned.
    Assistant,
    /// Translator — translates every message, no mention needed.
    Translate,
}

impl GroupMode {
    pub fn system_prompt_suffix(&self) -> &'static str {
        match self {
            GroupMode::Assistant => "",
            GroupMode::Translate => concat!(
                "\n\n## Group Translator Mode\n",
                "You are in translator mode for this group chat. ",
                "For every message, detect the language and translate it to English. ",
                "If the message is already in English, translate it to Spanish. ",
                "Keep translations natural and conversational. ",
                "Just provide the translation — no explanations or prefixes unless asked. ",
                "If the message is very short (emoji, 'ok', 'yes'), just echo it back unchanged."
            ),
        }
    }

    /// Whether the bot should respond to every message (no mention required).
    pub fn responds_to_all(&self) -> bool {
        match self {
            GroupMode::Assistant => false,
            GroupMode::Translate => true,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            GroupMode::Assistant => "Assistant",
            GroupMode::Translate => "Translator",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "assistant" | "chat" | "default" => Some(GroupMode::Assistant),
            "translate" | "translator" | "translation" => Some(GroupMode::Translate),
            _ => None,
        }
    }
}

impl Default for GroupMode {
    fn default() -> Self {
        GroupMode::Assistant
    }
}

/// In-memory store for per-group configuration.
/// Ephemeral — resets on restart (same as conversations).
#[derive(Clone)]
pub struct GroupConfigStore {
    modes: Arc<RwLock<HashMap<String, GroupMode>>>,
}

impl GroupConfigStore {
    pub fn new() -> Self {
        Self {
            modes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_mode(&self, group_id: &str) -> GroupMode {
        self.modes
            .read()
            .await
            .get(group_id)
            .cloned()
            .unwrap_or_default()
    }

    pub async fn set_mode(&self, group_id: &str, mode: GroupMode) {
        self.modes
            .write()
            .await
            .insert(group_id.to_string(), mode);
    }
}
