use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::types::{ServerRoomSummary, TeamMemberInfo, TeamRoomState};

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ClientMsg {
    ListRooms,
    CreateRoom {
        room_name: String,
        passcode: String,
        player_name: String,
        map_id: String,
    },
    JoinRoom {
        room_id: String,
        passcode: String,
        player_name: String,
        map_id: String,
    },
    Pos {
        player_name: String,
        map_id: String,
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
    },
    Ping,
    LeaveRoom,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ServerMsg {
    RoomList {
        rooms: Vec<ServerRoomSummary>,
    },
    Joined {
        room_id: String,
        room_name: String,
        is_host: bool,
        self_peer_id: String,
        members: Vec<TeamMemberInfo>,
    },
    RoomState {
        room_id: String,
        room_name: String,
        members: Vec<TeamMemberInfo>,
    },
    RoomClosed {
        reason: String,
    },
    Pong,
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerConnectionStatus {
    pub connected: bool,
    pub addr: String,
}

struct ActiveServerConn {
    addr: String,
    tx: mpsc::UnboundedSender<String>,
    tasks: Vec<JoinHandle<()>>,
}

pub struct ServerClientManager {
    conn: Arc<Mutex<Option<ActiveServerConn>>>,
    room_state: Arc<RwLock<TeamRoomState>>,
    rooms_cache: Arc<RwLock<Vec<ServerRoomSummary>>>,
    is_host_flag: Arc<RwLock<bool>>,
    self_peer_id: Arc<RwLock<String>>,
}

impl ServerClientManager {
    pub fn new() -> Self {
        Self {
            conn: Arc::new(Mutex::new(None)),
            room_state: Arc::new(RwLock::new(TeamRoomState::default())),
            rooms_cache: Arc::new(RwLock::new(Vec::new())),
            is_host_flag: Arc::new(RwLock::new(false)),
            self_peer_id: Arc::new(RwLock::new(String::new())),
        }
    }

    pub async fn get_status(&self) -> ServerConnectionStatus {
        let guard = self.conn.lock().await;
        match guard.as_ref() {
            Some(c) => ServerConnectionStatus {
                connected: true,
                addr: c.addr.clone(),
            },
            None => ServerConnectionStatus::default(),
        }
    }

    pub async fn get_room_state(&self) -> TeamRoomState {
        self.room_state.read().await.clone()
    }

    pub async fn disconnect(&self, app: &AppHandle) {
        let mut guard = self.conn.lock().await;
        if let Some(mut c) = guard.take() {
            for t in c.tasks.drain(..) {
                t.abort();
            }
        }
        *self.room_state.write().await = TeamRoomState::default();
        *self.rooms_cache.write().await = Vec::new();
        *self.is_host_flag.write().await = false;
        *self.self_peer_id.write().await = String::new();

        let _ = app.emit(
            "server-connection-changed",
            ServerConnectionStatus::default(),
        );
        let _ = app.emit("server-rooms-updated", Vec::<ServerRoomSummary>::new());
    }

    pub async fn connect(&self, app: &AppHandle, raw_addr: String) -> Result<ServerConnectionStatus, String> {
        self.disconnect(app).await;

        let trimmed = raw_addr.trim();
        if trimmed.is_empty() {
            return Err("请输入服务器 IP 与端口".to_string());
        }
        let ws_url = if trimmed.starts_with("ws://") || trimmed.starts_with("wss://") {
            trimmed.to_string()
        } else {
            format!("ws://{trimmed}")
        };

        let (ws_stream, _) = connect_async(&ws_url)
            .await
            .map_err(|e| format!("连接服务器失败 ({ws_url}): {e}"))?;

        let (mut ws_write, mut ws_read) = ws_stream.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();

        let write_task = tokio::spawn(async move {
            while let Some(text) = rx.recv().await {
                if ws_write.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
        });

        let ping_task = {
            let tx_c = tx.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    if let Ok(json) = serde_json::to_string(&ClientMsg::Ping) {
                        if tx_c.send(json).is_err() {
                            break;
                        }
                    }
                }
            })
        };

        let read_task = {
            let app_h = app.clone();
            let conn_arc = self.conn.clone();
            let room_snap = self.room_state.clone();
            let rooms_cache = self.rooms_cache.clone();
            let is_host_arc = self.is_host_flag.clone();
            let self_pid_arc = self.self_peer_id.clone();

            tokio::spawn(async move {
                while let Some(Ok(msg)) = ws_read.next().await {
                    let Message::Text(text) = msg else {
                        continue;
                    };
                    let Ok(parsed) = serde_json::from_str::<ServerMsg>(&text) else {
                        continue;
                    };
                    match parsed {
                        ServerMsg::RoomList { rooms } => {
                            *rooms_cache.write().await = rooms.clone();
                            let _ = app_h.emit("server-rooms-updated", &rooms);
                        }
                        ServerMsg::Joined {
                            room_id,
                            room_name,
                            is_host,
                            self_peer_id,
                            members,
                        } => {
                            *is_host_arc.write().await = is_host;
                            *self_pid_arc.write().await = self_peer_id.clone();
                            let state = TeamRoomState {
                                in_room: true,
                                is_host,
                                mode: "server".to_string(),
                                self_peer_id,
                                room_code: room_id,
                                full_code: room_name,
                                member_count: members.len(),
                                members,
                            };
                            *room_snap.write().await = state.clone();
                            let _ = app_h.emit("team-state-updated", &state);
                        }
                        ServerMsg::RoomState {
                            room_id,
                            room_name,
                            members,
                        } => {
                            let is_host = *is_host_arc.read().await;
                            let self_peer_id = self_pid_arc.read().await.clone();
                            let state = TeamRoomState {
                                in_room: true,
                                is_host,
                                mode: "server".to_string(),
                                self_peer_id,
                                room_code: room_id,
                                full_code: room_name,
                                member_count: members.len(),
                                members,
                            };
                            *room_snap.write().await = state.clone();
                            let _ = app_h.emit("team-state-updated", &state);
                        }
                        ServerMsg::RoomClosed { reason } => {
                            *room_snap.write().await = TeamRoomState::default();
                            *is_host_arc.write().await = false;
                            let _ = app_h.emit("team-state-updated", &TeamRoomState::default());
                            let _ = app_h.emit("team-room-closed", &reason);
                        }
                        ServerMsg::Error { message } => {
                            let _ = app_h.emit("server-error", &message);
                        }
                        ServerMsg::Pong => {}
                    }
                }

                // 服务器连接断开
                if let Some(mut c) = conn_arc.lock().await.take() {
                    for t in c.tasks.drain(..) {
                        t.abort();
                    }
                }
                let was_in_room = room_snap.read().await.in_room;
                *room_snap.write().await = TeamRoomState::default();
                *rooms_cache.write().await = Vec::new();
                let _ = app_h.emit(
                    "server-connection-changed",
                    ServerConnectionStatus::default(),
                );
                let _ = app_h.emit("server-rooms-updated", Vec::<ServerRoomSummary>::new());
                if was_in_room {
                    let _ = app_h.emit("team-state-updated", &TeamRoomState::default());
                    let _ = app_h.emit("team-room-closed", "与联机服务器断开连接");
                }
            })
        };

        // 连接成功后自动请求一次房间列表
        if let Ok(json) = serde_json::to_string(&ClientMsg::ListRooms) {
            let _ = tx.send(json);
        }

        *self.conn.lock().await = Some(ActiveServerConn {
            addr: trimmed.to_string(),
            tx,
            tasks: vec![write_task, ping_task, read_task],
        });

        let status = ServerConnectionStatus {
            connected: true,
            addr: trimmed.to_string(),
        };
        let _ = app.emit("server-connection-changed", &status);
        Ok(status)
    }

    async fn send_msg(&self, msg: &ClientMsg) -> Result<(), String> {
        let guard = self.conn.lock().await;
        let conn = guard
            .as_ref()
            .ok_or_else(|| "尚未连接到联机服务器".to_string())?;
        let json = serde_json::to_string(msg).map_err(|e| format!("序列化失败: {e}"))?;
        conn.tx
            .send(json)
            .map_err(|_| "发送失败，服务器连接已断开".to_string())
    }

    pub async fn list_rooms(&self) -> Result<(), String> {
        self.send_msg(&ClientMsg::ListRooms).await
    }

    pub async fn create_room(
        &self,
        room_name: String,
        passcode: String,
        player_name: String,
        map_id: String,
    ) -> Result<(), String> {
        if passcode.trim().is_empty() {
            return Err("请设置房间口令".to_string());
        }
        self.send_msg(&ClientMsg::CreateRoom {
            room_name,
            passcode,
            player_name,
            map_id,
        })
        .await
    }

    pub async fn join_room(
        &self,
        room_id: String,
        passcode: String,
        player_name: String,
        map_id: String,
    ) -> Result<(), String> {
        if passcode.trim().is_empty() {
            return Err("请输入房间口令".to_string());
        }
        self.send_msg(&ClientMsg::JoinRoom {
            room_id,
            passcode,
            player_name,
            map_id,
        })
        .await
    }

    pub async fn leave_room(&self, app: &AppHandle) -> Result<(), String> {
        if self.room_state.read().await.in_room {
            let _ = self.send_msg(&ClientMsg::LeaveRoom).await;
            tokio::time::sleep(Duration::from_millis(80)).await;
        }
        *self.room_state.write().await = TeamRoomState::default();
        *self.is_host_flag.write().await = false;
        let _ = app.emit("team-state-updated", &TeamRoomState::default());
        Ok(())
    }

    pub async fn sync_position(
        &self,
        player_name: String,
        map_id: String,
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
    ) {
        if !self.room_state.read().await.in_room {
            return;
        }
        let _ = self
            .send_msg(&ClientMsg::Pos {
                player_name,
                map_id,
                x,
                y,
                z,
            })
            .await;
    }
}
