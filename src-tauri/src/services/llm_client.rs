//! Multi-Provider LLM API Client
//!
//! Supports: Anthropic (Claude), OpenAI-compatible (OpenAI/vLLM/SGL), Gemini
//! Uses XGEN backend's LLM provider configuration.
//! Streams responses via Tauri events for real-time UI updates.

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Listener};

use crate::error::{AppError, Result};
use crate::services::XgenApiClient;
use crate::services::xgen_api::LlmProviderConfig;
use crate::services::tool_search;

const MAX_TOOL_ROUNDS: usize = 10;

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
XGEN은 노코드 AI 워크플로우 빌더 플랫폼으로, 캔버스에서 노드를 배치/연결하여 AI 파이프라인을 구성합니다.

[XGEN 플랫폼 구조]
- 워크플로우: 노드+엣지로 구성하는 AI 파이프라인. CRUD → 실행 → 배포 → 스케줄.
- 노드 종류: Agent(LLM 에이전트), MCP(외부도구 17종), RAG(벡터DB 검색), Memory(대화기록), Input/Output, Router(분기)
- 컬렉션: 벡터DB의 문서 컬렉션. 문서 업로드 → 청킹 → 임베딩 → 검색.
- 도구(Tool Storage): 외부 API를 등록한 HTTP 호출 정의.

[페이지 경로]
- /main?view=main-dashboard  대시보드
- /main?view=workflows       워크플로우 목록
- /main?view=new-chat        새 채팅
- /main?view=documents       지식 컬렉션(문서/벡터DB)
- /main?view=tool-storage    도구 저장소
- /canvas                    새 캔버스
- /canvas?load=workflow_id          기존 워크플로우 편집
- /admin?view=dashboard      관리자

[도구 사용 규칙]

API 검색/호출:
1. search_tools — XGEN API를 영문 키워드로 검색. 항상 먼저 검색 후 호출.
2. call_tool — 검색된 API 실행. tool_name과 arguments를 search 결과에서 정확히 가져올 것.

캔버스 조작 (캔버스 페이지에서만):
3. canvas_get_nodes — 현재 캔버스의 노드 목록
4. canvas_get_available_nodes — 추가 가능한 노드 타입 (category 필터 가능)
5. canvas_add_node — 노드 추가 (node_type 필수, 예: 'agents/xgen', 'tools/input_string')
6. canvas_remove_node — 노드 삭제
7. canvas_connect — 두 노드의 포트 연결
8. canvas_update_node_param — 노드 파라미터 변경
9. canvas_save — 워크플로우 저장

페이지 이동:
10. navigate — 사용자가 명시적으로 요청할 때만 사용

[핵심 행동 규칙]
- "목록 보여줘", "상태 알려줘" → search_tools + call_tool로 API 호출 → 텍스트로 정리. navigate 아님!
- "페이지로 가줘", "열어줘" → navigate
- "캔버스 열어줘" → navigate('/canvas') (새 캔버스) 또는 navigate('/canvas?load=workflow_id') (기존 워크플로우)
- 워크플로우를 열려면 먼저 call_tool로 목록을 조회해서 workflow_id를 확인한 후 navigate. 이름이 아니라 ID를 사용!
- "노드 추가해줘" → canvas_add_node (캔버스 페이지에서만)
- 검색 쿼리는 반드시 영문: "워크플로우 목록" → search_tools("list workflows")
- JSON 원본을 그대로 보여주지 말고 핵심만 한국어로 정리
- 캔버스를 벗어나지 않고 작업하는 것이 기본

