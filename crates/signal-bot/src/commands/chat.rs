//! Chat command - proxies messages to NEAR AI.

use crate::bot_config::BotConfigProvider;
use crate::commands::CommandHandler;
use crate::error::AppResult;
use crate::group::GroupConfigStore;
use crate::payments::PaymentGate;
use async_trait::async_trait;
use conversation_store::{ConversationStore, StoredToolCall};
use near_ai_client::{
    FunctionDefinitionApi, Message, NearAiClient, NearAiError, Role,
    ToolDefinition as NearToolDefinition,
};
use signal_client::{BotMessage, SignalClient};
use std::sync::Arc;
use tools::{FunctionCall as ToolsFunctionCall, ToolCall as ToolsToolCall, ToolExecutor, ToolRegistry};
use tracing::{debug, error, info, instrument, warn};

pub struct ChatHandler {
    near_ai: Arc<NearAiClient>,
    conversations: Arc<ConversationStore>,
    signal_client: Arc<SignalClient>,
    tool_executor: Arc<ToolExecutor>,
    tool_registry: Arc<ToolRegistry>,
    system_prompt: String,
    max_tool_iterations: usize,
    /// Signal username for identity in system prompt.
    signal_username: Option<String>,
    /// GitHub repo URL for identity in system prompt.
    github_repo: Option<String>,
    /// Optional payment gate for access control and usage tracking.
    payment_gate: Option<Arc<dyn PaymentGate>>,
    /// Optional per-bot config provider for multi-tenant deployments.
    bot_config_provider: Option<Arc<BotConfigProvider>>,
    /// Per-group config store (mode, etc.).
    group_config: GroupConfigStore,
}

impl ChatHandler {
    pub fn new(
        near_ai: Arc<NearAiClient>,
        conversations: Arc<ConversationStore>,
        signal_client: Arc<SignalClient>,
        tool_registry: Arc<ToolRegistry>,
        system_prompt: String,
        max_tool_iterations: usize,
        signal_username: Option<String>,
        github_repo: Option<String>,
        group_config: GroupConfigStore,
    ) -> Self {
        Self {
            near_ai,
            conversations,
            signal_client,
            tool_executor: Arc::new(ToolExecutor::new(tool_registry.clone())),
            tool_registry,
            system_prompt,
            max_tool_iterations,
            signal_username,
            github_repo,
            payment_gate: None,
            bot_config_provider: None,
            group_config,
        }
    }

    /// Create a new ChatHandler with a payment gate.
    pub fn with_payment_gate(
        near_ai: Arc<NearAiClient>,
        conversations: Arc<ConversationStore>,
        signal_client: Arc<SignalClient>,
        tool_registry: Arc<ToolRegistry>,
        system_prompt: String,
        max_tool_iterations: usize,
        signal_username: Option<String>,
        github_repo: Option<String>,
        payment_gate: Arc<dyn PaymentGate>,
        group_config: GroupConfigStore,
    ) -> Self {
        Self {
            near_ai,
            conversations,
            signal_client,
            tool_executor: Arc::new(ToolExecutor::new(tool_registry.clone())),
            tool_registry,
            system_prompt,
            max_tool_iterations,
            signal_username,
            github_repo,
            payment_gate: Some(payment_gate),
            bot_config_provider: None,
            group_config,
        }
    }

    /// Attach a per-bot config provider for multi-tenant deployments.
    pub fn with_bot_config(mut self, provider: Arc<BotConfigProvider>) -> Self {
        self.bot_config_provider = Some(provider);
        self
    }

    /// Build system prompt with identity information and current timestamp.
    fn build_system_prompt(&self, base_prompt: Option<&str>) -> String {
        let prompt = base_prompt.unwrap_or(&self.system_prompt);
        crate::config::build_system_prompt_with_identity(
            prompt,
            self.signal_username.as_deref(),
            self.github_repo.as_deref(),
        )
    }

