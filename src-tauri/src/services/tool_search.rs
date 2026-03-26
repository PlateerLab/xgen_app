//! Tool Search Service — Gateway Mode
//!
//! graph-tool-call 바이너리(sidecar)를 통해 OpenAPI spec 기반
//! API tool 검색 및 실행을 제공합니다.
//!
//! Gateway Mode:
//! - LLM에 search_tools + call_tool 2개 meta-tool만 제공
//! - LLM이 직접 검색 쿼리를 생성하고 tool을 선택/호출
//! - translate_query 같은 하드코딩 불필요

use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::{AppError, Result};

const DEFAULT_TOP_K: usize = 7;

/// API path prefix → 프론트엔드 페이지 매핑
fn get_page_for_api(api_path: &str) -> Option<&'static str> {
    let mappings: &[(&str, &str)] = &[
        // Workflow
        ("/api/workflow/list", "/main?view=workflows"),
        ("/api/workflow/execute", "/main?view=workflows"),
        ("/api/workflow/store", "/main?view=workflows"),
        ("/api/workflow/canvas", "/main?view=canvas"),
        ("/api/workflow/trace", "/main?view=workflows"),
        ("/api/workflow/schedule", "/main?view=workflows"),
        // Chat
        ("/api/chat", "/main?view=new-chat"),
        ("/api/interaction", "/main?view=new-chat"),
        // LLM / Config / Admin
        ("/api/llm", "/admin?view=dashboard"),
        ("/api/config", "/admin?view=dashboard"),
        ("/api/admin", "/admin?view=dashboard"),
        // Node / Tools
        ("/api/node", "/main?view=workflows"),
        ("/api/tools", "/main?view=workflows"),
        // Documents / RAG
        ("/api/documents", "/main?view=documents"),
        ("/api/retrieval", "/main?view=documents"),
        ("/api/embedding", "/admin?view=dashboard"),
        // Prompt
        ("/api/prompt", "/main?view=workflows"),
        // Model
        ("/api/model", "/modelOps?view=train-monitor"),
        // Service Request
        ("/api/service-request", "/main?view=service-request"),
        // Support
        ("/api/support", "/support?view=inquiry"),
        // Main
        ("/api/dashboard", "/main?view=main-dashboard"),
    ];

    for (prefix, page) in mappings {
        if api_path.starts_with(prefix) {
            return Some(page);
        }
    }
    None
}

// ============================================================
// Meta-tool definitions (LLM에 제공할 고정 tool 2개)
// ============================================================

