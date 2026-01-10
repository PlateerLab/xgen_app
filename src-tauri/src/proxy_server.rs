//! Local LLM Proxy Server
//!
//! HTTP 서버를 띄워서 외부(Gateway)에서 로컬 LLM에 접근할 수 있도록 합니다.
//! 이 서버는 ngrok 등의 터널링 서비스와 함께 사용됩니다.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, Method, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{any, get},
    Router,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};

/// 프록시 서버 상태
#[derive(Clone)]
pub struct ProxyServerState {
    /// 로컬 LLM 엔드포인트 (예: http://127.0.0.1:8080)
    pub local_llm_endpoint: Arc<RwLock<Option<String>>>,
    /// 허용된 API 키 (선택적 인증)
    pub api_key: Arc<RwLock<Option<String>>>,
}

impl ProxyServerState {
    pub fn new() -> Self {
        Self {
            local_llm_endpoint: Arc::new(RwLock::new(None)),
            api_key: Arc::new(RwLock::new(None)),
        }
    }
}

/// 프록시 서버 핸들
pub struct ProxyServer {
    state: ProxyServerState,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    port: Option<u16>,
}

impl ProxyServer {
    pub fn new() -> Self {
        Self {
            state: ProxyServerState::new(),
            shutdown_tx: None,
            port: None,
        }
    }

    /// 프록시 서버 시작
    pub async fn start(&mut self, port: u16) -> Result<u16, String> {
        if self.shutdown_tx.is_some() {
            return Err("Proxy server is already running".to_string());
        }

        let state = self.state.clone();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        // CORS 설정
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
            .allow_headers(Any);

        // 라우터 설정 (axum 0.8: wildcard uses {*path} syntax)
        let app = Router::new()
            .route("/health", get(health_check))
            .route("/v1/{*path}", any(proxy_handler))
            .route("/{*path}", any(proxy_handler))
            .layer(cors)
            .with_state(state);

        // 서버 바인딩
        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| format!("Failed to bind to port {}: {}", port, e))?;

        let actual_port = listener.local_addr()
            .map_err(|e| format!("Failed to get local address: {}", e))?
            .port();

        log::info!("Local LLM proxy server starting on port {}", actual_port);

        // 서버 실행 (백그라운드)
        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    shutdown_rx.await.ok();
                })
                .await
                .ok();
            log::info!("Local LLM proxy server stopped");
        });

        self.shutdown_tx = Some(shutdown_tx);
        self.port = Some(actual_port);

        Ok(actual_port)
    }

    /// 프록시 서버 중지
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
            self.port = None;
            log::info!("Local LLM proxy server shutdown signal sent");
        }
    }

    /// 로컬 LLM 엔드포인트 설정
    pub async fn set_local_llm_endpoint(&self, endpoint: Option<String>) {
        let mut guard = self.state.local_llm_endpoint.write().await;
        *guard = endpoint.clone();
        log::info!("Local LLM endpoint set to: {:?}", endpoint);
    }

    /// API 키 설정 (선택적 인증)
    pub async fn set_api_key(&self, key: Option<String>) {
        let mut guard = self.state.api_key.write().await;
        *guard = key;
    }

    /// 현재 포트 반환
    pub fn get_port(&self) -> Option<u16> {
        self.port
    }

    /// 현재 로컬 LLM 엔드포인트 반환
    pub async fn get_local_llm_endpoint(&self) -> Option<String> {
        self.state.local_llm_endpoint.read().await.clone()
    }

    /// 서버 실행 중인지 확인
    pub fn is_running(&self) -> bool {
        self.shutdown_tx.is_some()
    }
}

/// Health check 핸들러
async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

/// 프록시 핸들러 - 모든 요청을 로컬 LLM으로 전달
async fn proxy_handler(
    State(state): State<ProxyServerState>,
    Path(path): Path<String>,
    req: Request<Body>,
) -> Response {
    // 로컬 LLM 엔드포인트 확인
    let endpoint = {
        let guard = state.local_llm_endpoint.read().await;
        guard.clone()
    };

    let endpoint = match endpoint {
        Some(ep) => ep,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Local LLM endpoint not configured",
            )
                .into_response();
        }
    };

    // API 키 검증 (설정된 경우)
    let expected_key = {
        let guard = state.api_key.read().await;
        guard.clone()
    };

    if let Some(expected) = expected_key {
        let auth_header = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let provided_key = auth_header
            .strip_prefix("Bearer ")
            .unwrap_or(auth_header);

        if provided_key != expected {
            return (StatusCode::UNAUTHORIZED, "Invalid API key").into_response();
        }
    }

    // 대상 URL 구성
    let target_url = format!("{}/{}", endpoint.trim_end_matches('/'), path);
    log::debug!("Proxying request to: {}", target_url);

    // 요청 전달
    let client = reqwest::Client::new();
    let method = req.method().clone();

    let mut request_builder = match method {
        Method::GET => client.get(&target_url),
        Method::POST => client.post(&target_url),
        Method::PUT => client.put(&target_url),
        Method::DELETE => client.delete(&target_url),
        Method::PATCH => client.patch(&target_url),
        _ => {
            return (StatusCode::METHOD_NOT_ALLOWED, "Method not allowed").into_response();
        }
    };

    // 헤더 복사 (일부 제외)
    for (name, value) in req.headers().iter() {
        if name != header::HOST && name != header::CONNECTION {
            if let Ok(v) = value.to_str() {
                request_builder = request_builder.header(name.as_str(), v);
            }
        }
    }

    // Body 전달
    let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("Failed to read request body: {}", e);
            return (StatusCode::BAD_REQUEST, "Failed to read request body").into_response();
        }
    };

    if !body_bytes.is_empty() {
        request_builder = request_builder.body(body_bytes.to_vec());
    }

    // 요청 실행
    match request_builder.send().await {
        Ok(resp) => {
            let status = StatusCode::from_u16(resp.status().as_u16())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

            // 스트리밍 응답 처리
            let content_type = resp
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/json")
                .to_string();

            let body_stream = resp.bytes_stream();

            Response::builder()
                .status(status)
                .header(header::CONTENT_TYPE, content_type)
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::from_stream(body_stream))
                .unwrap_or_else(|_| {
                    (StatusCode::INTERNAL_SERVER_ERROR, "Failed to build response").into_response()
                })
        }
        Err(e) => {
            log::error!("Proxy request failed: {}", e);
            (
                StatusCode::BAD_GATEWAY,
                format!("Failed to connect to local LLM: {}", e),
            )
                .into_response()
        }
    }
}
