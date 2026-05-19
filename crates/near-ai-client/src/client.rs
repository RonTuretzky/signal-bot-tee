//! NEAR AI Cloud HTTP client.

use crate::error::NearAiError;
use crate::types::*;
use futures::StreamExt;
use reqwest::{Client, StatusCode};
use secrecy::{ExposeSecret, SecretString};
use std::time::Duration;
use tokio::time::sleep;
use tokio_stream::Stream;
use tracing::{debug, instrument, warn};

/// Default retry configuration
const DEFAULT_MAX_RETRIES: u32 = 3;
const DEFAULT_INITIAL_BACKOFF_MS: u64 = 100;
const DEFAULT_MAX_BACKOFF_MS: u64 = 5000;

/// NEAR AI Cloud client.
///
/// The API key is stored using `SecretString` to prevent accidental
/// exposure in logs or debug output.
#[derive(Clone)]
pub struct NearAiClient {
    client: Client,
    base_url: String,
    api_key: SecretString,
    model: String,
}

impl NearAiClient {
    /// Create a new NEAR AI client.
    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
        timeout: Duration,
    ) -> Result<Self, NearAiError> {
        let client = Client::builder()
            .timeout(timeout)
            .build()?;

        Ok(Self {
            client,
            base_url: base_url.into(),
            api_key: SecretString::new(api_key.into()),
            model: model.into(),
        })
    }

    /// Get the configured model name.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Send a chat completion request.
    #[instrument(skip(self, messages), fields(message_count = messages.len()))]
    pub async fn chat(
        &self,
        messages: Vec<Message>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<String, NearAiError> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            temperature,
            max_tokens,
            stream: Some(false),
            tools: None,
            tool_choice: None,
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key.expose_secret()))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let chat_response = self.handle_response::<ChatResponse>(response).await?;

        // Extract content from response, returning error if empty
        chat_response
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .filter(|content| !content.is_empty())
            .ok_or(NearAiError::EmptyResponse)
    }

    /// Send a chat completion request with tool support and optional model override.
    #[instrument(skip(self, messages, tools), fields(message_count = messages.len()))]
    pub async fn chat_with_tools(
        &self,
        messages: Vec<Message>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
        tools: Option<&[ToolDefinition]>,
    ) -> Result<ChatResponseWithTools, NearAiError> {
        self.chat_with_tools_and_model(messages, temperature, max_tokens, tools, None).await
    }

    /// Send a chat completion request with tool support and explicit model override.
    #[instrument(skip(self, messages, tools), fields(message_count = messages.len()))]
    pub async fn chat_with_tools_and_model(
        &self,
        messages: Vec<Message>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
        tools: Option<&[ToolDefinition]>,
        model_override: Option<&str>,
    ) -> Result<ChatResponseWithTools, NearAiError> {
        let request = ChatRequest {
            model: model_override.unwrap_or(&self.model).to_string(),
            messages,
            temperature,
            max_tokens,
            stream: Some(false),
            tools: tools.map(|t| t.to_vec()),
            tool_choice: tools.map(|_| "auto".to_string()),
        };

        let url = format!("{}/chat/completions", self.base_url);
        debug!("Sending chat_with_tools request to {}", url);

        let response = match self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key.expose_secret()))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("HTTP request to {} failed: {}", url, e);
                return Err(e.into());
            }
        };

        debug!("Response status: {}", response.status());
        let chat_response = self.handle_response::<ChatResponse>(response).await?;

        // Extract usage before consuming response
        let usage = chat_response.usage;

        // Extract from first choice
        let choice = chat_response
            .choices
            .into_iter()
            .next()
            .ok_or(NearAiError::EmptyResponse)?;

        Ok(ChatResponseWithTools {
            content: choice.message.content,
            tool_calls: choice.message.tool_calls,
            finish_reason: choice.finish_reason.unwrap_or_default(),
            usage,
        })
    }

    /// Send a streaming chat completion request.
    #[instrument(skip(self, messages), fields(message_count = messages.len()))]
    pub async fn chat_stream(
        &self,
        messages: Vec<Message>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<impl Stream<Item = Result<String, NearAiError>>, NearAiError> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            temperature,
            max_tokens,
            stream: Some(true),
            tools: None,
            tool_choice: None,
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key.expose_secret()))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(self.extract_error(response).await);
        }

        let stream = response.bytes_stream().map(|result| {
            result
                .map_err(NearAiError::from)
                .and_then(|bytes| {
                    // Parse SSE data
                    let text = String::from_utf8_lossy(&bytes);
                    let mut content = String::new();

                    for line in text.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                continue;
                            }
                            if let Ok(chunk) = serde_json::from_str::<ChatChunk>(data) {
                                if let Some(delta_content) = chunk
                                    .choices
                                    .first()
                                    .and_then(|c| c.delta.content.as_ref())
                                {
                                    content.push_str(delta_content);
                                }
                            }
                        }
                    }

                    Ok(content)
                })
        });

        Ok(stream)
    }

    /// List available models.
    /// Note: NEAR AI Cloud doesn't have a /models endpoint, so we return known models.
    #[instrument(skip(self))]
    pub async fn list_models(&self) -> Result<Vec<Model>, NearAiError> {
        // NEAR AI Cloud doesn't expose a /models endpoint
        // Return a list of known available models (as of Dec 2025)
        Ok(vec![
            Model { id: "deepseek-ai/DeepSeek-V3.1".to_string(), object: "model".to_string(), created: 0, owned_by: "deepseek".to_string() },
            Model { id: "openai/gpt-oss-120b".to_string(), object: "model".to_string(), created: 0, owned_by: "openai".to_string() },
            Model { id: "Qwen/Qwen3-30B-A3B-Instruct-2507".to_string(), object: "model".to_string(), created: 0, owned_by: "alibaba".to_string() },
            Model { id: "zai-org/GLM-4.6".to_string(), object: "model".to_string(), created: 0, owned_by: "zhipu".to_string() },
        ])
    }

    /// Get attestation report from NEAR AI.
    #[instrument(skip(self))]
    pub async fn get_attestation(&self) -> Result<AttestationReport, NearAiError> {
        let response = self
            .client
            .get(format!("{}/attestation", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key.expose_secret()))
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Send a chat completion request with automatic retry and exponential backoff.
    ///
    /// Retries on transient errors (network issues, rate limits) up to `max_retries` times.
    /// Does not retry on authentication errors or empty responses.
    #[instrument(skip(self, messages), fields(message_count = messages.len()))]
    pub async fn chat_with_retry(
        &self,
        messages: Vec<Message>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
        max_retries: Option<u32>,
    ) -> Result<String, NearAiError> {
        let max_retries = max_retries.unwrap_or(DEFAULT_MAX_RETRIES);
        let mut backoff_ms = DEFAULT_INITIAL_BACKOFF_MS;
        let mut last_error = None;

        for attempt in 0..=max_retries {
            if attempt > 0 {
                debug!("Retry attempt {} after {}ms backoff", attempt, backoff_ms);
                sleep(Duration::from_millis(backoff_ms)).await;
                // Exponential backoff with cap
                backoff_ms = (backoff_ms * 2).min(DEFAULT_MAX_BACKOFF_MS);
            }

            match self.chat(messages.clone(), temperature, max_tokens).await {
                Ok(response) => return Ok(response),
                Err(NearAiError::Unauthorized) => return Err(NearAiError::Unauthorized),
                Err(NearAiError::EmptyResponse) => return Err(NearAiError::EmptyResponse),
                Err(e) => {
                    warn!("Chat request failed (attempt {}): {}", attempt + 1, e);
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or(NearAiError::Api {
            status: 0,
            message: "Max retries exceeded".into(),
        }))
    }

    /// Health check - returns true if API is reachable.
    /// Tests connectivity by sending a minimal chat completion request.
    pub async fn health_check(&self) -> bool {
        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![Message::user("ping")],
            temperature: None,
            max_tokens: Some(1),
            stream: Some(false),
            tools: None,
            tool_choice: None,
        };

        match self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key.expose_secret()))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    true
                } else {
                    warn!("NEAR AI health check failed: HTTP {}", response.status());
                    false
                }
            }
            Err(e) => {
                warn!("NEAR AI health check failed: {}", e);
                false
            }
        }
    }

    /// Handle HTTP response, converting errors appropriately.
    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, NearAiError> {
        let status = response.status();

        if status.is_success() {
            let body = response.text().await?;
            debug!("Response body: {}", &body[..body.len().min(200)]);
            serde_json::from_str(&body).map_err(NearAiError::from)
        } else {
            Err(self.extract_error(response).await)
        }
    }

    /// Extract error information from failed response.
    async fn extract_error(&self, response: reqwest::Response) -> NearAiError {
        let status = response.status();

        match status {
            StatusCode::TOO_MANY_REQUESTS => {
                warn!("Rate limit exceeded");
                NearAiError::RateLimit
            }
            StatusCode::UNAUTHORIZED => {
                warn!("Authentication failed");
                NearAiError::Unauthorized
            }
            _ => {
                let message = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".into());
                NearAiError::Api {
                    status: status.as_u16(),
                    message,
                }
            }
        }
    }
}