/// Gateway meta-tool 정의 반환 (search_tools + call_tool)
pub fn meta_tool_definitions() -> Vec<Value> {
    vec![
        serde_json::json!({
            "name": "search_tools",
            "description": "Search available XGEN API tools by keyword. Returns tool names, descriptions, HTTP methods, paths, and parameter schemas. Use English keywords for best results. Always search first before calling a tool.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "English search keywords (e.g. 'execute workflow', 'list agents', 'create schedule', 'LLM status')"
                    },
                    "top_k": {
                        "type": "integer",
                        "description": "Number of results (default: 7)"
                    }
                },
                "required": ["query"]
            }
        }),
        serde_json::json!({
            "name": "call_tool",
            "description": "Execute an API tool found via search_tools. Pass the exact tool_name from search results and matching arguments.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "tool_name": {
                        "type": "string",
                        "description": "Exact tool name from search_tools results"
                    },
                    "arguments": {
                        "type": "object",
                        "description": "Tool arguments matching the parameter schema from search results"
                    }
                },
                "required": ["tool_name"]
            }
        }),
        serde_json::json!({
            "name": "navigate",
            "description": "Navigate the main XGEN window to a page. Only use when the user explicitly asks to go somewhere.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Page path (e.g. '/main?view=workflows', '/admin?view=dashboard', '/main?view=canvas')"
                    }
                },
                "required": ["path"]
            }
        }),
        // ============================================================
        // Canvas tools — 캔버스 워크플로우 조작 (프론트엔드에서 실행)
        // ============================================================
        serde_json::json!({
            "name": "canvas_get_nodes",
            "description": "Get the list of all nodes currently on the canvas with their types, positions, and parameter values.",
            "input_schema": { "type": "object", "properties": {} }
        }),
        serde_json::json!({
            "name": "canvas_get_available_nodes",
            "description": "Get all available node types that can be added to the canvas, grouped by category (agents, chat_models, document_loaders, mcp, tools, etc.).",
            "input_schema": {
                "type": "object",
                "properties": {
                    "category": {
                        "type": "string",
                        "description": "Optional category filter (e.g. 'agents', 'document_loaders', 'mcp', 'tools')"
                    }
                }
            }
        }),
        serde_json::json!({
            "name": "canvas_add_node",
            "description": "Add a new node to the canvas. The node will be auto-positioned if position is not specified.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "node_type": {
                        "type": "string",
                        "description": "Node type ID from canvas_get_available_nodes (e.g. 'agents/xgen', 'document_loaders/Qdrant', 'tools/input_string', 'tools/print_agent_output')"
                    },
                    "position": {
                        "type": "object",
                        "description": "Optional {x, y} position on canvas",
                        "properties": { "x": {"type":"number"}, "y": {"type":"number"} }
                    }
                },
                "required": ["node_type"]
            }
        }),
        serde_json::json!({
            "name": "canvas_remove_node",
            "description": "Remove a node from the canvas by its ID.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "node_id": { "type": "string", "description": "Node ID to remove" }
                },
                "required": ["node_id"]
            }
        }),
        serde_json::json!({
            "name": "canvas_connect",
            "description": "Connect two nodes by their ports. Creates an edge from source output to target input.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "source_node": { "type": "string", "description": "Source node ID" },
                    "source_port": { "type": "string", "description": "Source output port name" },
                    "target_node": { "type": "string", "description": "Target node ID" },
                    "target_port": { "type": "string", "description": "Target input port name" }
                },
                "required": ["source_node", "source_port", "target_node", "target_port"]
            }
        }),
        serde_json::json!({
            "name": "canvas_disconnect",
            "description": "Remove an edge (connection) between nodes.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "edge_id": { "type": "string", "description": "Edge ID to remove" }
                },
                "required": ["edge_id"]
            }
        }),
        serde_json::json!({
            "name": "canvas_update_node_param",
            "description": "Update a parameter value of a node on the canvas (e.g. set collection name on RAG node, change LLM model).",
            "input_schema": {
                "type": "object",
                "properties": {
                    "node_id": { "type": "string", "description": "Target node ID" },
                    "param_name": { "type": "string", "description": "Parameter name to update" },
                    "value": { "description": "New value for the parameter" }
                },
                "required": ["node_id", "param_name", "value"]
            }
        }),
        serde_json::json!({
            "name": "canvas_save",
            "description": "Save the current workflow to the server.",
            "input_schema": { "type": "object", "properties": {} }
        }),
    ]
}

// ============================================================
// search_tools meta-tool 실행
// ============================================================

