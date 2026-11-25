use std::net::UdpSocket;
use std::sync::Arc;

use futures_util::SinkExt;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, watch, Mutex};
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
    shutdown_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    active_port: Arc<Mutex<Option<u16>>>,
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
            shutdown_tx: Arc::new(Mutex::new(None)),
            active_port: Arc::new(Mutex::new(None)),
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
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.task.lock().await.take() {
            let _ = tokio::time::timeout(std::time::Duration::from_millis(500), handle).await;
        }
        *self.active_port.lock().await = None;
        *self.status.lock().await = LanServerStatus::default();
    }

    pub async fn restart(&self, app: &AppHandle, enabled: bool, port: u16) -> LanServerStatus {
        // 若局域网服务已在相同端口正常运行，则无需销毁重建，避免端口重绑冲突
        if enabled {
            let cur_port = *self.active_port.lock().await;
            let cur_status = self.status.lock().await.clone();
            if cur_status.running && cur_port == Some(port) {
                return cur_status;
            }
        }

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
        *self.active_port.lock().await = Some(port);
        *self.status.lock().await = status.clone();

        let app_handle = app.clone();
        let tx = self.snapshot_tx.clone();
        let (stop_tx, mut stop_rx) = oneshot::channel::<()>();

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut stop_rx => break,
                    res = listener.accept() => {
                        let Ok((stream, _)) = res else {
                            break;
                        };
                        let app_c = app_handle.clone();
                        let rx_c = tx.subscribe();
                        tokio::spawn(async move {
                            handle_lan_conn(stream, app_c, rx_c).await;
                        });
                    }
                }
            }
        });

        *self.shutdown_tx.lock().await = Some(stop_tx);
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

    // 2. 静态网页与 SVG 地图资源 HTTP 服务（先完整读取请求头，避免关闭连接时触发 TCP RST）
    let mut req_buf = vec![0u8; 16384];
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
    let mut first_line_parts = req_str
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace();
    let method = first_line_parts.next().unwrap_or("GET");
    let req_path = first_line_parts.next().unwrap_or("/");

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
            let cache_control = if asset_path.starts_with("maps/") || asset_path.starts_with("assets/") {
                "public, max-age=86400"
            } else {
                "no-cache"
            };
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: {}\r\nConnection: close\r\n\r\n",
                asset.mime_type,
                asset.bytes.len(),
                cache_control
            );
            let _ = stream.write_all(header.as_bytes()).await;
            if method != "HEAD" {
                let _ = stream.write_all(&asset.bytes).await;
            }
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