    /// Get the system prompt and model for a specific account, using per-bot config if available.
    async fn get_bot_config_for(&self, receiving_account: &str) -> (String, Option<String>) {
        if let Some(ref provider) = self.bot_config_provider {
            let config = provider.get_config(receiving_account).await;
            let prompt = if let Some(ref p) = config.system_prompt {
                self.build_system_prompt(Some(p))
            } else {
                self.build_system_prompt(None)
            };
            return (prompt, config.model);
        }
        (self.build_system_prompt(None), None)
    }

    /// Check if the bot is mentioned in a group message.
    fn is_bot_mentioned(&self, text: &str) -> bool {
        let lower = text.to_lowercase();
        // Check for @username mention
        if let Some(ref username) = self.signal_username {
            let at_mention = format!("@{}", username.to_lowercase());
            if lower.contains(&at_mention) {
                return true;
            }
            // Also check just the username part before the dot (e.g., "nearai" from "nearai.54")
            if let Some(base) = username.split('.').next() {
                let at_base = format!("@{}", base.to_lowercase());
                if lower.contains(&at_base) {
                    return true;
                }
            }
        }
        false
    }

    /// Strip the bot's @mention from the message text.
    fn strip_mention(&self, text: &str) -> String {
        let mut result = text.to_string();
        if let Some(ref username) = self.signal_username {
            // Remove @username.XX and @username variants (case-insensitive)
            let at_full = format!("@{}", username);
            result = result.replace(&at_full, "").replace(&at_full.to_lowercase(), "");
            if let Some(base) = username.split('.').next() {
                let at_base = format!("@{}", base);
                result = result.replace(&at_base, "").replace(&at_base.to_lowercase(), "");
            }
        }
        result.trim().to_string()
    }

    /// Build messages for NEAR AI request from conversation store.
    async fn build_messages(&self, conversation_id: &str, system_prompt: &str) -> AppResult<Vec<Message>> {
        let system_prompt = system_prompt.to_string();
        let stored_messages = self
            .conversations
            .to_openai_messages(conversation_id, Some(&system_prompt))
            .await?;

        // Convert to NEAR AI message format
        let messages: Vec<Message> = stored_messages
            .into_iter()
            .map(|m| {
                // Convert tool_calls from StoredToolCall to ToolCall if present
                let tool_calls = m.tool_calls.map(|calls| {
                    calls
                        .into_iter()
                        .map(|c| near_ai_client::ToolCall {
                            id: c.id,
                            call_type: "function".to_string(),
                            function: near_ai_client::FunctionCall {
                                name: c.name,
                                arguments: c.arguments,
                            },
                        })
                        .collect()
                });

                Message {
                    role: match m.role.as_str() {
                        "system" => Role::System,
                        "assistant" => Role::Assistant,
                        "tool" => Role::Tool,
                        _ => Role::User,
                    },
                    content: m.content,
                    tool_call_id: m.tool_call_id,
                    tool_calls,
                }
            })
            .collect();

        Ok(messages)
    }

    /// Finalize and store the response.
    async fn finalize_response(
        &self,
        conversation_id: &str,
        content: Option<String>,
    ) -> AppResult<String> {
        let response = content.unwrap_or_else(|| "I don't have a response.".into());
        self.conversations
            .add_message(conversation_id, "assistant", &response, None)
            .await?;
        Ok(response)
    }
}

#[async_trait]
impl CommandHandler for ChatHandler {
    fn is_default(&self) -> bool {
        true
    }