/// search_tools: 쿼리로 API tool을 검색하고 상세 정보를 텍스트로 반환
pub async fn search_tools_text(
    query: &str,
    openapi_source: &str,
    top_k: Option<usize>,
) -> Result<String> {
    let bin = find_binary()?;
    let k = top_k.unwrap_or(DEFAULT_TOP_K);

    log::info!("search_tools: query='{}', top_k={}", query, k);

    // graph 빌드 (캐싱)
    let graph_path = ensure_graph(&bin, openapi_source).await?;

    // search 실행
    let tool_names = search_tool_names(&bin, query, openapi_source, k).await?;

    if tool_names.is_empty() {
        return Ok("No tools found. Try different English keywords.".into());
    }

    // graph에서 tool 상세 정보 로드
    let graph_tools = load_graph_tools(&graph_path)?;

    // LLM이 읽을 수 있는 텍스트로 포매팅
    let mut lines = Vec::new();
    lines.push(format!("Found {} tools for \"{}\":\n", tool_names.len(), query));

    for (i, name) in tool_names.iter().enumerate() {
        if let Some(tool) = graph_tools.get(name) {
            let desc = tool["description"].as_str().unwrap_or("");
            let method = tool["metadata"]["method"].as_str().unwrap_or("get").to_uppercase();
            let path = tool["metadata"]["path"].as_str().unwrap_or("");

            lines.push(format!("{}. {}", i + 1, name));
            lines.push(format!("   {} {}", method, path));
            lines.push(format!("   {}", desc));

            // Related frontend page
            if let Some(page) = get_page_for_api(path) {
                lines.push(format!("   📄 Related page: {}", page));
            }

            // Parameters
            if let Some(params) = tool["parameters"].as_array() {
                if !params.is_empty() {
                    lines.push("   Parameters:".to_string());
                    for p in params {
                        let pname = p["name"].as_str().unwrap_or("?");
                        let ptype = p["type"].as_str().unwrap_or("string");
                        let required = if p["required"].as_bool().unwrap_or(false) { ", REQUIRED" } else { "" };
                        let pdesc = p["description"].as_str().unwrap_or("");
                        let location = p["in"].as_str().unwrap_or("");

                        let mut detail = format!("     - {} ({}{})", pname, ptype, required);
                        if !location.is_empty() {
                            detail.push_str(&format!(" [in: {}]", location));
                        }
                        if !pdesc.is_empty() {
                            detail.push_str(&format!(" — {}", pdesc));
                        }
                        // enum values
                        if let Some(enums) = p["enum"].as_array() {
                            let vals: Vec<&str> = enums.iter().filter_map(|v| v.as_str()).collect();
                            if !vals.is_empty() {
                                detail.push_str(&format!(" [enum: {}]", vals.join(", ")));
                            }
                        }
                        lines.push(detail);
                    }
                }
            }
            lines.push(String::new()); // blank line between tools
        }
    }

    Ok(lines.join("\n"))
}

// ============================================================
// call_tool meta-tool 실행 (기존 execute_tool_call)
// ============================================================

/// call_tool: graph-tool-call call로 API 직접 실행
pub async fn execute_tool_call(
    tool_name: &str,
    args: &Value,
    openapi_source: &str,
    base_url: &str,
    auth_token: Option<&str>,
) -> Result<Value> {
    let bin = find_binary()?;
    let args_str = serde_json::to_string(args).unwrap_or_default();

    log::info!("call_tool: {} args={}", tool_name, &args_str.chars().take(100).collect::<String>());

    let mut cmd = tokio::process::Command::new(&bin);
    cmd.args([
        "call", tool_name,
        "-s", openapi_source,
        "--base-url", base_url,
        "--tool", tool_name,
        "--allow-private-hosts",
    ]);

    if let Some(token) = auth_token {
        log::info!("call_tool: auth token present ({}...)", &token[..token.len().min(20)]);
        cmd.args(["--auth-token", token]);
    } else {
        log::warn!("call_tool: NO auth token — API calls requiring auth will fail");
    }

    if !args_str.is_empty() && args_str != "{}" && args_str != "null" {
        cmd.args(["--args", &args_str]);
    }

    let output = cmd.output().await
        .map_err(|e| AppError::Cli(format!("call failed: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Cli(format!("call failed: {}\n{}", stderr, stdout)));
    }

    // graph-tool-call call 출력: JSON (status, headers, body)
    match serde_json::from_str::<Value>(&stdout) {
        Ok(v) => Ok(v.get("body").cloned().unwrap_or(v)),
        Err(_) => Ok(Value::String(stdout.to_string())),
    }
}

// ============================================================
// Internal helpers
// ============================================================

/// graph-tool-call sidecar 경로 찾기
fn find_binary() -> Result<String> {
    // 1. 환경변수 (개발/테스트용)
    if let Ok(path) = std::env::var("GRAPH_TOOL_CALL_BIN") {
        if std::path::Path::new(&path).exists() {
            return Ok(path);
        }
    }

    // 2. Tauri 번들 sidecar (externalBin)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for name in &[
                "graph-tool-call",
                "graph-tool-call.exe",
                &format!("graph-tool-call-{}", std::env::consts::ARCH),
            ] {
                let p = dir.join(name);
                if p.exists() {
                    return Ok(p.to_string_lossy().to_string());
                }
            }
        }
    }

    // 3. 시스템 PATH (개발 환경 fallback)
    for cmd in &["which", "where"] {
        if let Ok(output) = std::process::Command::new(cmd).arg("graph-tool-call").output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).lines().next().unwrap_or("").trim().to_string();
                if !path.is_empty() && std::path::Path::new(&path).exists() {
                    return Ok(path);
                }
            }
        }
    }

    Err(AppError::Cli("graph-tool-call 바이너리를 찾을 수 없습니다. 앱을 재설치하세요.".into()))
}

