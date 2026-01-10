//! Bore Tunnel Client
//!
//! 자체 bore 서버를 통해 로컬 LLM을 외부에서 접근 가능하게 함.
//! bore 프로토콜: null-terminated JSON (not length-prefixed)

use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 터널 서버 설정
const TUNNEL_SERVER_HOST: &str = "14.6.220.91";
const TUNNEL_SERVER_PORT: u16 = 7835;

/// bore 클라이언트 메시지 (서버로 보내는 메시지)
#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    /// 인증 응답
    Authenticate(String),
    /// 초기 연결 요청 (포트 번호, 0 = 자동 할당)
    Hello(u16),
    /// 연결 수락
    Accept(Uuid),
}

/// bore 서버 메시지 (서버에서 받는 메시지)
#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    /// 인증 챌린지
    Challenge(Uuid),
    /// Hello 응답 (할당된 포트)
    Hello(u16),
    /// 하트비트
    Heartbeat,
    /// 새 연결 요청
    Connection(Uuid),
    /// 에러
    Error(String),
}

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

    /// null-terminated JSON 메시지 전송
    async fn send_message<T: Serialize>(stream: &mut TcpStream, msg: &T) -> Result<(), String> {
        let json = serde_json::to_string(msg)
            .map_err(|e| format!("Failed to serialize: {}", e))?;

        // JSON + null terminator
        let mut bytes = json.into_bytes();
        bytes.push(0);

        stream.write_all(&bytes).await
            .map_err(|e| format!("Failed to send: {}", e))?;
        stream.flush().await
            .map_err(|e| format!("Failed to flush: {}", e))?;

        Ok(())
    }

    /// null-terminated JSON 메시지 수신
    async fn recv_message<T: for<'de> Deserialize<'de>>(reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>) -> Result<T, String> {
        let mut buf = Vec::new();

        // null 문자까지 읽기
        reader.read_until(0, &mut buf).await
            .map_err(|e| format!("Failed to read: {}", e))?;

        // null 문자 제거
        if buf.last() == Some(&0) {
            buf.pop();
        }

        if buf.is_empty() {
            return Err("Empty message received".to_string());
        }

        serde_json::from_slice(&buf)
            .map_err(|e| format!("Failed to parse JSON: {} (raw: {:?})", e, String::from_utf8_lossy(&buf)))
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

        // Hello 메시지 전송 (포트 0 = 자동 할당)
        let hello = ClientMessage::Hello(0);
        Self::send_message(&mut stream, &hello).await?;

        log::info!("Sent Hello message, waiting for response...");

        // 스트림 분리
        let (read_half, write_half) = stream.into_split();
        let mut reader = BufReader::new(read_half);

        // 서버 응답 읽기
        let response: ServerMessage = Self::recv_message(&mut reader).await?;

        let assigned_port = match response {
            ServerMessage::Hello(port) => {
                log::info!("Server assigned port: {}", port);
                port
            }
            ServerMessage::Error(msg) => {
                return Err(format!("Server error: {}", msg));
            }
            other => {
                return Err(format!("Unexpected response: {:?}", other));
            }
        };

        let public_url = format!("http://{}:{}", self.server_host, assigned_port);
        log::info!("Tunnel established: {} -> localhost:{}", public_url, local_port);

        // 릴레이 태스크 시작
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let server_host = self.server_host.clone();
        let server_port = self.server_port;

        tokio::spawn(async move {
            Self::relay_loop(reader, write_half, local_port, server_host, server_port, assigned_port, shutdown_rx).await;
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
        mut reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
        mut _write_half: tokio::net::tcp::OwnedWriteHalf,
        local_port: u16,
        server_host: String,
        server_port: u16,
        assigned_port: u16,
        mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    ) {
        log::info!("Tunnel relay loop started");

        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    log::info!("Tunnel relay shutdown requested");
                    break;
                }
                result = Self::recv_message::<ServerMessage>(&mut reader) => {
                    match result {
                        Ok(msg) => {
                            if let Err(e) = Self::handle_server_message(
                                msg,
                                local_port,
                                &server_host,
                                server_port,
                                assigned_port,
                            ).await {
                                log::error!("Error handling message: {}", e);
                            }
                        }
                        Err(e) => {
                            log::error!("Tunnel relay error: {}", e);
                            break;
                        }
                    }
                }
            }
        }

        log::info!("Tunnel relay loop ended");
    }

    /// 서버 메시지 처리
    async fn handle_server_message(
        msg: ServerMessage,
        local_port: u16,
        server_host: &str,
        server_port: u16,
        assigned_port: u16,
    ) -> Result<(), String> {
        match msg {
            ServerMessage::Connection(uuid) => {
                log::info!("New connection request: {}", uuid);

                // 새 데이터 연결 생성
                let server_addr = format!("{}:{}", server_host, server_port);

                tokio::spawn(async move {
                    if let Err(e) = Self::handle_connection(
                        uuid,
                        local_port,
                        &server_addr,
                        assigned_port,
                    ).await {
                        log::error!("Connection {} failed: {}", uuid, e);
                    }
                });
            }
            ServerMessage::Heartbeat => {
                log::debug!("Received heartbeat");
                // bore 서버는 하트비트 응답을 기대하지 않음
            }
            ServerMessage::Error(msg) => {
                log::error!("Server error: {}", msg);
                return Err(msg);
            }
            other => {
                log::warn!("Unexpected message: {:?}", other);
            }
        }

        Ok(())
    }

    /// 개별 연결 처리 - 새 TCP 연결로 Accept 전송 후 프록시
    async fn handle_connection(
        uuid: Uuid,
        local_port: u16,
        server_addr: &str,
        _assigned_port: u16,
    ) -> Result<(), String> {
        // 서버에 새 연결 (Accept용)
        let mut server_stream = TcpStream::connect(server_addr)
            .await
            .map_err(|e| format!("Failed to connect for accept: {}", e))?;

        // Accept 메시지 전송
        let accept = ClientMessage::Accept(uuid);
        Self::send_message(&mut server_stream, &accept).await?;

        log::debug!("Sent Accept for connection {}", uuid);

        // 로컬 서비스에 연결
        let local_addr = format!("127.0.0.1:{}", local_port);
        let local_stream = TcpStream::connect(&local_addr)
            .await
            .map_err(|e| format!("Failed to connect to local service: {}", e))?;

        log::debug!("Connected to local service for {}", uuid);

        // 양방향 프록시
        let (mut server_read, mut server_write) = server_stream.into_split();
        let (mut local_read, mut local_write) = local_stream.into_split();

        let s2l = tokio::spawn(async move {
            tokio::io::copy(&mut server_read, &mut local_write).await
        });

        let l2s = tokio::spawn(async move {
            tokio::io::copy(&mut local_read, &mut server_write).await
        });

        // 하나가 끝나면 둘 다 종료
        tokio::select! {
            r = s2l => {
                if let Err(e) = r {
                    log::debug!("Server->Local copy ended: {}", e);
                }
            }
            r = l2s => {
                if let Err(e) = r {
                    log::debug!("Local->Server copy ended: {}", e);
                }
            }
        }

        log::debug!("Connection {} finished", uuid);
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
