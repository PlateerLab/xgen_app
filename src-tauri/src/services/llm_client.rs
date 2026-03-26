//! Multi-Provider LLM API Client
//!
//! Supports: Anthropic (Claude), OpenAI-compatible (OpenAI/vLLM/SGL), Gemini
//! Uses XGEN backend's LLM provider configuration.
//! Streams responses via Tauri events for real-time UI updates.

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::error::{AppError, Result};
use crate::services::XgenApiClient;
use crate::services::xgen_api::LlmProviderConfig;
use crate::services::tool_search;

const MAX_TOOL_ROUNDS: usize = 5;

/// LLM API client — multi-provider support
pub struct LlmClient {
    client: reqwest::Client,
    config: LlmProviderConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: Value,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliStreamEvent {
    pub session_id: String,
    pub event_type: String,
    pub data: Value,
}

/// Provider type for dispatch
enum ProviderType {
    Anthropic,
    OpenAICompat, // openai, vllm, sgl
    Gemini,
}

impl LlmClient {
    pub fn from_config(config: LlmProviderConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            config,
        }
    }

    pub async fn from_xgen(
        xgen_api: &XgenApiClient,
        provider: Option<&str>,
        model: Option<&str>,
    ) -> Result<Self> {
        let config = xgen_api.get_llm_config(provider, model).await?;
        if config.api_key.is_empty() {
            return Err(AppError::LlmApi(format!(
                "XGEN 백엔드에 {} API 키가 설정되지 않았습니다.", config.provider
            )));
        }
        log::info!("CLI using provider: {} / model: {}", config.provider, config.model);
        Ok(Self::from_config(config))
    }

    fn provider_type(&self) -> ProviderType {
        match self.config.provider.as_str() {
            "anthropic" => ProviderType::Anthropic,
            "gemini" => ProviderType::Gemini,
            _ => ProviderType::OpenAICompat, // openai, vllm, sgl
        }
    }