/// graph 캐시 파일 경로
fn graph_cache_path(source: &str) -> PathBuf {
    let hash = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        source.hash(&mut h);
        h.finish()
    };
    std::env::temp_dir().join(format!("xgen-graph-{}.json", hash))
}

/// graph-tool-call ingest 실행 (5분 캐싱)
async fn ensure_graph(bin: &str, source: &str) -> Result<PathBuf> {
    let path = graph_cache_path(source);

    if path.exists() {
        if let Ok(meta) = std::fs::metadata(&path) {
            if let Ok(modified) = meta.modified() {
                if modified.elapsed().unwrap_or_default().as_secs() < 300 {
                    return Ok(path);
                }
            }
        }
    }

    log::info!("Building tool graph from {}", source);
    let output = tokio::process::Command::new(bin)
        .args(["ingest", source, "-o", &path.to_string_lossy(), "-q", "--allow-private-hosts"])
        .output()
        .await
        .map_err(|e| AppError::Cli(format!("graph-tool-call ingest failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Cli(format!("ingest failed: {}", stderr)));
    }

    Ok(path)
}

/// graph JSON에서 tool 정보 로드
fn load_graph_tools(graph_path: &PathBuf) -> Result<HashMap<String, Value>> {
    let content = std::fs::read_to_string(graph_path)
        .map_err(|e| AppError::Cli(format!("Failed to read graph: {}", e)))?;
    let graph: Value = serde_json::from_str(&content)
        .map_err(|e| AppError::Cli(format!("Failed to parse graph: {}", e)))?;

    let tools = graph["tools"].as_object()
        .ok_or_else(|| AppError::Cli("Invalid graph: no tools".into()))?;

    let mut result = HashMap::new();
    for (name, tool) in tools {
        result.insert(name.clone(), tool.clone());
    }
    Ok(result)
}

/// graph-tool-call search 실행 → tool 이름 목록 반환
async fn search_tool_names(bin: &str, query: &str, source: &str, top_k: usize) -> Result<Vec<String>> {
    let output = tokio::process::Command::new(bin)
        .args(["search", query, "--source", source, "--top-k", &top_k.to_string(), "--allow-private-hosts"])
        .output()
        .await
        .map_err(|e| AppError::Cli(format!("search failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Cli(format!("search failed: {}", stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut names = Vec::new();

    for line in stdout.lines() {
        let trimmed = line.trim();
        // "  1. tool_name_here" 패턴
        if trimmed.starts_with(|c: char| c.is_ascii_digit()) {
            if let Some(name) = trimmed.split(". ").nth(1) {
                names.push(name.trim().to_string());
            }
        }
    }

    Ok(names)
}