    #[instrument(skip(self, message), fields(user = %message.source, is_group = %message.is_group))]
    async fn execute(&self, message: &BotMessage) -> AppResult<String> {
        // Use reply_target as conversation key:
        // - For DMs: sender's phone number
        // - For groups: group_id (shared context for all members)
        let conversation_id = message.reply_target();
        // For credits, use the sender's phone number (not group ID)
        let user_id = &message.source;

        // For groups: determine the message text to use (stripped of @mention)
        let user_text = if message.is_group {
            self.strip_mention(&message.text)
        } else {
            message.text.clone()
        };

        if message.is_group {
            info!(
                "Group chat from {} in {}: {}...",
                &message.source[..message.source.len().min(8)],
                &conversation_id[..conversation_id.len().min(12)],
                &user_text[..user_text.len().min(50)]
            );
        } else {
            info!(
                "Chat from {}: {}...",
                &conversation_id[..conversation_id.len().min(8)],
                &user_text[..user_text.len().min(50)]
            );
        }

        // Group @mention filter: only respond if mentioned or in responds-to-all mode
        if message.is_group {
            let mode = self.group_config.get_mode(conversation_id).await;
            if !mode.responds_to_all() && !self.is_bot_mentioned(&message.text) {
                debug!("Ignoring group message — bot not mentioned (mode: {})", mode.display_name());
                return Ok(String::new()); // Empty = no reply
            }
        }

        // Check if this is a brand new conversation (first message)
        let is_new_user = self
            .conversations
            .get(conversation_id)
            .await
            .ok()
            .flatten()
            .is_none();

        // Pre-flight payment check
        if let Some(ref gate) = self.payment_gate {
            if message.is_group {
                // Groups: rate-limit by group_id
                if let Err(msg) = gate.check_group_access(conversation_id).await {
                    return Ok(msg);
                }
            } else {
                // DMs: rate-limit by sender
                if let Err(msg) = gate.check_access(user_id, user_text.len()).await {
                    return Ok(msg);
                }
            }
        }

        // Resolve per-bot system prompt and model (or use global defaults)
        let (mut system_prompt, model_override) = self.get_bot_config_for(&message.receiving_account).await;

        // For groups: append group mode suffix to system prompt
        if message.is_group {
            let mode = self.group_config.get_mode(conversation_id).await;
            let suffix = mode.system_prompt_suffix();
            if !suffix.is_empty() {
                system_prompt.push_str(suffix);
            }
        }

        // Send welcome message for first-time users (DMs only)
        if is_new_user && !message.is_group {
            let welcome = "Welcome! I'm an AI assistant running securely in a TEE. \
                           Send `!help` for commands or just start chatting.";
            let _ = self.signal_client.reply(message, welcome).await;
        }

        // Add user message to history (use cleaned text, not raw with @mention)
        self.conversations
            .add_message(conversation_id, "user", &user_text, Some(&system_prompt))
            .await?;

        // Get tool definitions and convert to NEAR AI format
        let tool_defs = self.tool_registry.get_definitions();
        let near_tools: Vec<NearToolDefinition> = tool_defs
            .into_iter()
            .map(|d| NearToolDefinition {
                tool_type: d.tool_type,
                function: FunctionDefinitionApi {
                    name: d.function.name,
                    description: d.function.description,
                    parameters: d.function.parameters,
                },
            })
            .collect();

        // Tool execution loop - only offer tools on first iteration
        let mut tools_executed = false;
        // Track total token usage across all iterations
        let mut total_prompt_tokens: u32 = 0;
        let mut total_completion_tokens: u32 = 0;

        for iteration in 0..self.max_tool_iterations {
            debug!("Tool execution loop iteration {}, tools_executed={}", iteration, tools_executed);

            // Build messages from conversation store
            let messages = self.build_messages(conversation_id, &system_prompt).await?;

            // Only offer tools if we haven't executed any yet
            let tools_to_offer = if !tools_executed && !near_tools.is_empty() {
                Some(&near_tools[..])
            } else {
                None
            };

            // Call NEAR AI with tools (or without if already executed)
            let response = match self
                .near_ai
                .chat_with_tools_and_model(
                    messages,
                    Some(0.7),
                    None,
                    tools_to_offer,
                    model_override.as_deref(),
                )
                .await
            {
                Ok(r) => r,
                Err(NearAiError::RateLimit) => {
                    return Ok(
                        "I'm receiving too many requests. Please wait a moment and try again."
                            .into(),
                    );
                }
                Err(NearAiError::EmptyResponse) => {
                    error!("NEAR AI returned empty response");
                    return Ok(
                        "The AI service returned an empty response. Please try rephrasing your message."
                            .into(),
                    );
                }
                Err(e) => {
                    error!("NEAR AI error: {}", e);
                    return Ok(
                        "Sorry, I encountered an error connecting to the AI service. Please try again."
                            .into(),
                    );
                }
            };

            // Accumulate token usage (if available)
            if let Some(ref usage) = response.usage {
                total_prompt_tokens += usage.prompt_tokens;
                total_completion_tokens += usage.completion_tokens;
                debug!(
                    "Accumulated usage: {} prompt + {} completion tokens",
                    total_prompt_tokens, total_completion_tokens
                );
            }

            // Check if response has tool calls (must be non-empty)
            if let Some(tool_calls) = response.tool_calls {
                if tool_calls.is_empty() {
                    // Empty tool_calls array - treat as final response
                    debug!("LLM returned empty tool_calls array, treating as final response");
                } else {
                debug!("LLM requested {} tool calls", tool_calls.len());

                // Store assistant message with tool calls
                let stored_calls: Vec<StoredToolCall> = tool_calls
                    .iter()
                    .map(|tc| StoredToolCall {
                        id: tc.id.clone(),
                        name: tc.function.name.clone(),
                        arguments: tc.function.arguments.clone(),
                    })
                    .collect();

                self.conversations
                    .add_assistant_with_tools(conversation_id, response.content.as_deref(), &stored_calls)
                    .await?;

                // Execute each tool call
                for tool_call in tool_calls {
                    // Send progress message
                    let progress_msg = format!("🔧 Using {}...", tool_call.function.name);
                    if let Err(e) = self
                        .signal_client
                        .send(&message.receiving_account, message.reply_target(), &progress_msg)
                        .await
                    {
                        warn!("Failed to send progress message: {}", e);
                    }

                    // Convert to tools crate format and execute
                    let tools_call = ToolsToolCall {
                        id: tool_call.id.clone(),
                        call_type: tool_call.call_type.clone(),
                        function: ToolsFunctionCall {
                            name: tool_call.function.name.clone(),
                            arguments: tool_call.function.arguments.clone(),
                        },
                    };

                    let result = self.tool_executor.execute(&tools_call).await;
                    let result_content = if result.success {
                        debug!("Tool {} succeeded: {}...", tool_call.function.name, &result.content[..result.content.len().min(100)]);
                        result.content
                    } else {
                        warn!("Tool {} failed: {}", tool_call.function.name, result.content);
                        result.content
                    };

                    // Store tool result
                    self.conversations
                        .add_tool_result(conversation_id, &tool_call.id, &result_content)
                        .await?;
                }

                // Mark that tools have been executed - don't offer them again
                tools_executed = true;

                // Continue loop to let LLM process tool results
                continue;
                }  // close else (non-empty tool_calls)
            }  // close if let Some(tool_calls)

            // No tool calls (or empty array) - this is the final response
            let mut final_response = self.finalize_response(conversation_id, response.content).await?;

            // Record usage via payment gate
            if let Some(ref gate) = self.payment_gate {
                if let Some(suffix) = gate
                    .record_usage(user_id, conversation_id, total_prompt_tokens, total_completion_tokens)
                    .await
                {
                    final_response.push_str(&suffix);
                }
            }

            info!(
                "Response to {}: {} chars",
                &conversation_id[..conversation_id.len().min(12)],
                final_response.len()
            );

            return Ok(final_response);
        }

        // Max iterations reached
        warn!("Max tool iterations ({}) reached for {}", self.max_tool_iterations, conversation_id);
        Ok("I've reached my maximum number of tool uses for this request. Please start a new conversation.".into())
    }
}
