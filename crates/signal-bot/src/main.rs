//! Signal AI Proxy Bot - Main entry point.

use signal_bot::bot_config::BotConfigProvider;
use signal_bot::commands::*;
use signal_bot::config::Config;
use signal_bot::error::AppResult;
use signal_bot::group::GroupConfigStore;
use signal_bot::payments::{PaymentGate, StripeGate, X402Gate};
use anyhow::Context;
use conversation_store::ConversationStore;
use dstack_client::DstackClient;
use near_ai_client::NearAiClient;
use signal_client::{MessageReceiver, SignalClient};
use std::sync::Arc;
use tokio::signal;
use tokio_stream::StreamExt;
use tools::{ToolRegistry, builtin::{CalculatorTool, WeatherTool, WebSearchTool}};
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Create and configure tool registry based on config.
fn create_tool_registry(config: &signal_bot::config::ToolsConfig) -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    if !config.enabled {
        info!("Tools system disabled by configuration");
        return registry;
    }

    // Calculator - always available (no API key needed)
    if config.calculator.enabled {
        registry.register(Arc::new(CalculatorTool::new()));
        info!("Registered tool: calculate");
    }

    // Weather - always available (no API key needed)
    if config.weather.enabled {
        registry.register(Arc::new(WeatherTool::new()));
        info!("Registered tool: get_weather");
    }

    // Web search - requires API key
    if config.web_search.enabled {
        if let Some(api_key) = &config.web_search.api_key {
            let tool = WebSearchTool::new(api_key.clone())
                .with_max_results(config.web_search.max_results);
            registry.register(Arc::new(tool));
            info!("Registered tool: web_search (max_results: {})", config.web_search.max_results);
        } else {
            warn!("Web search tool enabled but TOOLS__WEB_SEARCH__API_KEY not set - skipping");
        }
    }

    let enabled_count = registry.list_enabled().len();
    info!("Tool registry ready with {} enabled tools", enabled_count);

    registry
}

