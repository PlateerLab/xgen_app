//! XGEN Backend API Client
//!
//! HTTP client for xgen-backend-gateway REST API.
//! Used by LLM tool use to execute XGEN operations.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderConfig {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub base_url: String,
    pub api_base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableProvider {
    pub name: String,
    pub model: String,
    pub configured: bool,
    pub available: bool,
}

/// XGEN API Client
pub struct XgenApiClient {
    client: reqwest::Client,
    base_url: String,
    auth_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkflowSummary {
    pub workflow_id: Option<String>,
    pub workflow_name: Option<String>,
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScheduleSummary {
    pub schedule_id: Option<String>,
    pub workflow_id: Option<String>,
    pub cron_expression: Option<String>,
    pub status: Option<String>,
}

impl XgenApiClient {
    pub fn new(base_url: String, auth_token: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_token,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn auth_token(&self) -> Option<&str> {
        self.auth_token.as_deref()
    }

    pub fn set_auth_token(&mut self, token: String) {
        self.auth_token = Some(token);
    }

    /// Generic HTTP request to XGEN API
    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.request(method, &url);

        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }
        req = req.header("Content-Type", "application/json");

        if let Some(body) = body {
            req = req.json(&body);
        }

        let resp = req.send().await.map_err(|e| {
            AppError::XgenApi(format!("Request failed: {}", e))
        })?;

        let status = resp.status();
        let text = resp.text().await.map_err(|e| {
            AppError::XgenApi(format!("Failed to read response: {}", e))
        })?;

        if !status.is_success() {
            return Err(AppError::XgenApi(format!(
                "HTTP {} - {}",
                status.as_u16(),
                text
            )));
        }

        match serde_json::from_str::<Value>(&text) {
            Ok(v) => Ok(v),
            Err(_) => Ok(Value::String(text)),
        }
    }

    // ---- Workflow APIs ----

    pub async fn list_workflows(&self) -> Result<Value> {
        self.request(reqwest::Method::GET, "/api/workflow/list", None).await
    }

    pub async fn get_workflow(&self, workflow_id: &str) -> Result<Value> {
        self.request(
            reqwest::Method::GET,
            &format!("/api/workflow/load/{}", workflow_id),
            None,
        ).await
    }

    pub async fn save_workflow(&self, workflow: Value) -> Result<Value> {
        self.request(reqwest::Method::POST, "/api/workflow/save", Some(workflow)).await
    }

    pub async fn delete_workflow(&self, workflow_id: &str) -> Result<Value> {
        self.request(
            reqwest::Method::DELETE,
            &format!("/api/workflow/delete/{}", workflow_id),
            None,
        ).await
    }

    pub async fn execute_workflow(&self, params: Value) -> Result<Value> {
        self.request(reqwest::Method::POST, "/api/workflow/execute", Some(params)).await
    }

    pub async fn stop_workflow(&self, params: Value) -> Result<Value> {
        self.request(reqwest::Method::POST, "/api/workflow/stop", Some(params)).await
    }

    // ---- Schedule APIs ----

    pub async fn list_schedules(&self) -> Result<Value> {
        self.request(reqwest::Method::GET, "/api/workflow/schedule/list", None).await
    }

    pub async fn create_schedule(&self, schedule: Value) -> Result<Value> {
        self.request(reqwest::Method::POST, "/api/workflow/schedule/create", Some(schedule)).await
    }

    // ---- Node APIs ----

    pub async fn list_nodes(&self) -> Result<Value> {
        self.request(reqwest::Method::GET, "/api/node/get", None).await
    }

    pub async fn get_node_detail(&self, node_id: &str) -> Result<Value> {
        self.request(
            reqwest::Method::GET,
            &format!("/api/node/detail?nodeId={}", node_id),
            None,
        ).await
    }

    // ---- Tool APIs ----

    pub async fn list_tools(&self) -> Result<Value> {
        self.request(reqwest::Method::GET, "/api/tools/storage/list", None).await
    }

    // ---- Config APIs ----

    pub async fn get_configs(&self) -> Result<Value> {
        self.request(reqwest::Method::GET, "/api/config/persistent", None).await
    }

    // ---- LLM APIs ----

    pub async fn get_llm_status(&self) -> Result<Value> {
        self.request(reqwest::Method::GET, "/api/llm/status", None).await
    }

    /// List available LLM providers with their models from XGEN backend
    pub async fn list_available_providers(&self) -> Result<Vec<AvailableProvider>> {
        let status = self.get_llm_status().await?;
        let configs = self.get_configs().await?;
        let categories = &configs["categories"];

        let providers = status["providers"].as_object()
            .ok_or_else(|| AppError::XgenApi("Invalid providers response".into()))?;

        let mut result = Vec::new();
        for (name, info) in providers {
            let configured = info["configured"].as_bool().unwrap_or(false);
            let available = info["available"].as_bool().unwrap_or(false);

            let model_key = format!("{}_MODEL_DEFAULT", name.to_uppercase());
            let model = categories[name]["configs"][&model_key]["current_value"]
                .as_str().unwrap_or("").to_string();

            result.push(AvailableProvider {
                name: name.clone(),
                model,
                configured,
                available,
            });
        }

        // Sort: configured+available first
        result.sort_by(|a, b| {
            let a_score = (a.configured as u8) * 2 + (a.available as u8);
            let b_score = (b.configured as u8) * 2 + (b.available as u8);
            b_score.cmp(&a_score)
        });

        Ok(result)
    }

    /// Get LLM provider config from XGEN backend.
    /// If provider/model specified, use those. Otherwise default to anthropic.
    pub async fn get_llm_config(
        &self,
        preferred_provider: Option<&str>,
        preferred_model: Option<&str>,
    ) -> Result<LlmProviderConfig> {
        let configs = self.get_configs().await?;
        let categories = &configs["categories"];

        // Default to anthropic for best tool use support
        let provider = preferred_provider.unwrap_or("anthropic").to_string();

        let prefix = provider.to_uppercase();
        let key_name = format!("{}_API_KEY", prefix);
        let model_name = format!("{}_MODEL_DEFAULT", prefix);
        let base_url_name = format!("{}_API_BASE_URL", prefix);

        let provider_configs = &categories[&provider]["configs"];

        let api_key = provider_configs[&key_name]["current_value"]
            .as_str().unwrap_or("").to_string();

        let default_model = provider_configs[&model_name]["current_value"]
            .as_str().unwrap_or("claude-sonnet-4-20250514").to_string();

        let api_base_url = provider_configs[&base_url_name]["current_value"]
            .as_str().map(|s| s.to_string());

        // Use preferred model if specified, otherwise use default from config
        let model = preferred_model
            .map(|m| m.to_string())
            .unwrap_or(default_model);

        // Fallback to anthropic if chosen provider has no key
        if api_key.is_empty() && provider != "anthropic" {
            log::warn!("Provider '{}' has no API key, falling back to anthropic", provider);
            let anth = &categories["anthropic"]["configs"];
            let key = anth["ANTHROPIC_API_KEY"]["current_value"]
                .as_str().unwrap_or("").to_string();
            let mdl = anth["ANTHROPIC_MODEL_DEFAULT"]["current_value"]
                .as_str().unwrap_or("claude-sonnet-4-20250514").to_string();
            return Ok(LlmProviderConfig {
                provider: "anthropic".to_string(),
                model: mdl,
                api_key: key,
                base_url: self.base_url.clone(),
                api_base_url: None,
            });
        }

        Ok(LlmProviderConfig {
            provider,
            model,
            api_key,
            base_url: self.base_url.clone(),
            api_base_url,
        })
    }

    // ---- Tool Definitions for LLM ----

    /// Returns tool definitions in Claude API format
    pub fn tool_definitions() -> Vec<Value> {
        vec![
            serde_json::json!({
                "name": "list_workflows",
                "description": "XGEN 워크플로우 목록을 조회합니다. 이름, ID, 설명, 생성일 등을 확인할 수 있습니다.",
                "input_schema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }),
            serde_json::json!({
                "name": "get_workflow",
                "description": "특정 워크플로우의 상세 정보를 조회합니다.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "workflow_id": { "type": "string", "description": "워크플로우 ID" }
                    },
                    "required": ["workflow_id"]
                }
            }),
            serde_json::json!({
                "name": "save_workflow",
                "description": "워크플로우를 생성하거나 수정합니다.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "workflow": { "type": "object", "description": "워크플로우 데이터 (name, nodes, edges 등)" }
                    },
                    "required": ["workflow"]
                }
            }),
            serde_json::json!({
                "name": "execute_workflow",
                "description": "워크플로우를 실행합니다.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "workflow_id": { "type": "string", "description": "실행할 워크플로우 ID" },
                        "input": { "type": "object", "description": "워크플로우 입력 데이터" }
                    },
                    "required": ["workflow_id"]
                }
            }),
            serde_json::json!({
                "name": "list_schedules",
                "description": "워크플로우 스케줄 목록을 조회합니다.",
                "input_schema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }),
            serde_json::json!({
                "name": "create_schedule",
                "description": "워크플로우 스케줄을 생성합니다. cron 표현식으로 주기적 실행을 설정합니다.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "workflow_id": { "type": "string", "description": "워크플로우 ID" },
                        "cron_expression": { "type": "string", "description": "cron 표현식 (예: '0 9 * * *' = 매일 9시)" },
                        "name": { "type": "string", "description": "스케줄 이름" }
                    },
                    "required": ["workflow_id", "cron_expression"]
                }
            }),
            serde_json::json!({
                "name": "list_nodes",
                "description": "사용 가능한 노드(에이전트) 목록을 조회합니다.",
                "input_schema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }),
            serde_json::json!({
                "name": "list_tools",
                "description": "등록된 도구(tool) 목록을 조회합니다.",
                "input_schema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }),
            serde_json::json!({
                "name": "get_llm_status",
                "description": "현재 LLM 제공자 상태를 확인합니다.",
                "input_schema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }),
        ]
    }

    /// Execute a tool by name with given input
    pub async fn execute_tool(&self, tool_name: &str, input: Value) -> Result<Value> {
        match tool_name {
            "list_workflows" => self.list_workflows().await,
            "get_workflow" => {
                let id = input["workflow_id"].as_str()
                    .ok_or_else(|| AppError::Cli("workflow_id required".into()))?;
                self.get_workflow(id).await
            }
            "save_workflow" => {
                let workflow = input.get("workflow").cloned()
                    .ok_or_else(|| AppError::Cli("workflow data required".into()))?;
                self.save_workflow(workflow).await
            }
            "execute_workflow" => {
                let id = input["workflow_id"].as_str()
                    .ok_or_else(|| AppError::Cli("workflow_id required".into()))?;
                let exec_input = input.get("input").cloned().unwrap_or(Value::Object(Default::default()));
                self.execute_workflow(serde_json::json!({
                    "workflow_id": id,
                    "input": exec_input,
                })).await
            }
            "list_schedules" => self.list_schedules().await,
            "create_schedule" => {
                self.create_schedule(input).await
            }
            "list_nodes" => self.list_nodes().await,
            "list_tools" => self.list_tools().await,
            "get_llm_status" => self.get_llm_status().await,
            _ => Err(AppError::Cli(format!("Unknown tool: {}", tool_name))),
        }
    }
}
