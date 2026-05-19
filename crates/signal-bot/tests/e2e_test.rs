//! End-to-end integration tests for Signal Bot.

mod common;

use common::{mock_near_ai_server, test_near_ai_client};
use conversation_store::ConversationStore;
use signal_client::{BotMessage, SignalClient};
use std::sync::Arc;
use std::time::Duration;
use tools::ToolRegistry;
use wiremock::matchers::{method, path, body_json, body_string_contains};
use wiremock::{Mock, MockServer, ResponseTemplate};
use signal_bot::commands::{ChatHandler, CommandHandler};
use signal_bot::group::GroupConfigStore;

#[tokio::test]
async fn test_bot_chat_e2e() {
    // 1. Setup Mock Servers
    let near_ai_server = mock_near_ai_server().await;
    let signal_server = MockServer::start().await;

    // 2. Setup Clients & Components
    let near_ai = Arc::new(test_near_ai_client(&near_ai_server));
    let conversations = Arc::new(ConversationStore::new(50, Duration::from_secs(3600)));
    let signal = Arc::new(SignalClient::new(signal_server.uri()).unwrap());
    let tool_registry = Arc::new(ToolRegistry::new());
    
    let chat_handler = ChatHandler::new(
        near_ai.clone(),
        conversations.clone(),
        signal.clone(),
        tool_registry.clone(),
        "You are a helpful assistant.".to_string(),
        5,
        None,
        None,
        GroupConfigStore::new(),
    );

    // 3. Mock NEAR AI Response
    let ai_response = serde_json::json!({
        "id": "chatcmpl-123",
        "object": "chat.completion",
        "created": 1677652288,
        "model": "test-model",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "Hello! I am your AI assistant."
            },
            "finish_reason": "stop"
        }]
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&ai_response))
        .mount(&near_ai_server)
        .await;

    // 4. Mock Signal Send Response (the reply)
    Mock::given(method("POST"))
        .and(path("/v2/send"))
        .and(body_json(serde_json::json!({
            "message": "Hello! I am your AI assistant.",
            "number": "+987654321",
            "recipients": ["+123456789"]
        })))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&signal_server)
        .await;

    // 5. Simulate Incoming Message
    let incoming = BotMessage {
        source: "+123456789".to_string(),
        text: "Hi there!".to_string(),
        timestamp: 123456789,
        is_group: false,
        group_id: None,
        receiving_account: "+987654321".to_string(),
    };

    // 6. Execute Handler
    let response = chat_handler.execute(&incoming).await.unwrap();
    assert_eq!(response, "Hello! I am your AI assistant.");

    // 7. Send reply via Signal (as main.rs does)
    signal.reply(&incoming, &response).await.unwrap();

    // 8. Verify conversation history
    let history = conversations.get("+123456789").await.unwrap().unwrap();
    assert_eq!(history.messages.len(), 2);
    assert_eq!(history.messages[0].role, "user");
    assert_eq!(history.messages[1].role, "assistant");
}

#[tokio::test]
async fn test_bot_tool_use_e2e() {
    // 1. Setup Mock Servers
    let near_ai_server = mock_near_ai_server().await;
    let signal_server = MockServer::start().await;

    // 2. Setup Clients & Components
    let near_ai = Arc::new(test_near_ai_client(&near_ai_server));
    let conversations = Arc::new(ConversationStore::new(50, Duration::from_secs(3600)));
    let signal = Arc::new(SignalClient::new(signal_server.uri()).unwrap());
    let mut tool_registry = ToolRegistry::new();
    tool_registry.register(Arc::new(tools::builtin::CalculatorTool::new()));
    let tool_registry = Arc::new(tool_registry);
    
    let chat_handler = ChatHandler::new(
        near_ai.clone(),
        conversations.clone(),
        signal.clone(),
        tool_registry.clone(),
        "You are a helpful assistant.".to_string(),
        5,
        None,
        None,
        GroupConfigStore::new(),
    );

    // 3. Mock NEAR AI Response 1: Tool Call
    let tool_call_id = "call_abc123";
    let ai_tool_response = serde_json::json!({
        "id": "chatcmpl-1",
        "object": "chat.completion",
        "created": 1677652288,
        "model": "test-model",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": tool_call_id,
                    "type": "function",
                    "function": {
                        "name": "calculate",
                        "arguments": "{\"expression\": \"2 + 2\"}"
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }]
    });

    let ai_final_response = serde_json::json!({
        "id": "chatcmpl-2",
        "object": "chat.completion",
        "created": 1677652289,
        "model": "test-model",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "The result of 2 + 2 is 4."
            },
            "finish_reason": "stop"
        }]
    });

    // Mock for SECOND call (contains tool result)
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(body_string_contains("\"role\":\"tool\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(&ai_final_response))
        .expect(1)
        .mount(&near_ai_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&ai_tool_response))
        .expect(1)
        .mount(&near_ai_server)
        .await;

    // 5. Mock Signal Progress Message
    Mock::given(method("POST"))
        .and(path("/v2/send"))
        .and(body_json(serde_json::json!({
            "message": "🔧 Using calculate...",
            "number": "+987654321",
            "recipients": ["+123456789"]
        })))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&signal_server)
        .await;

    // 6. Mock Signal Final Response
    Mock::given(method("POST"))
        .and(path("/v2/send"))
        .and(body_json(serde_json::json!({
            "message": "The result of 2 + 2 is 4.",
            "number": "+987654321",
            "recipients": ["+123456789"]
        })))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&signal_server)
        .await;

    // 7. Execute
    let incoming = BotMessage {
        source: "+123456789".to_string(),
        text: "How much is 2+2?".to_string(),
        timestamp: 123456789,
        is_group: false,
        group_id: None,
        receiving_account: "+987654321".to_string(),
    };

    let response = chat_handler.execute(&incoming).await.unwrap();
    assert_eq!(response, "The result of 2 + 2 is 4.");

    // Send final reply as in main.rs
    signal.reply(&incoming, &response).await.unwrap();

    // 8. Verify history
    let history = conversations.get("+123456789").await.unwrap().unwrap();
    // user + assistant (tool call) + tool (result) + assistant (final)
    assert_eq!(history.messages.len(), 4);
    assert_eq!(history.messages[0].role, "user");
    assert_eq!(history.messages[1].role, "assistant");
    assert_eq!(history.messages[2].role, "tool");
    assert_eq!(history.messages[3].role, "assistant");
}