[워크플로우 구성 패턴]
기본 RAG: Input String → Agent Xgen(+ Qdrant Search + DB Memory) → Print Any (Stream)
도구 에이전트: Input String → Agent Xgen(+ MCP Tools) → Print Any (Stream)
조건 분기: Agent → Router → 분기별 Agent → Print Any"#
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
        let openapi_source = format!("{}/api/openapi", xgen_api.base_url());

        // Gateway mode: 고정 meta-tool 2개
        let tools = tool_search::meta_tool_definitions();
        let mut final_text = String::new();

        for round in 0..MAX_TOOL_ROUNDS {
            println!("[CLI test] gateway round {}/{} ({})", round + 1, MAX_TOOL_ROUNDS, self.config.provider);

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

                        println!("  [gateway] {} → {:?}", tool_name, tool_input);

                        let result = match tool_name {
                            "search_tools" => {
                                let query = tool_input["query"].as_str().unwrap_or("help");
                                let top_k = tool_input["top_k"].as_u64().map(|v| v as usize);
                                match tool_search::search_tools_text(query, &openapi_source, top_k).await {
                                    Ok(text) => {
                                        println!("  [search] {}...", text.chars().take(200).collect::<String>());
                                        text
                                    }
                                    Err(e) => format!("Search error: {}", e),
                                }
                            }
                            "call_tool" => {
                                let actual_tool = tool_input["tool_name"].as_str().unwrap_or("");
                                let args = tool_input.get("arguments").cloned().unwrap_or(serde_json::json!({}));
                                match tool_search::execute_tool_call(
                                    actual_tool, &args, &openapi_source,
                                    xgen_api.base_url(), xgen_api.auth_token(),
                                ).await {
                                    Ok(v) => {
                                        let s = serde_json::to_string_pretty(&v).unwrap_or_default();
                                        println!("  [call] {}...", s.chars().take(200).collect::<String>());
                                        s
                                    }
                                    Err(e) => format!("Call error: {}", e),
                                }
                            }
                            "navigate" => {
                                let path = tool_input["path"].as_str().unwrap_or("/");
                                println!("  [navigate] {}", path);
                                format!("Navigated to {}", path)
                            }
                            _ => format!("Unknown tool: {}", tool_name),
                        };

                        // Compress large tool results (JSON/HTML/text type-aware)
                        let compressed = tool_search::compress_tool_result(&result);

                        tool_results.push(serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": tool_id,
                            "content": compressed,
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
        let openapi_source = format!("{}/api/openapi", xgen_api.base_url());

        // Gateway mode: 고정 meta-tool 2개 (search_tools + call_tool)
        let tools = tool_search::meta_tool_definitions();
        let mut final_text = String::new();

        for round in 0..MAX_TOOL_ROUNDS {
            log::info!("CLI [{}] gateway round {}/{}", self.config.provider, round + 1, MAX_TOOL_ROUNDS);

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

                        log::info!("Gateway dispatch: {} (id: {})", tool_name, tool_id);

                        let _ = app.emit("cli:event", CliStreamEvent {
                            session_id: session_id.to_string(),
                            event_type: "tool_call".into(),
                            data: serde_json::json!({"id":tool_id,"name":tool_name,"input":tool_input}),
                        });

                        // Meta-tool dispatch
                        let result = match tool_name {
                            "search_tools" => {
                                let query = tool_input["query"].as_str().unwrap_or("help");
                                let top_k = tool_input["top_k"].as_u64().map(|v| v as usize);
                                match tool_search::search_tools_text(query, &openapi_source, top_k).await {
                                    Ok(text) => text,
                                    Err(e) => format!("Search error: {}", e),
                                }
                            }
                            "call_tool" => {
                                let actual_tool = tool_input["tool_name"].as_str().unwrap_or("");
                                let args = tool_input.get("arguments").cloned().unwrap_or(serde_json::json!({}));
                                match tool_search::execute_tool_call(
                                    actual_tool, &args, &openapi_source,
                                    xgen_api.base_url(), xgen_api.auth_token(),
                                ).await {
                                    Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_default(),
                                    Err(e) => format!("Call error: {}", e),
                                }
                            }
                            "navigate" => {
                                let path = tool_input["path"].as_str().unwrap_or("/");
                                log::info!("navigate: {}", path);
                                let _ = app.emit_to("main", "navigate", serde_json::json!({"path": path}));
                                format!("Navigated to {}", path)
                            }
                            // Canvas tools — 프론트엔드로 이벤트 전달 후 결과 대기
                            name if name.starts_with("canvas_") => {
                                let request_id = uuid::Uuid::new_v4().to_string();
                                log::info!("canvas command: {} (req: {})", name, request_id);

                                // 메인 윈도우에 canvas 명령 전달
                                let _ = app.emit_to("main", "canvas:command", serde_json::json!({
                                    "requestId": request_id,
                                    "action": name.strip_prefix("canvas_").unwrap_or(name),
                                    "params": tool_input,
                                }));

                                // 결과 대기 (oneshot channel)
                                let (tx, rx) = tokio::sync::oneshot::channel::<String>();
                                let tx = std::sync::Arc::new(tokio::sync::Mutex::new(Some(tx)));
                                let req_id_clone = request_id.clone();
                                let tx_clone = tx.clone();

                                // 결과 이벤트 리스너 등록
                                let handler = app.listen("canvas:result", move |event: tauri::Event| {
                                    let payload_str = event.payload();
                                    if let Ok(payload) = serde_json::from_str::<serde_json::Value>(payload_str) {
                                        if payload["requestId"].as_str() == Some(req_id_clone.as_str()) {
                                            let result = payload["result"].to_string();
                                            if let Ok(mut guard) = tx_clone.try_lock() {
                                                if let Some(sender) = guard.take() {
                                                    let _ = sender.send(result);
                                                }
                                            }
                                        }
                                    }
                                });

                                // 5초 타임아웃으로 결과 대기
                                let canvas_result = match tokio::time::timeout(
                                    std::time::Duration::from_secs(5), rx
                                ).await {
                                    Ok(Ok(result)) => result,
                                    Ok(Err(_)) => "Error: canvas response channel closed".to_string(),
                                    Err(_) => "Error: canvas command timed out (5s)".to_string(),
                                };

                                app.unlisten(handler);
                                canvas_result
                            }
                            _ => format!("Unknown tool: {}", tool_name),
                        };

                        let _ = app.emit("cli:event", CliStreamEvent {
                            session_id: session_id.to_string(),
                            event_type: "tool_result".into(),
                            data: serde_json::json!({"id":tool_id,"name":tool_name,"result_preview":result.chars().take(200).collect::<String>()}),
                        });

                        // Compress large tool results (JSON/HTML/text type-aware)
                        let compressed = tool_search::compress_tool_result(&result);

                        tool_results.push(serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": tool_id,
                            "content": compressed,
                        }));
                    }
                }

                // Safety: ensure all tool_use IDs have matching tool_results
                let tool_use_ids: Vec<String> = content.iter()
                    .filter(|b| b["type"].as_str() == Some("tool_use"))
                    .filter_map(|b| b["id"].as_str().map(|s| s.to_string()))
                    .collect();
                let tool_result_ids: Vec<String> = tool_results.iter()
                    .filter_map(|r| r["tool_use_id"].as_str().map(|s| s.to_string()))
                    .collect();

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
