//! Tool Search Service
//!
//! graph-tool-call 바이너리(sidecar)를 사용하여 OpenAPI spec에서
//! 사용자 쿼리에 관련된 tool을 검색하고, LLM tool_use 형식으로 변환합니다.
//!
//! Flow:
//! 1. graph-tool-call ingest → graph JSON 빌드 (캐싱)
//! 2. graph-tool-call search → 관련 tool 이름 목록
//! 3. graph JSON에서 해당 tool의 parameters 추출
//! 4. Claude/OpenAI tool_use 형식으로 변환하여 반환

use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::{AppError, Result};

const DEFAULT_TOP_K: usize = 5;

/// graph-tool-call sidecar 경로 찾기
fn find_binary() -> Result<String> {
    // 1. 환경변수
    if let Ok(path) = std::env::var("GRAPH_TOOL_CALL_BIN") {
        if std::path::Path::new(&path).exists() {
            return Ok(path);
        }
    }

    // 2. 실행 파일 옆 (Tauri 번들)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for name in &["graph-tool-call", "graph-tool-call.exe", "graph-tool-call-bin", "graph-tool-call-bin.exe"] {
                let p = dir.join(name);
                if p.exists() {
                    return Ok(p.to_string_lossy().to_string());
                }
            }
        }
    }

    // 3. 시스템 PATH
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

    Err(AppError::Cli("graph-tool-call 바이너리를 찾을 수 없습니다".into()))
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

/// graph-tool-call ingest 실행 (캐싱)
async fn ensure_graph(bin: &str, source: &str) -> Result<PathBuf> {
    let path = graph_cache_path(source);

    // 5분 이내 캐시면 재사용
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

/// graph tool → Claude tool_use 형식으로 변환
fn to_llm_tool_schema(tool: &Value) -> Value {
    let name = tool["name"].as_str().unwrap_or("unknown");
    let desc = tool["description"].as_str().unwrap_or("");
    let method = tool["metadata"]["method"].as_str().unwrap_or("get");
    let path = tool["metadata"]["path"].as_str().unwrap_or("");

    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    if let Some(params) = tool["parameters"].as_array() {
        for param in params {
            let param_name = param["name"].as_str().unwrap_or("").to_string();
            let param_type = param["type"].as_str().unwrap_or("string");
            let param_desc = param["description"].as_str().unwrap_or("");
            let is_required = param["required"].as_bool().unwrap_or(false);

            let mut prop = serde_json::json!({
                "type": param_type,
                "description": param_desc,
            });

            if let Some(enum_values) = param["enum"].as_array() {
                prop["enum"] = Value::Array(enum_values.clone());
            }

            properties.insert(param_name.clone(), prop);
            if is_required {
                required.push(Value::String(param_name));
            }
        }
    }

    serde_json::json!({
        "name": name,
        "description": format!("{} [{} {}]", desc, method.to_uppercase(), path),
        "input_schema": {
            "type": "object",
            "properties": properties,
            "required": required,
        }
    })
}

/// 한국어 키워드를 영문 검색어로 변환 (graph-tool-call은 영문 검색이 정확)
fn translate_query(query: &str) -> String {
    let mut q = query.to_string();
    let mappings = [
        ("워크플로우", "workflow"), ("목록", "list"), ("조회", "list"),
        ("보여줘", "list"), ("알려줘", "get"), ("확인", "status"),
        ("실행", "execute"), ("생성", "create"), ("삭제", "delete"),
        ("수정", "update"), ("저장", "save"), ("스케줄", "schedule"),
        ("예약", "schedule"), ("노드", "node"), ("도구", "tool"),
        ("에이전트", "agent"), ("모델", "model"), ("상태", "status"),
        ("검색", "search"), ("문서", "document"), ("임베딩", "embedding"),
        ("프롬프트", "prompt"), ("설정", "config"), ("인증", "auth"),
        ("로그인", "login"), ("사용자", "user"), ("관리자", "admin"),
        ("배포", "deploy"), ("배치", "batch"), ("히스토리", "history"),
        ("로그", "log"), ("성능", "performance"),
    ];
    for (ko, en) in &mappings {
        if q.contains(ko) {
            q = q.replace(ko, en);
        }
    }
    // 남은 한국어가 있으면 원본도 붙이기
    if q != query {
        q = format!("{}", q.trim());
    }
    q
}

/// 메인 API: 사용자 쿼리로 관련 tool을 검색하고 LLM tool schema로 반환
pub async fn search_tools_for_llm(
    query: &str,
    openapi_source: &str,
    top_k: Option<usize>,
) -> Result<Vec<Value>> {
    let bin = find_binary()?;
    let k = top_k.unwrap_or(DEFAULT_TOP_K);

    // 1. graph 빌드 (캐싱)
    let graph_path = ensure_graph(&bin, openapi_source).await?;

    // 2. search로 관련 tool 이름 찾기 (한국어 → 영문 변환)
    let english_query = translate_query(query);
    log::info!("Tool search: '{}' → '{}'", query, english_query);
    let tool_names = search_tool_names(&bin, &english_query, openapi_source, k).await?;
    log::info!("Found {} tools for query '{}'", tool_names.len(), query);

    if tool_names.is_empty() {
        return Ok(vec![]);
    }

    // 3. graph에서 tool 정보 로드
    let graph_tools = load_graph_tools(&graph_path)?;

    // 4. LLM tool schema로 변환
    let mut llm_tools = Vec::new();
    for name in &tool_names {
        if let Some(tool) = graph_tools.get(name) {
            llm_tools.push(to_llm_tool_schema(tool));
        }
    }

    Ok(llm_tools)
}

/// graph-tool-call call로 API 직접 실행
pub async fn execute_tool_call(
    tool_name: &str,
    args: &Value,
    openapi_source: &str,
    base_url: &str,
    auth_token: Option<&str>,
) -> Result<Value> {
    let bin = find_binary()?;

    let args_str = serde_json::to_string(args).unwrap_or_default();

    let mut cmd = tokio::process::Command::new(&bin);
    cmd.args([
        "call", tool_name,
        "-s", openapi_source,
        "--base-url", base_url,
        "--tool", tool_name,
        "--allow-private-hosts",
    ]);

    if let Some(token) = auth_token {
        cmd.args(["--auth-token", token]);
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
