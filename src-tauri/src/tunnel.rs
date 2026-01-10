//! Bore Tunnel Client
//!
//! 자체 bore 서버를 통해 로컬 LLM을 외부에서 접근 가능하게 함.
//! ngrok과 달리 authtoken 없이 완전 자동화 가능.

use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// 터널 서버 설정
const TUNNEL_SERVER_HOST: &str = "14.6.220.91";
const TUNNEL_SERVER_PORT: u16 = 7835;

/// 터널 상태
#[derive(Clone, serde::Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TunnelStatus {
    pub connected: bool,
    pub public_url: Option<String>,
    pub local_port: Option<u16>,
    pub error: Option<String>,
}

/// 활성 터널 정보
struct ActiveTunnel {
    public_url: String,
    local_port: u16,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

/// bore 터널 매니저
pub struct TunnelManager {
    tunnel: Arc<RwLock<Option<ActiveTunnel>>>,
    status: Arc<RwLock<TunnelStatus>>,
    server_host: String,
    server_port: u16,
}

impl TunnelManager {
    pub fn new() -> Self {
        Self {
            tunnel: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(TunnelStatus::default())),
            server_host: TUNNEL_SERVER_HOST.to_string(),
            server_port: TUNNEL_SERVER_PORT,
        }
    }

    /// 커스텀 서버 설정
    pub fn with_server(mut self, host: String, port: u16) -> Self {
        self.server_host = host;
        self.server_port = port;
        self
    }

    /// 터널 시작
    pub async fn start(&self, local_port: u16) -> Result<String, String> {
        // 이미 연결된 경우
        {
            let tunnel = self.tunnel.read().await;
            if let Some(active) = tunnel.as_ref() {
                if active.local_port == local_port {
                    log::info!("Tunnel already connected: {}", active.public_url);
                    return Ok(active.public_url.clone());
                }
            }
        }

        // 기존 터널 중지
        self.stop().await;

        log::info!(
            "Starting bore tunnel: localhost:{} -> {}:{}",
            local_port,
            self.server_host,
            self.server_port
        );

        // bore 서버에 연결
        let server_addr = format!("{}:{}", self.server_host, self.server_port);
        let mut stream = TcpStream::connect(&server_addr)
            .await
            .map_err(|e| format!("Failed to connect to tunnel server: {}", e))?;

        // bore 프로토콜: 포트 요청 (0 = 자동 할당)
        // bore는 간단한 JSON 프로토콜 사용
        let hello = serde_json::json!({
            "Hello": { "port": 0 }  // 0 = 서버가 포트 자동 할당
        });
        let hello_bytes = serde_json::to_vec(&hello)
            .map_err(|e| format!("Failed to serialize: {}", e))?;

        // 길이 prefix (u64 big-endian)
        stream
            .write_all(&(hello_bytes.len() as u64).to_be_bytes())
            .await
            .map_err(|e| format!("Failed to send: {}", e))?;
        stream
            .write_all(&hello_bytes)
            .await
            .map_err(|e| format!("Failed to send: {}", e))?;

        // 서버 응답 읽기
        let mut len_buf = [0u8; 8];
        stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| format!("Failed to read response length: {}", e))?;
        let len = u64::from_be_bytes(len_buf) as usize;

        let mut response_buf = vec![0u8; len];
        stream
            .read_exact(&mut response_buf)
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        let response: serde_json::Value = serde_json::from_slice(&response_buf)
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        // 할당된 포트 추출
        let assigned_port = response
            .get("Hello")
            .and_then(|h| h.get("port"))
            .and_then(|p| p.as_u64())
            .ok_or_else(|| format!("Invalid response from server: {:?}", response))?
            as u16;

        let public_url = format!("http://{}:{}", self.server_host, assigned_port);
        log::info!("Tunnel established: {} -> localhost:{}", public_url, local_port);

        // 릴레이 태스크 시작
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let local_port_clone = local_port;
        let server_host = self.server_host.clone();
        let server_port = self.server_port;

        tokio::spawn(async move {
            Self::relay_loop(stream, local_port_clone, server_host, server_port, shutdown_rx).await;
        });

