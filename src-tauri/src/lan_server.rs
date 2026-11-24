use std::net::UdpSocket;
use std::sync::Arc;

use futures_util::SinkExt;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, Mutex};
use tokio::task::JoinHandle;
use tokio_tungstenite::{accept_async, tungstenite::Message};

use crate::types::{LanServerStatus, ScreenshotCoordPayload, TeamMemberInfo};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LanSnapshot {
    pub map_id: String,
    pub player_name: String,
    pub latest_coord: Option<ScreenshotCoordPayload>,
    pub teammates: Vec<TeamMemberInfo>,
}

pub struct LanServerManager {
    snapshot_tx: watch::Sender<String>,
    task: Arc<Mutex<Option<JoinHandle<()>>>>,
    status: Arc<Mutex<LanServerStatus>>,
}

impl LanServerManager {
    pub fn new() -> Self {
        let init_json = serde_json::to_string(&LanSnapshot {
            map_id: "customs".to_string(),
            ..Default::default()
        })
        .unwrap_or_else(|_| "{}".to_string());
        let (snapshot_tx, _) = watch::channel(init_json);

        Self {
            snapshot_tx,
            task: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(LanServerStatus::default())),
        }
    }

    pub async fn get_status(&self) -> LanServerStatus {
        self.status.lock().await.clone()
    }

    pub fn update_snapshot(&self, snapshot: LanSnapshot) {
        if let Ok(json) = serde_json::to_string(&snapshot) {
            let _ = self.snapshot_tx.send(json);
        }
    }

    pub async fn stop(&self) {
        if let Some(handle) = self.task.lock().await.take() {
            handle.abort();
        }
        *self.status.lock().await = LanServerStatus::default();
    }

    pub async fn restart(&self, app: &AppHandle, enabled: bool, port: u16) -> LanServerStatus {
        self.stop().await;

        if !enabled {
            return LanServerStatus::default();
        }

        let bind_addr = format!("0.0.0.0:{port}");
        let listener = match TcpListener::bind(&bind_addr).await {
            Ok(l) => l,
            Err(e) => {
                let err_status = LanServerStatus {
                    running: false,
                    url: String::new(),
                    error: Some(format!("端口 {port} 启动失败: {e}")),
                };
                *self.status.lock().await = err_status.clone();
                return err_status;
            }
        };

        let lan_ip = detect_lan_ip().unwrap_or_else(|| "127.0.0.1".to_string());
        let url = format!("http://{lan_ip}:{port}");
        let status = LanServerStatus {
            running: true,
            url,
            error: None,
        };
        *self.status.lock().await = status.clone();

        let app_handle = app.clone();
        let tx = self.snapshot_tx.clone();

        let handle = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let app_c = app_handle.clone();
                let rx_c = tx.subscribe();
                tokio::spawn(async move {
                    handle_lan_conn(stream, app_c, rx_c).await;
                });
            }
        });

        *self.task.lock().await = Some(handle);
        status
    }
}

async fn handle_lan_conn(
    mut stream: TcpStream,
    app: AppHandle,
    mut rx: watch::Receiver<String>,
) {
    let mut peek_buf = [0u8; 2048];
    let Ok(n) = stream.peek(&mut peek_buf).await else {
        return;
    };
    if n == 0 {
        return;
    }

    let req_head = String::from_utf8_lossy(&peek_buf[..n]);
    let lower = req_head.to_ascii_lowercase();

    // 1. WebSocket 实时状态推送通道
    if lower.contains("upgrade: websocket") {
        let Ok(mut ws) = accept_async(stream).await else {
            return;
        };
        let init_msg = rx.borrow().clone();
        if ws.send(Message::Text(init_msg.into())).await.is_err() {
            return;
        }
        while rx.changed().await.is_ok() {
            let next_msg = rx.borrow().clone();
            if ws.send(Message::Text(next_msg.into())).await.is_err() {
                break;
            }
        }
        return;
    }

    // 2. 静态网页与 SVG 地图资源 HTTP 服务（先读取消费请求头，避免关闭连接时触发 TCP RST）
    let mut req_buf = vec![0u8; 4096];
    let mut total = 0usize;
    loop {
        if total >= req_buf.len() {
            break;
        }
        let Ok(read_n) = stream.read(&mut req_buf[total..]).await else {
            return;
        };
        if read_n == 0 {
            break;
        }
        total += read_n;
        if req_buf[..total].windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }

    let req_str = String::from_utf8_lossy(&req_buf[..total]);
    let req_path = req_str
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");

    let clean_path = req_path.split('?').next().unwrap_or("/").trim_start_matches('/');
    let asset_path = if clean_path.is_empty() {
        "index.html"
    } else {
        clean_path
    };

    let resolver = app.asset_resolver();
    let resolved = resolver
        .get(asset_path.to_string())
        .or_else(|| resolver.get(format!("/{asset_path}")))
        .or_else(|| {
            if !asset_path.contains('.') {
                resolver
                    .get("index.html".to_string())
                    .or_else(|| resolver.get("/index.html".to_string()))
            } else {
                None
            }
        });

    match resolved {
        Some(asset) => {
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
                asset.mime_type,
                asset.bytes.len()
            );
            let _ = stream.write_all(header.as_bytes()).await;
            let _ = stream.write_all(&asset.bytes).await;
        }
        None => {
            let body = b"404 Not Found";
            let header = format!(
                "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(header.as_bytes()).await;
            let _ = stream.write_all(body).await;
        }
    }
    let _ = stream.flush().await;
    let _ = stream.shutdown().await;
}

fn detect_lan_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let addr = socket.local_addr().ok()?;
    Some(addr.ip().to_string())
}