    fn system_prompt() -> &'static str {
        r#"당신은 XGEN AI 플랫폼 어시스턴트입니다.
사용자의 요청에 따라 XGEN API를 호출하여 워크플로우 관리, 실행, 모니터링 등을 수행합니다.

역할:
- 워크플로우 목록 조회, 생성, 실행, 삭제
- 스케줄 생성 및 관리
- 노드/도구/LLM 상태 확인
- 문서 검색 및 RAG
- 사용자 질문에 친절하게 답변

tool 호출 결과를 사용자에게 보기 좋게 정리해서 한국어로 답변하세요.
JSON 결과는 핵심 정보만 추려서 읽기 쉽게 정리하세요.

주어진 tool 중에서 사용자 요청에 가장 적합한 것을 선택하세요.
적합한 tool이 없으면 tool을 호출하지 말고 직접 답변하세요."#
    }

    // ============================================================
    // Anthropic (Claude) API
    // ============================================================

    async fn call_anthropic_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[Value],
        session_id: &str,
        app: &AppHandle,
    ) -> Result<Value> {
        let body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": 4096,
            "system": Self::system_prompt(),
            "messages": messages,
            "tools": tools,
            "stream": true,
        });

        let resp = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::LlmApi(format!("Anthropic request failed: {}", e)))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::LlmApi(format!("Anthropic HTTP {} - {}", 0, text)));
        }

        self.parse_anthropic_sse(resp, session_id, app).await
    }

    async fn parse_anthropic_sse(
        &self,
        resp: reqwest::Response,
        session_id: &str,
        app: &AppHandle,
    ) -> Result<Value> {
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut content_blocks: Vec<Value> = Vec::new();
        let mut current_text = String::new();
        let mut current_block_is_text = false;
        let mut stop_reason: Option<String> = None;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| AppError::LlmApi(format!("Stream error: {}", e)))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buffer.find("\n\n") {
                let event_block = buffer[..pos].to_string();
                buffer = buffer[pos + 2..].to_string();

                for line in event_block.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" { continue; }
                        if let Ok(event) = serde_json::from_str::<Value>(data) {
                            match event["type"].as_str().unwrap_or("") {
                                "content_block_start" => {
                                    let block = &event["content_block"];
                                    if block["type"].as_str() == Some("tool_use") {
                                        current_block_is_text = false;
                                        content_blocks.push(serde_json::json!({
                                            "type": "tool_use",
                                            "id": block["id"],
                                            "name": block["name"],
                                            "input_json": "",
                                        }));
                                        let _ = app.emit("cli:event", CliStreamEvent {
                                            session_id: session_id.to_string(),
                                            event_type: "tool_call_start".into(),
                                            data: serde_json::json!({"name": block["name"]}),
                                        });
                                    } else {
                                        current_block_is_text = true;
                                        current_text.clear();
                                    }
                                }
                                "content_block_delta" => {
                                    let delta = &event["delta"];
                                    if delta["type"].as_str() == Some("text_delta") {
                                        let t = delta["text"].as_str().unwrap_or("");
                                        current_text.push_str(t);
                                        let _ = app.emit("cli:event", CliStreamEvent {
                                            session_id: session_id.to_string(),
                                            event_type: "token".into(),
                                            data: Value::String(t.to_string()),
                                        });
                                    } else if delta["type"].as_str() == Some("input_json_delta") {
                                        let part = delta["partial_json"].as_str().unwrap_or("");
                                        if let Some(last) = content_blocks.last_mut() {
                                            if let Some(s) = last.get("input_json").and_then(|v| v.as_str()) {
                                                last["input_json"] = Value::String(format!("{}{}", s, part));
                                            }
                                        }
                                    }
                                }
                                "content_block_stop" => {
                                    // Only push text block when the current block IS a text block
                                    if current_block_is_text && !current_text.is_empty() {
                                        content_blocks.push(serde_json::json!({
                                            "type": "text", "text": current_text.clone(),
                                        }));
                                        current_text.clear();
                                    }
                                    current_block_is_text = false;
                                }
                                "message_delta" => {
                                    if let Some(r) = event["delta"]["stop_reason"].as_str() {
                                        stop_reason = Some(r.to_string());
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        let content: Vec<Value> = content_blocks.iter().map(|b| {
            if b["type"].as_str() == Some("tool_use") {
                let input_json = b["input_json"].as_str().unwrap_or("{}");
                let input: Value = serde_json::from_str(input_json).unwrap_or(serde_json::json!({}));
                serde_json::json!({"type":"tool_use","id":b["id"],"name":b["name"],"input":input})
            } else { b.clone() }
        }).collect();

        Ok(serde_json::json!({"role":"assistant","content":content,"stop_reason":stop_reason}))
    }

    // ============================================================
    // OpenAI-compatible API (OpenAI, vLLM, SGL)
    // ============================================================

    fn openai_tools(tools: &[Value]) -> Vec<Value> {
        tools.iter().map(|t| serde_json::json!({
            "type": "function",
            "function": {
                "name": t["name"],
                "description": t["description"],
                "parameters": t["input_schema"],
            }
        })).collect()
    }

    fn messages_to_openai(messages: &[ChatMessage]) -> Vec<Value> {
        messages.iter().map(|m| {
            match m.content {
                Value::String(ref s) => serde_json::json!({"role": m.role, "content": s}),
                Value::Array(ref blocks) => {
                    // Convert Anthropic-style blocks to OpenAI format
                    let mut result = serde_json::json!({"role": m.role});

                    if m.role == "assistant" {
                        let mut text_parts = Vec::new();
                        let mut tool_calls = Vec::new();
                        for b in blocks {
                            match b["type"].as_str() {
                                Some("text") => text_parts.push(b["text"].as_str().unwrap_or("")),
                                Some("tool_use") => tool_calls.push(serde_json::json!({
                                    "id": b["id"],
                                    "type": "function",
                                    "function": {"name": b["name"], "arguments": b["input"].to_string()},
                                })),
                                _ => {}
                            }
                        }
                        if !text_parts.is_empty() {
                            result["content"] = Value::String(text_parts.join(""));
                        }
                        if !tool_calls.is_empty() {
                            result["tool_calls"] = Value::Array(tool_calls);
                        }
                        result
                    } else if m.role == "user" {
                        // tool_result blocks → separate tool messages
                        let has_tool_results = blocks.iter().any(|b| b["type"].as_str() == Some("tool_result"));
                        if has_tool_results {
                            // Return first tool result; caller handles multiple
                            if let Some(b) = blocks.first() {
                                return serde_json::json!({
                                    "role": "tool",
                                    "tool_call_id": b["tool_use_id"],
                                    "content": b["content"],
                                });
                            }
                        }
                        serde_json::json!({"role": "user", "content": serde_json::to_string(blocks).unwrap_or_default()})
                    } else {
                        serde_json::json!({"role": m.role, "content": serde_json::to_string(blocks).unwrap_or_default()})
                    }
                }
                _ => serde_json::json!({"role": m.role, "content": ""}),
            }
        }).collect()
    }

    async fn call_openai_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[Value],
        session_id: &str,
        app: &AppHandle,
    ) -> Result<Value> {
        let base_url = self.config.api_base_url.as_deref()
            .unwrap_or("https://api.openai.com");
        let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));

        let openai_messages = Self::messages_to_openai(messages);
        let openai_tools = Self::openai_tools(tools);

        let mut body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": 4096,
            "messages": [
                {"role": "system", "content": Self::system_prompt()},
            ],
            "stream": true,
        });
        // Append conversation messages
        if let Some(arr) = body["messages"].as_array_mut() {
            arr.extend(openai_messages);
        }
        if !openai_tools.is_empty() {
            body["tools"] = Value::Array(openai_tools);
        }

        let resp = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::LlmApi(format!("OpenAI request failed: {}", e)))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::LlmApi(format!("OpenAI HTTP error - {}", text)));
        }

        self.parse_openai_sse(resp, session_id, app).await
    }

    async fn parse_openai_sse(
        &self,
        resp: reqwest::Response,
        session_id: &str,
        app: &AppHandle,
    ) -> Result<Value> {
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut text_content = String::new();
        let mut tool_calls: Vec<Value> = Vec::new();
        let mut finish_reason: Option<String> = None;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| AppError::LlmApi(format!("Stream error: {}", e)))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buffer.find("\n") {
                let line = buffer[..pos].to_string();
                buffer = buffer[pos + 1..].to_string();

                if let Some(data) = line.strip_prefix("data: ") {
                    if data.trim() == "[DONE]" { continue; }
                    if let Ok(event) = serde_json::from_str::<Value>(data) {
                        if let Some(choice) = event["choices"].as_array().and_then(|c| c.first()) {
                            let delta = &choice["delta"];

                            // Text content
                            if let Some(t) = delta["content"].as_str() {
                                text_content.push_str(t);
                                let _ = app.emit("cli:event", CliStreamEvent {
                                    session_id: session_id.to_string(),
                                    event_type: "token".into(),
                                    data: Value::String(t.to_string()),
                                });
                            }

                            // Tool calls
                            if let Some(tcs) = delta["tool_calls"].as_array() {
                                for tc in tcs {
                                    let idx = tc["index"].as_u64().unwrap_or(0) as usize;
                                    while tool_calls.len() <= idx {
                                        tool_calls.push(serde_json::json!({
                                            "id": "", "name": "", "arguments": "",
                                        }));
                                    }
                                    if let Some(id) = tc["id"].as_str() {
                                        tool_calls[idx]["id"] = Value::String(id.to_string());
                                    }
                                    if let Some(func) = tc["function"].as_object() {
                                        if let Some(name) = func.get("name").and_then(|n| n.as_str()) {
                                            tool_calls[idx]["name"] = Value::String(name.to_string());
                                            let _ = app.emit("cli:event", CliStreamEvent {
                                                session_id: session_id.to_string(),
                                                event_type: "tool_call_start".into(),
                                                data: serde_json::json!({"name": name}),
                                            });
                                        }
                                        if let Some(args) = func.get("arguments").and_then(|a| a.as_str()) {
                                            let existing = tool_calls[idx]["arguments"].as_str().unwrap_or("");
                                            tool_calls[idx]["arguments"] = Value::String(format!("{}{}", existing, args));
                                        }
                                    }
                                }
                            }

                            if let Some(r) = choice["finish_reason"].as_str() {
                                finish_reason = Some(r.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Convert to Anthropic-style content blocks (unified format)
        let mut content: Vec<Value> = Vec::new();
        if !text_content.is_empty() {
            content.push(serde_json::json!({"type":"text","text": text_content}));
        }
        for tc in &tool_calls {
            let args = tc["arguments"].as_str().unwrap_or("{}");
            let input: Value = serde_json::from_str(args).unwrap_or(serde_json::json!({}));
            content.push(serde_json::json!({
                "type": "tool_use",
                "id": tc["id"],
                "name": tc["name"],
                "input": input,
            }));
        }

        let stop = match finish_reason.as_deref() {
            Some("tool_calls") => "tool_use",
            Some("stop") => "end_turn",
            _ => "end_turn",
        };

        Ok(serde_json::json!({"role":"assistant","content":content,"stop_reason":stop}))
    }

    // ============================================================
    // Gemini API (Google AI)
    // ============================================================

    async fn call_gemini_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[Value],
        session_id: &str,
        app: &AppHandle,
    ) -> Result<Value> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            self.config.model, self.config.api_key
        );

        // Convert messages to Gemini format
        let mut contents: Vec<Value> = Vec::new();
        for m in messages {
            let role = if m.role == "user" { "user" } else { "model" };
            let text = match &m.content {
                Value::String(s) => s.clone(),
                Value::Array(blocks) => {
                    blocks.iter()
                        .filter_map(|b| b["text"].as_str().or_else(|| b["content"].as_str()))
                        .collect::<Vec<_>>()
                        .join("\n")
                }
                _ => String::new(),
            };
            if !text.is_empty() {
                contents.push(serde_json::json!({
                    "role": role,
                    "parts": [{"text": text}],
                }));
            }
        }

        // Gemini tool definitions
        let gemini_tools = if !tools.is_empty() {
            let funcs: Vec<Value> = tools.iter().map(|t| serde_json::json!({
                "name": t["name"],
                "description": t["description"],
                "parameters": t["input_schema"],
            })).collect();
            Some(serde_json::json!([{"functionDeclarations": funcs}]))
        } else {
            None
        };

        let mut body = serde_json::json!({
            "system_instruction": {"parts": [{"text": Self::system_prompt()}]},
            "contents": contents,
        });
        if let Some(gt) = gemini_tools {
            body["tools"] = gt;
        }

        let resp = self.client
            .post(&url)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::LlmApi(format!("Gemini request failed: {}", e)))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::LlmApi(format!("Gemini HTTP error - {}", text)));
        }

        self.parse_gemini_sse(resp, session_id, app).await
    }

    async fn parse_gemini_sse(
        &self,
        resp: reqwest::Response,
        session_id: &str,
        app: &AppHandle,
    ) -> Result<Value> {
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut text_content = String::new();
        let mut tool_calls: Vec<Value> = Vec::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| AppError::LlmApi(format!("Stream error: {}", e)))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buffer.find("\n") {
                let line = buffer[..pos].to_string();
                buffer = buffer[pos + 1..].to_string();

                if let Some(data) = line.strip_prefix("data: ") {
                    if let Ok(event) = serde_json::from_str::<Value>(data) {
                        if let Some(candidates) = event["candidates"].as_array() {
                            for cand in candidates {
                                if let Some(parts) = cand["content"]["parts"].as_array() {
                                    for part in parts {
                                        if let Some(t) = part["text"].as_str() {
                                            text_content.push_str(t);
                                            let _ = app.emit("cli:event", CliStreamEvent {
                                                session_id: session_id.to_string(),
                                                event_type: "token".into(),
                                                data: Value::String(t.to_string()),
                                            });
                                        }
                                        if let Some(fc) = part.get("functionCall") {
                                            let name = fc["name"].as_str().unwrap_or("");
                                            let args = fc.get("args").cloned().unwrap_or_default();
                                            let id = format!("toolu_gemini_{}", tool_calls.len());
                                            tool_calls.push(serde_json::json!({
                                                "type": "tool_use",
                                                "id": id,
                                                "name": name,
                                                "input": args,
                                            }));
                                            let _ = app.emit("cli:event", CliStreamEvent {
                                                session_id: session_id.to_string(),
                                                event_type: "tool_call_start".into(),
                                                data: serde_json::json!({"name": name}),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut content: Vec<Value> = Vec::new();
        if !text_content.is_empty() {
            content.push(serde_json::json!({"type":"text","text": text_content}));
        }
        content.extend(tool_calls.iter().cloned());

        let stop = if tool_calls.is_empty() { "end_turn" } else { "tool_use" };

        Ok(serde_json::json!({"role":"assistant","content":content,"stop_reason":stop}))
    }

    // ============================================================
    // Non-streaming API call (for tests, no AppHandle needed)
    // ============================================================

    async fn call_anthropic_nostream(
        &self,
        messages: &[ChatMessage],
        tools: &[Value],
    ) -> Result<Value> {
        let body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": 4096,
            "system": Self::system_prompt(),
            "messages": messages,
            "tools": tools,
        });

        let resp = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::LlmApi(format!("Request failed: {}", e)))?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(AppError::LlmApi(format!("HTTP {} - {}", status.as_u16(), text)));
        }

        let response: Value = serde_json::from_str(&text)
            .map_err(|e| AppError::LlmApi(format!("Parse error: {}", e)))?;

        let stop_reason = response["stop_reason"].as_str().unwrap_or("end_turn");
        let content = response["content"].clone();

        Ok(serde_json::json!({"role":"assistant","content":content,"stop_reason":stop_reason}))
    }

    /// Non-streaming tool use loop (for testing without AppHandle)
    pub async fn send_with_tools_nostream(
        &self,
        messages: &mut Vec<ChatMessage>,
        xgen_api: &XgenApiClient,
    ) -> Result<String> {
        // 사용자 메시지에서 쿼리 추출하여 관련 tool 동적 검색
        let user_query = messages.last()
            .and_then(|m| m.content.as_str())
            .unwrap_or("help");

        let openapi_source = format!("{}/api/openapi", xgen_api.base_url());
        let tools = match tool_search::search_tools_for_llm(user_query, &openapi_source, Some(7)).await {
            Ok(t) if !t.is_empty() => {
                println!("  [tools] Found {} dynamic tools for '{}'", t.len(), user_query);
                t
            }
            Ok(_) | Err(_) => {
                println!("  [tools] Fallback to hardcoded tools");
                XgenApiClient::tool_definitions()
            }
        };
        let mut final_text = String::new();

        for round in 0..MAX_TOOL_ROUNDS {
            println!("[CLI test] round {}/{} ({})", round + 1, MAX_TOOL_ROUNDS, self.config.provider);

            let response = self.call_anthropic_nostream(messages, &tools).await?;

            let stop_reason = response["stop_reason"].as_str().unwrap_or("end_turn");
            let content = response["content"].as_array()
                .ok_or_else(|| AppError::LlmApi("Invalid response content".into()))?;

            messages.push(ChatMessage {
                role: "assistant".into(),
                content: Value::Array(content.clone()),
            });

            if stop_reason == "tool_use" {
                let mut tool_results: Vec<Value> = Vec::new();

                for block in content {
                    if block["type"].as_str() == Some("tool_use") {
                        let tool_id = block["id"].as_str().unwrap_or("");
                        let tool_name = block["name"].as_str().unwrap_or("");
                        let tool_input = block["input"].clone();

                        println!("  [tool] {} → {:?}", tool_name, tool_input);

                        // graph-tool-call call로 실행 (OpenAPI 기반 동적 실행)
                        let result = match tool_search::execute_tool_call(
                            tool_name,
                            &tool_input,
                            &openapi_source,
                            xgen_api.base_url(),
                            xgen_api.auth_token(),
                        ).await {
                            Ok(v) => {
                                let s = serde_json::to_string_pretty(&v).unwrap_or_default();
                                println!("  [result] {}...", s.chars().take(200).collect::<String>());
                                s
                            },
                            Err(e) => {
                                // fallback: xgen_api.execute_tool
                                println!("  [graph-tool-call fallback] {}", e);
                                match xgen_api.execute_tool(tool_name, tool_input.clone()).await {
                                    Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_default(),
                                    Err(e2) => format!("Error: {}", e2),
                                }
                            },
                        };

                        tool_results.push(serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": tool_id,
                            "content": result,
                        }));
                    }
                }

                messages.push(ChatMessage {
                    role: "user".into(),
                    content: Value::Array(tool_results),
                });
            } else {
                for block in content {
                    if block["type"].as_str() == Some("text") {
                        if let Some(text) = block["text"].as_str() {
                            final_text.push_str(text);
                        }
                    }
                }
                break;
            }
        }

        Ok(final_text)
    }

    // ============================================================
    // Unified streaming entry point
    // ============================================================

    async fn call_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[Value],
        session_id: &str,
        app: &AppHandle,
    ) -> Result<Value> {
        match self.provider_type() {
            ProviderType::Anthropic => self.call_anthropic_stream(messages, tools, session_id, app).await,
            ProviderType::OpenAICompat => self.call_openai_stream(messages, tools, session_id, app).await,
            ProviderType::Gemini => self.call_gemini_stream(messages, tools, session_id, app).await,
        }
    }

    /// Main entry: send message with tool use loop (unified across all providers)
    pub async fn send_with_tools(
        &self,
        messages: &mut Vec<ChatMessage>,
        xgen_api: &XgenApiClient,
        session_id: &str,
        app: &AppHandle,
    ) -> Result<String> {
        // 동적 tool 검색
        let user_query = messages.last()
            .and_then(|m| m.content.as_str())
            .unwrap_or("help");

        let openapi_source = format!("{}/api/openapi", xgen_api.base_url());
        let tools = match tool_search::search_tools_for_llm(user_query, &openapi_source, Some(7)).await {
            Ok(t) if !t.is_empty() => {
                log::info!("Found {} dynamic tools for '{}'", t.len(), user_query);
                t
            }
            Ok(_) | Err(_) => {
                log::warn!("Fallback to hardcoded tools");
                XgenApiClient::tool_definitions()
            }
        };
        let mut final_text = String::new();

        for round in 0..MAX_TOOL_ROUNDS {
            log::info!("CLI [{}] tool use round {}/{}", self.config.provider, round + 1, MAX_TOOL_ROUNDS);

            let response = self.call_stream(messages, &tools, session_id, app).await?;

            let stop_reason = response["stop_reason"].as_str().unwrap_or("end_turn");
            let content = response["content"].as_array()
                .ok_or_else(|| AppError::LlmApi("Invalid response content".into()))?;

            messages.push(ChatMessage {
                role: "assistant".into(),
                content: Value::Array(content.clone()),
            });

            if stop_reason == "tool_use" {
                let mut tool_results: Vec<Value> = Vec::new();

                for block in content {
                    if block["type"].as_str() == Some("tool_use") {
                        let tool_id = block["id"].as_str().unwrap_or("");
                        let tool_name = block["name"].as_str().unwrap_or("");
                        let tool_input = block["input"].clone();

                        log::info!("Executing tool: {}", tool_name);

                        let _ = app.emit("cli:event", CliStreamEvent {
                            session_id: session_id.to_string(),
                            event_type: "tool_call".into(),
                            data: serde_json::json!({"id":tool_id,"name":tool_name,"input":tool_input}),
                        });

                        let result = match tool_search::execute_tool_call(
                            tool_name,
                            &tool_input,
                            &openapi_source,
                            xgen_api.base_url(),
                            xgen_api.auth_token(),
                        ).await {
                            Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_default(),
                            Err(e) => {
                                log::warn!("graph-tool-call fallback: {}", e);
                                match xgen_api.execute_tool(tool_name, tool_input).await {
                                    Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_default(),
                                    Err(e2) => format!("Error: {}", e2),
                                }
                            }
                        };

                        let _ = app.emit("cli:event", CliStreamEvent {
                            session_id: session_id.to_string(),
                            event_type: "tool_result".into(),
                            data: serde_json::json!({"id":tool_id,"name":tool_name,"result_preview":result.chars().take(200).collect::<String>()}),
                        });

                        tool_results.push(serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": tool_id,
                            "content": result,
                        }));
                    }
                }

                // Validate: every tool_use must have a matching tool_result
                let tool_use_ids: Vec<String> = content.iter()
                    .filter(|b| b["type"].as_str() == Some("tool_use"))
                    .filter_map(|b| b["id"].as_str().map(|s| s.to_string()))
                    .collect();
                let tool_result_ids: Vec<String> = tool_results.iter()
                    .filter_map(|r| r["tool_use_id"].as_str().map(|s| s.to_string()))
                    .collect();
                log::info!("tool_use IDs: {:?}, tool_result IDs: {:?}", tool_use_ids, tool_result_ids);

                // Safety: add placeholder tool_result for any missing IDs
                for use_id in &tool_use_ids {
                    if !tool_result_ids.contains(use_id) {
                        log::warn!("Missing tool_result for tool_use_id: {}", use_id);
                        tool_results.push(serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": use_id,
                            "content": "Error: tool execution skipped",
                        }));
                    }
                }

                messages.push(ChatMessage {
                    role: "user".into(),
                    content: Value::Array(tool_results),
                });
            } else {
                for block in content {
                    if block["type"].as_str() == Some("text") {
                        if let Some(text) = block["text"].as_str() {
                            final_text.push_str(text);
                        }
                    }
                }
                break;
            }
        }

        let _ = app.emit("cli:event", CliStreamEvent {
            session_id: session_id.to_string(),
            event_type: "done".into(),
            data: Value::String(final_text.clone()),
        });

        Ok(final_text)
    }
}