#[tokio::main]
async fn main() -> AppResult<()> {
    // Load configuration
    let config = Config::load().context("Failed to load configuration")?;

    // Initialize logging
    init_logging(&config.bot.log_level);

    info!("Starting Signal AI Proxy Bot...");

    // Initialize clients
    let near_ai = Arc::new(
        NearAiClient::new(
            &config.near_ai.api_key,
            &config.near_ai.base_url,
            &config.near_ai.model,
            config.near_ai.timeout,
        )
        .context("Failed to create NEAR AI client")?,
    );

    let conversations = Arc::new(ConversationStore::new(
        config.conversation.max_messages,
        config.conversation.ttl,
    ));

    let dstack = Arc::new(DstackClient::new(&config.dstack.socket_path));

    let signal = Arc::new(
        SignalClient::new(&config.signal.service_url)
            .context("Failed to create Signal client")?,
    );

    // Create tool registry based on config
    let tool_registry = Arc::new(create_tool_registry(&config.tools));

    // Initialize payment system based on provider
    let payment_gate: Option<Arc<dyn PaymentGate>> = match config.payments.provider.as_str() {
        "stripe" => {
            info!("Initializing Stripe payment system...");
            let stripe_dstack = DstackClient::new(&config.dstack.socket_path);
            let stripe_config = config.payments.stripe.clone();

            match stripe_payments::spawn_stripe_server(stripe_config.clone(), stripe_dstack).await {
                Ok(Some((handle, store))) => {
                    info!("Stripe webhook server started on port {}", stripe_config.server_port);
                    tokio::spawn(async move { handle.await.ok(); });

                    let gate: Arc<dyn PaymentGate> = Arc::new(StripeGate {
                        store,
                        free_daily_messages: stripe_config.free_daily_messages,
                    });
                    Some(gate)
                }
                Ok(None) => {
                    warn!("Stripe configured but not enabled");
                    None
                }
                Err(e) => {
                    error!("Failed to initialize Stripe: {}", e);
                    None
                }
            }
        }
        "x402" => {
            info!("Initializing x402 payment system...");
            let x402_config = config.payments.x402.clone();

            let payment_dstack = DstackClient::new(&config.dstack.socket_path);
            let server_dstack = DstackClient::new(&config.dstack.socket_path);

            let store = x402_payments::CreditStore::new(
                payment_dstack,
                x402_config.storage_path.clone(),
            )
            .await
            .context("Failed to initialize credit store")?;

            // Spawn x402 payment HTTP server
            if let Some(handle) = x402_payments::spawn_payment_server(
                x402_config.clone(),
                server_dstack,
            )
            .await
            .context("Failed to start x402 payment server")? {
                info!("x402 payment server started on port {}", x402_config.server_port);
                tokio::spawn(async move {
                    if let Err(e) = handle.await {
                        error!("x402 payment server error: {:?}", e);
                    }
                });
            }

            let gate: Arc<dyn PaymentGate> = Arc::new(X402Gate {
                credit_store: store,
                pricing_config: x402_config.pricing.clone(),
            });
            Some(gate)
        }
        "none" | "" => {
            info!("Payments disabled");
            None
        }
        other => {
            warn!("Unknown payment provider '{}', payments disabled", other);
            None
        }
    };

    // Health checks
    if near_ai.health_check().await {
        info!("NEAR AI healthy - Model: {}", config.near_ai.model);
    } else {
        warn!("NEAR AI health check failed - will retry on requests");
    }

    info!(
        "In-memory conversation store ready (max_messages={}, ttl={:?})",
        config.conversation.max_messages, config.conversation.ttl
    );

    if dstack.is_in_tee().await {
        if let Ok(info) = dstack.get_app_info().await {
            info!(
                "Running in TEE - App ID: {}",
                info.app_id.as_deref().unwrap_or("unknown")
            );
        }
    } else {
        warn!("Not running in TEE environment - attestation unavailable");
    }

    if !signal.health_check().await {
        error!("Signal API not reachable at {}", config.signal.service_url);
        return Err(anyhow::anyhow!("Signal API not reachable").into());
    }
    info!("Signal API healthy");

    // Create per-bot config provider for multi-tenant deployments
    let bot_config_provider = Arc::new(BotConfigProvider::new(
        &config.signal.proxy_url,
        &config.near_ai.model,
        &config.bot.system_prompt,
    ));
    info!("Per-bot config provider ready (proxy: {})", config.signal.proxy_url);

    // Create group config store (shared between chat handler and mode command)
    let group_config = GroupConfigStore::new();

    // Create command handlers
    let chat_handler: Box<dyn CommandHandler> = match payment_gate {
        Some(ref gate) => Box::new(
            ChatHandler::with_payment_gate(
                near_ai.clone(),
                conversations.clone(),
                signal.clone(),
                tool_registry.clone(),
                config.bot.system_prompt.clone(),
                config.tools.max_tool_calls,
                config.bot.signal_username.clone(),
                config.bot.github_repo.clone(),
                gate.clone(),
                group_config.clone(),
            )
            .with_bot_config(bot_config_provider.clone()),
        ),
        None => Box::new(
            ChatHandler::new(
                near_ai.clone(),
                conversations.clone(),
                signal.clone(),
                tool_registry.clone(),
                config.bot.system_prompt.clone(),
                config.tools.max_tool_calls,
                config.bot.signal_username.clone(),
                config.bot.github_repo.clone(),
                group_config.clone(),
            )
            .with_bot_config(bot_config_provider.clone()),
        ),
    };

    let mut handlers: Vec<Box<dyn CommandHandler>> = vec![
        chat_handler,
        Box::new(VerifyHandler::new(dstack.clone())),
        Box::new(ClearHandler::new(conversations.clone())),
        Box::new(HelpHandler::with_payment_provider(&config.payments.provider)),
        Box::new(ModelsHandler::new(near_ai.clone())),
        Box::new(ModeHandler::new(group_config.clone())),
    ];

    // Add payment-provider-specific command handlers
    match config.payments.provider.as_str() {
        "stripe" => {
            // We need to get the store again for command handlers
            // The Stripe store is inside the StripeGate
            if payment_gate.is_some() {
                // Downcast to get the store — but we can avoid that by storing it separately.
                // Instead, re-initialize store access from config.
                let stripe_config = config.payments.stripe.clone();
                let cmd_dstack = DstackClient::new(&config.dstack.socket_path);
                if let Ok(store) = stripe_payments::SubscriptionStore::new(
                    cmd_dstack,
                    stripe_config.storage_path.clone(),
                ).await {
                    let stripe_client = stripe_payments::client::StripeClient::new(&stripe_config);
                    handlers.push(Box::new(SubscribeHandler::new(
                        stripe_client.clone(),
                        store.clone(),
                        stripe_config.clone(),
                    )));
                    handlers.push(Box::new(SubscriptionHandler::new(
                        store.clone(),
                        stripe_config.free_daily_messages,
                    )));
                    handlers.push(Box::new(ManageHandler::new(
                        stripe_client,
                        store,
                        stripe_config,
                    )));
                    info!("Stripe commands enabled: !subscribe, !subscription, !manage");
                }
            }
        }
        "x402" => {
            // For x402, we need the CreditStore from the gate
            let x402_config = config.payments.x402.clone();
            let cmd_dstack = DstackClient::new(&config.dstack.socket_path);
            if let Ok(store) = x402_payments::CreditStore::new(
                cmd_dstack,
                x402_config.storage_path.clone(),
            ).await {
                handlers.push(Box::new(BalanceHandler::new(store)));
                handlers.push(Box::new(DepositHandler::new(x402_config)));
                info!("x402 commands enabled: !balance, !deposit");
            }
        }
        _ => {}
    }

    info!("Registered {} command handlers", handlers.len());
    info!("NEAR AI endpoint: {}", config.near_ai.base_url);
    info!("Listening for messages...");

    // Start message receiver
    let receiver = MessageReceiver::new((*signal).clone(), config.signal.poll_interval);
    let mut stream = Box::pin(receiver.stream());

    // Main message loop
    loop {
        tokio::select! {
            Some(message) = stream.next() => {
                // Find matching handler
                let handler = handlers
                    .iter()
                    .find(|h| h.matches(&message));

                if let Some(handler) = handler {
                    match handler.execute(&message).await {
                        Ok(response) if !response.is_empty() => {
                            if let Err(e) = signal.reply(&message, &response).await {
                                error!("Failed to send reply: {}", e);
                            }
                        }
                        Ok(_) => {
                            // Empty response — bot chose not to reply (e.g., unmentioned in group)
                        }
                        Err(e) => {
                            error!("Handler error: {}", e);
                            let _ = signal
                                .reply(&message, "Sorry, something went wrong.")
                                .await;
                        }
                    }
                }
            }
            _ = signal::ctrl_c() => {
                info!("Shutdown signal received");
                break;
            }
        }
    }

    info!("Shutting down...");
    Ok(())
}

fn init_logging(level: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}
