//! CLI Integration Test
//!
//! Tests XGEN API client and LLM config fetching against live backend.
//! Run: cargo test --test cli_integration -- --nocapture

use serde_json::Value;

const XGEN_BASE_URL: &str = "https://xgen.x2bee.com";
const XGEN_TOKEN: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiI1IiwidXNlcm5hbWUiOiLshpDshLHspIAiLCJpc19hZG1pbiI6dHJ1ZSwiZXhwIjoxNzc0NDUxNzUzLCJ0eXBlIjoiYWNjZXNzIn0.UrrKOGkjNJA_XDPHFLt-xTfw3ruxXQ1f2SeKumfHn-M";

#[tokio::test]
async fn test_xgen_api_workflow_list() {
    let client = app_lib::services::XgenApiClient::new(
        XGEN_BASE_URL.to_string(),
        Some(XGEN_TOKEN.to_string()),
    );

    let result = client.list_workflows().await;
    println!("=== Workflow List ===");
    match &result {
        Ok(v) => {
            let workflows = v["workflows"].as_array();
            println!("Found {} workflows", workflows.map(|w| w.len()).unwrap_or(0));
            if let Some(wfs) = workflows {
                for w in wfs.iter().take(3) {
                    println!("  - {} ({})",
                        w["workflow_name"].as_str().unwrap_or("?"),
                        w["workflow_id"].as_str().unwrap_or("?"));
                }
            }
        }
        Err(e) => println!("Error: {}", e),
    }
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_xgen_api_llm_status() {
    let client = app_lib::services::XgenApiClient::new(
        XGEN_BASE_URL.to_string(),
        Some(XGEN_TOKEN.to_string()),
    );

    let result = client.get_llm_status().await;
    println!("=== LLM Status ===");
    match &result {
        Ok(v) => {
            println!("Provider: {}", v["current_provider"].as_str().unwrap_or("?"));
            println!("Available: {:?}", v["available_providers"]);
        }
        Err(e) => println!("Error: {}", e),
    }
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_xgen_api_llm_config_default() {
    let client = app_lib::services::XgenApiClient::new(
        XGEN_BASE_URL.to_string(),
        Some(XGEN_TOKEN.to_string()),
    );

    let result = client.get_llm_config(None, None).await;
    println!("=== LLM Config (default=anthropic) ===");
    match &result {
        Ok(config) => {
            println!("Provider: {}", config.provider);
            println!("Model: {}", config.model);
            println!("API Key: {}...", &config.api_key[..15.min(config.api_key.len())]);
            assert_eq!(config.provider, "anthropic");
            assert!(!config.api_key.is_empty());
        }
        Err(e) => println!("Error: {}", e),
    }
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_xgen_api_llm_config_per_provider() {
    let client = app_lib::services::XgenApiClient::new(
        XGEN_BASE_URL.to_string(),
        Some(XGEN_TOKEN.to_string()),
    );

    for provider in &["anthropic", "openai", "gemini"] {
        let result = client.get_llm_config(Some(provider), None).await;
        println!("=== {} ===", provider);
        match &result {
            Ok(config) => {
                println!("  Model: {}", config.model);
                println!("  Has Key: {}", !config.api_key.is_empty());
            }
            Err(e) => println!("  Error: {}", e),
        }
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_list_available_providers() {
    let client = app_lib::services::XgenApiClient::new(
        XGEN_BASE_URL.to_string(),
        Some(XGEN_TOKEN.to_string()),
    );

    let result = client.list_available_providers().await;
    println!("=== Available Providers ===");
    match &result {
        Ok(providers) => {
            for p in providers {
                println!("  {} (model: {}, configured: {}, available: {})",
                    p.name, p.model, p.configured, p.available);
            }
            assert!(!providers.is_empty());
        }
        Err(e) => println!("Error: {}", e),
    }
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_llm_client_from_xgen() {
    let xgen = app_lib::services::XgenApiClient::new(
        XGEN_BASE_URL.to_string(),
        Some(XGEN_TOKEN.to_string()),
    );

    // Test default (anthropic)
    let result = app_lib::services::LlmClient::from_xgen(&xgen, None, None).await;
    println!("=== LLM Client (default) ===");
    assert!(result.is_ok(), "Default LLM client should init OK");

    // Test with specific provider
    let result = app_lib::services::LlmClient::from_xgen(&xgen, Some("openai"), None).await;
    println!("=== LLM Client (openai) ===");
    assert!(result.is_ok(), "OpenAI LLM client should init OK");
}

#[tokio::test]
async fn test_tool_definitions() {
    let tools = app_lib::services::XgenApiClient::tool_definitions();
    println!("=== Tool Definitions ===");
    println!("Total tools: {}", tools.len());
    for tool in &tools {
        println!("  - {}: {}",
            tool["name"].as_str().unwrap_or("?"),
            tool["description"].as_str().unwrap_or("?"));
    }
    assert!(tools.len() >= 5, "Should have at least 5 tools");
}