        // 상태 업데이트
        {
            let mut tunnel = self.tunnel.write().await;
            *tunnel = Some(ActiveTunnel {
                public_url: public_url.clone(),
                local_port,
                shutdown_tx,
            });
        }

        {
            let mut status = self.status.write().await;
            status.connected = true;
            status.public_url = Some(public_url.clone());
            status.local_port = Some(local_port);
            status.error = None;
        }

        Ok(public_url)
    }

    /// 릴레이 루프 - 터널 서버와 로컬 포트 간 데이터 전달
    async fn relay_loop(
        mut control_stream: TcpStream,
        local_port: u16,
        server_host: String,
        server_port: u16,
        mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    ) {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    log::info!("Tunnel relay shutdown requested");
                    break;
                }
                result = Self::handle_connection(&mut control_stream, local_port, &server_host, server_port) => {
                    if let Err(e) = result {
                        log::error!("Tunnel relay error: {}", e);
                        break;
                    }
                }
            }
        }
    }

    /// 개별 연결 처리
    async fn handle_connection(
        control_stream: &mut TcpStream,
        local_port: u16,
        server_host: &str,
        _server_port: u16,
    ) -> Result<(), String> {
        // bore 프로토콜: 서버가 Connection 메시지 전송
        let mut len_buf = [0u8; 8];
        control_stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| format!("Control read error: {}", e))?;
        let len = u64::from_be_bytes(len_buf) as usize;

        let mut msg_buf = vec![0u8; len];
        control_stream
            .read_exact(&mut msg_buf)
            .await
            .map_err(|e| format!("Control read error: {}", e))?;

        let msg: serde_json::Value = serde_json::from_slice(&msg_buf)
            .map_err(|e| format!("Parse error: {}", e))?;

        // Connection 요청 처리
        if let Some(conn) = msg.get("Connection") {
            let conn_port = conn
                .get("port")
                .and_then(|p| p.as_u64())
                .ok_or_else(|| "Invalid connection message".to_string())?
                as u16;

            // 새 데이터 연결
            let data_addr = format!("{}:{}", server_host, conn_port);
            let data_stream = TcpStream::connect(&data_addr)
                .await
                .map_err(|e| format!("Data connection failed: {}", e))?;

            // 로컬 포트에 연결
            let local_stream = TcpStream::connect(format!("127.0.0.1:{}", local_port))
                .await
                .map_err(|e| format!("Local connection failed: {}", e))?;

            // 양방향 릴레이
            tokio::spawn(async move {
                let (mut data_read, mut data_write) = data_stream.into_split();
                let (mut local_read, mut local_write) = local_stream.into_split();

                let _ = tokio::join!(
                    tokio::io::copy(&mut data_read, &mut local_write),
                    tokio::io::copy(&mut local_read, &mut data_write),
                );
            });
        } else if msg.get("Heartbeat").is_some() {
            // 하트비트 응답
            let heartbeat = serde_json::json!({"Heartbeat": {}});
            let heartbeat_bytes = serde_json::to_vec(&heartbeat).unwrap();
            control_stream
                .write_all(&(heartbeat_bytes.len() as u64).to_be_bytes())
                .await
                .map_err(|e| format!("Heartbeat write error: {}", e))?;
            control_stream
                .write_all(&heartbeat_bytes)
                .await
                .map_err(|e| format!("Heartbeat write error: {}", e))?;
        }

        Ok(())
    }

    /// 터널 중지
    pub async fn stop(&self) {
        let mut tunnel = self.tunnel.write().await;
        if let Some(active) = tunnel.take() {
            log::info!("Stopping tunnel: {}", active.public_url);
            let _ = active.shutdown_tx.send(());
        }

        let mut status = self.status.write().await;
        status.connected = false;
        status.public_url = None;
        status.local_port = None;
    }

    /// 현재 상태 조회
    pub async fn get_status(&self) -> TunnelStatus {
        self.status.read().await.clone()
    }

    /// 연결 여부 확인
    pub async fn is_connected(&self) -> bool {
        self.status.read().await.connected
    }

    /// Public URL 조회
    pub async fn get_public_url(&self) -> Option<String> {
        self.status.read().await.public_url.clone()
    }
}

impl Default for TunnelManager {
    fn default() -> Self {
        Self::new()
    }
}
