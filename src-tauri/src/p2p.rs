use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use iroh::{
    endpoint::{presets, Connection},
    Endpoint, EndpointId,
};
use pkarr::{Client as PkarrClient, Keypair as PkarrKeypair, SignedPacket};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;

use crate::types::{TeamMemberInfo, TeamRoomState};

const ALPN: &[u8] = b"tkfmap/team-sync/1";
const CODE_CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum WireMessage {
    Hello {
        peer_id: String,
        player_name: String,
        map_id: String,
    },
    Pos {
        peer_id: String,
        player_name: String,
        map_id: String,
        x: f64,
        y: f64,
        z: f64,
    },
    Ping {
        peer_id: String,
    },
    Leave {
        peer_id: String,
    },
    RoomState {
        room_code: String,
        members: Vec<TeamMemberInfo>,
    },
    RoomClosed,
}

struct ActiveSession {
    is_host: bool,
    self_peer_id: String,
    room_code: String,
    full_code: String,
    endpoint: Endpoint,
    tasks: Vec<JoinHandle<()>>,
    /// 房主用于向所有已连接队友广播消息的发送端列表
    peer_txs: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<WireMessage>>>>,
    /// 房主保留的底层 QUIC 连接，确保连接不被提前 drop 且可主动关闭
    peer_conns: Arc<Mutex<HashMap<String, Connection>>>,
    /// 房主维护的成员状态表
    host_members: Arc<Mutex<HashMap<String, (TeamMemberInfo, u64)>>>,
    /// 队友用于向房主发送消息的发送端
    host_tx: Option<mpsc::UnboundedSender<WireMessage>>,
    /// 队友保留的到房主的 QUIC 连接
    host_conn: Option<Connection>,
}

pub struct P2pManager {
    session: Arc<Mutex<Option<ActiveSession>>>,
    state_snapshot: Arc<RwLock<TeamRoomState>>,
}

impl P2pManager {
    pub fn new() -> Self {
        Self {
            session: Arc::new(Mutex::new(None)),
            state_snapshot: Arc::new(RwLock::new(TeamRoomState::default())),
        }
    }

    pub async fn get_state(&self) -> TeamRoomState {
        self.state_snapshot.read().await.clone()
    }

    pub async fn leave_room(&self, app: &AppHandle) -> TeamRoomState {
        let mut guard = self.session.lock().await;
        if let Some(mut sess) = guard.take() {
            if sess.is_host {
                {
                    let txs = sess.peer_txs.lock().await;
                    for tx in txs.values() {
                        let _ = tx.send(WireMessage::RoomClosed);
                    }
                }
                // 给 write_task 留出时间把 RoomClosed 刷新到 QUIC 流后再关闭连接
                tokio::time::sleep(Duration::from_millis(120)).await;
                let conns = sess.peer_conns.lock().await;
                for conn in conns.values() {
                    conn.close(0u32.into(), b"room closed");
                }
            } else {
                if let Some(tx) = &sess.host_tx {
                    let _ = tx.send(WireMessage::Leave {
                        peer_id: sess.self_peer_id.clone(),
                    });
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                if let Some(conn) = &sess.host_conn {
                    conn.close(0u32.into(), b"peer left");
                }
            }

            for task in sess.tasks.drain(..) {
                task.abort();
            }
            sess.endpoint.close().await;
        }
        let empty = TeamRoomState::default();
        *self.state_snapshot.write().await = empty.clone();
        let _ = app.emit("team-state-updated", &empty);
        empty
    }

    pub async fn create_room(
        &self,
        app: &AppHandle,
        player_name: String,
        map_id: String,
    ) -> Result<TeamRoomState, String> {
        self.leave_room(app).await;

        let room_code = generate_room_code();
        let endpoint = Endpoint::builder(presets::N0)
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await
            .map_err(|e| format!("初始化 P2P 节点失败: {e}"))?;

        endpoint.online().await;
        let host_id = endpoint.id();
        let self_peer_id = host_id.to_string();
        let full_code = format!("{room_code}:{self_peer_id}");

        // 后台异步发布短房间 Code -> Host EndpointId 到公共 DHT (Pkarr)
        let code_for_dht = room_code.clone();
        let id_for_dht = self_peer_id.clone();
        tokio::spawn(async move {
            let _ = publish_room_to_pkarr(&code_for_dht, &id_for_dht).await;
        });

        let now = now_secs();
        let mut init_members = HashMap::new();
        let host_member = TeamMemberInfo {
            peer_id: self_peer_id.clone(),
            player_name: player_name.clone(),
            map_id: map_id.clone(),
            x: None,
            y: None,
            z: None,
            updated_at: now,
        };
        init_members.insert(self_peer_id.clone(), (host_member.clone(), now));

        let host_members = Arc::new(Mutex::new(init_members));
        let peer_txs: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<WireMessage>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let peer_conns: Arc<Mutex<HashMap<String, Connection>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let initial_state = TeamRoomState {
            in_room: true,
            is_host: true,
            mode: "p2p".to_string(),
            self_peer_id: self_peer_id.clone(),
            room_code: room_code.clone(),
            full_code: full_code.clone(),
            member_count: 1,
            members: vec![host_member],
        };
        *self.state_snapshot.write().await = initial_state.clone();
        let _ = app.emit("team-state-updated", &initial_state);

        // 1. 接收队友连接任务
        let accept_task = {
            let ep = endpoint.clone();
            let app_h = app.clone();
            let members = host_members.clone();
            let txs = peer_txs.clone();
            let conns = peer_conns.clone();
            let snap = self.state_snapshot.clone();
            let code = room_code.clone();
            let full = full_code.clone();
            let host_pid = self_peer_id.clone();

            tokio::spawn(async move {
                while let Some(incoming) = ep.accept().await {
                    let app_c = app_h.clone();
                    let members_c = members.clone();
                    let txs_c = txs.clone();
                    let conns_c = conns.clone();
                    let snap_c = snap.clone();
                    let code_c = code.clone();
                    let full_c = full.clone();
                    let host_pid_c = host_pid.clone();

                    tokio::spawn(async move {
                        let Ok(conn) = incoming.await else {
                            return;
                        };
                        let Ok((send, recv)) = conn.accept_bi().await else {
                            return;
                        };
                        handle_host_peer_stream(
                            app_c,
                            conn,
                            send,
                            recv,
                            members_c,
                            txs_c,
                            conns_c,
                            snap_c,
                            code_c,
                            full_c,
                            host_pid_c,
                        )
                        .await;
                    });
                }
            })
        };

        // 2. 周期性广播房间状态（兼作房主心跳）+ 清理超时失联队友 + DHT 续期
        let cleanup_task = {
            let app_h = app.clone();
            let members = host_members.clone();
            let txs = peer_txs.clone();
            let conns = peer_conns.clone();
            let snap = self.state_snapshot.clone();
            let code = room_code.clone();
            let full = full_code.clone();
            let host_pid = self_peer_id.clone();

            tokio::spawn(async move {
                let mut ticks = 0u32;
                loop {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    ticks += 1;
                    let now = now_secs();
                    let mut removed = Vec::new();
                    {
                        let mut m = members.lock().await;
                        m.retain(|pid, (_, last_seen)| {
                            if *pid == host_pid {
                                return true;
                            }
                            if now.saturating_sub(*last_seen) > 15 {
                                removed.push(pid.clone());
                                false
                            } else {
                                true
                            }
                        });
                    }
                    if !removed.is_empty() {
                        let mut t = txs.lock().await;
                        let mut c = conns.lock().await;
                        for pid in removed {
                            t.remove(&pid);
                            c.remove(&pid);
                        }
                    }
                    // 每 5 秒定时向全体队员同步最新人数与成员状态
                    broadcast_host_state(&app_h, &members, &txs, &snap, &code, &full, &host_pid)
                        .await;

                    // 每 120 秒续期一次 DHT 记录
                    if ticks % 24 == 0 {
                        let c = code.clone();
                        let p = host_pid.clone();
                        tokio::spawn(async move {
                            let _ = publish_room_to_pkarr(&c, &p).await;
                        });
                    }
                }
            })
        };

        *self.session.lock().await = Some(ActiveSession {
            is_host: true,
            self_peer_id,
            room_code,
            full_code,
            endpoint,
            tasks: vec![accept_task, cleanup_task],
            peer_txs,
            peer_conns,
            host_members,
            host_tx: None,
            host_conn: None,
        });

        Ok(initial_state)
    }

    pub async fn join_room(
        &self,
        app: &AppHandle,
        code_or_full: String,
        player_name: String,
        map_id: String,
    ) -> Result<TeamRoomState, String> {
        self.leave_room(app).await;

        let trimmed = code_or_full.trim().to_string();
        if trimmed.is_empty() {
            return Err("请输入房间 Code".to_string());
        }

        let (room_code, host_id_str) = if let Some((c, id)) = trimmed.split_once(':') {
            (c.trim().to_uppercase(), id.trim().to_string())
        } else if trimmed.len() <= 10 {
            let code = trimmed.to_uppercase();
            let resolved = resolve_room_from_pkarr(&code).await?;
            (code, resolved)
        } else {
            ("DIRECT".to_string(), trimmed)
        };

        let host_endpoint_id: EndpointId = host_id_str
            .parse()
            .map_err(|_| "无效的房主节点 ID 或房间 Code".to_string())?;

        let endpoint = Endpoint::builder(presets::N0)
            .bind()
            .await
            .map_err(|e| format!("初始化 P2P 节点失败: {e}"))?;
        endpoint.online().await;

        let self_peer_id = endpoint.id().to_string();
        let conn = endpoint
            .connect(host_endpoint_id, ALPN)
            .await
            .map_err(|e| format!("连接房主失败，请确认房间 Code 是否正确: {e}"))?;

        let (mut send, recv) = conn
            .open_bi()
            .await
            .map_err(|e| format!("打开 P2P 数据流失败: {e}"))?;

        let (tx, mut rx) = mpsc::unbounded_channel::<WireMessage>();
        let _ = tx.send(WireMessage::Hello {
            peer_id: self_peer_id.clone(),
            player_name,
            map_id,
        });

        let full_code = format!("{room_code}:{host_id_str}");

        // 写入任务
        let write_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Ok(mut line) = serde_json::to_string(&msg) {
                    line.push('\n');
                    if send.write_all(line.as_bytes()).await.is_err() {
                        break;
                    }
                }
            }
        });

        // 心跳任务
        let ping_task = {
            let tx_c = tx.clone();
            let pid = self_peer_id.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    if tx_c.send(WireMessage::Ping { peer_id: pid.clone() }).is_err() {
                        break;
                    }
                }
            })
        };

        // 读取任务（同时监听 QUIC 连接关闭与 15 秒房主心跳超时）
        let read_task = {
            let app_h = app.clone();
            let snap = self.state_snapshot.clone();
            let sess_arc = self.session.clone();
            let self_pid = self_peer_id.clone();
            let full_c = full_code.clone();
            let conn_watch = conn.clone();

            tokio::spawn(async move {
                let mut reader = BufReader::new(recv);
                let mut line = String::new();
                loop {
                    let read_fut = reader.read_line(&mut line);
                    let res = tokio::select! {
                        r = tokio::time::timeout(Duration::from_secs(16), read_fut) => r,
                        _ = conn_watch.closed() => break,
                    };

                    let Ok(Ok(n)) = res else {
                        break;
                    };
                    if n == 0 {
                        break;
                    }
                    if let Ok(msg) = serde_json::from_str::<WireMessage>(line.trim()) {
                        match msg {
                            WireMessage::RoomState { room_code, members } => {
                                let state = TeamRoomState {
                                    in_room: true,
                                    is_host: false,
                                    mode: "p2p".to_string(),
                                    self_peer_id: self_pid.clone(),
                                    room_code,
                                    full_code: full_c.clone(),
                                    member_count: members.len(),
                                    members,
                                };
                                *snap.write().await = state.clone();
                                let _ = app_h.emit("team-state-updated", &state);
                            }
                            WireMessage::RoomClosed => break,
                            _ => {}
                        }
                    }
                    line.clear();
                }

                // 先更新前端状态并发送解散通知，再异步清理会话与关闭当前任务
                let empty = TeamRoomState::default();
                *snap.write().await = empty.clone();
                let _ = app_h.emit("team-state-updated", &empty);
                let _ = app_h.emit("team-room-closed", "房主已离开，房间已解散");

                if let Some(mut sess) = sess_arc.lock().await.take() {
                    let ep = sess.endpoint;
                    let tasks: Vec<_> = sess.tasks.drain(..).collect();
                    tokio::spawn(async move {
                        for t in tasks {
                            t.abort();
                        }
                        ep.close().await;
                    });
                }
            })
        };

        let initial_state = TeamRoomState {
            in_room: true,
            is_host: false,
            mode: "p2p".to_string(),
            self_peer_id: self_peer_id.clone(),
            room_code: room_code.clone(),
            full_code: full_code.clone(),
            member_count: 1,
            members: Vec::new(),
        };
        *self.state_snapshot.write().await = initial_state.clone();
        let _ = app.emit("team-state-updated", &initial_state);

        *self.session.lock().await = Some(ActiveSession {
            is_host: false,
            self_peer_id,
            room_code,
            full_code,
            endpoint,
            tasks: vec![write_task, ping_task, read_task],
            peer_txs: Arc::new(Mutex::new(HashMap::new())),
            peer_conns: Arc::new(Mutex::new(HashMap::new())),
            host_members: Arc::new(Mutex::new(HashMap::new())),
            host_tx: Some(tx),
            host_conn: Some(conn),
        });

        Ok(initial_state)
    }

    pub async fn sync_position(
        &self,
        app: &AppHandle,
        player_name: String,
        map_id: String,
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
    ) {
        let guard = self.session.lock().await;
        let Some(sess) = guard.as_ref() else {
            return;
        };

        if sess.is_host {
            let now = now_secs();
            {
                let mut m = sess.host_members.lock().await;
                let entry = m.entry(sess.self_peer_id.clone()).or_insert_with(|| {
                    (
                        TeamMemberInfo {
                            peer_id: sess.self_peer_id.clone(),
                            player_name: player_name.clone(),
                            map_id: map_id.clone(),
                            x: None,
                            y: None,
                            z: None,
                            updated_at: now,
                        },
                        now,
                    )
                });
                entry.0.player_name = player_name;
                entry.0.map_id = map_id;
                if x.is_some() && z.is_some() {
                    entry.0.x = x;
                    entry.0.y = y;
                    entry.0.z = z;
                }
                entry.0.updated_at = now;
                entry.1 = now;
            }
            broadcast_host_state(
                app,
                &sess.host_members,
                &sess.peer_txs,
                &self.state_snapshot,
                &sess.room_code,
                &sess.full_code,
                &sess.self_peer_id,
            )
            .await;
        } else if let Some(tx) = &sess.host_tx {
            if let (Some(px), Some(pz)) = (x, z) {
                let _ = tx.send(WireMessage::Pos {
                    peer_id: sess.self_peer_id.clone(),
                    player_name,
                    map_id,
                    x: px,
                    y: y.unwrap_or(0.0),
                    z: pz,
                });
            } else {
                let _ = tx.send(WireMessage::Hello {
                    peer_id: sess.self_peer_id.clone(),
                    player_name,
                    map_id,
                });
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_host_peer_stream(
    app: AppHandle,
    conn: Connection,
    mut send: iroh::endpoint::SendStream,
    recv: iroh::endpoint::RecvStream,
    members: Arc<Mutex<HashMap<String, (TeamMemberInfo, u64)>>>,
    txs: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<WireMessage>>>>,
    conns: Arc<Mutex<HashMap<String, Connection>>>,
    snap: Arc<RwLock<TeamRoomState>>,
    room_code: String,
    full_code: String,
    host_pid: String,
) {
    let (tx, mut rx) = mpsc::unbounded_channel::<WireMessage>();
    let write_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Ok(mut line) = serde_json::to_string(&msg) {
                line.push('\n');
                if send.write_all(line.as_bytes()).await.is_err() {
                    break;
                }
            }
        }
    });

    let mut reader = BufReader::new(recv);
    let mut line = String::new();
    let mut registered_peer_id: Option<String> = None;

    loop {
        let read_fut = reader.read_line(&mut line);
        let res = tokio::select! {
            r = read_fut => r,
            _ = conn.closed() => break,
        };
        let Ok(n) = res else {
            break;
        };
        if n == 0 {
            break;
        }

        if let Ok(msg) = serde_json::from_str::<WireMessage>(line.trim()) {
            let now = now_secs();
            match msg {
                WireMessage::Hello {
                    peer_id,
                    player_name,
                    map_id,
                } => {
                    registered_peer_id = Some(peer_id.clone());
                    txs.lock().await.insert(peer_id.clone(), tx.clone());
                    conns.lock().await.insert(peer_id.clone(), conn.clone());
                    {
                        let mut m = members.lock().await;
                        let entry = m.entry(peer_id.clone()).or_insert_with(|| {
                            (
                                TeamMemberInfo {
                                    peer_id: peer_id.clone(),
                                    player_name: player_name.clone(),
                                    map_id: map_id.clone(),
                                    x: None,
                                    y: None,
                                    z: None,
                                    updated_at: now,
                                },
                                now,
                            )
                        });
                        entry.0.player_name = player_name;
                        entry.0.map_id = map_id;
                        entry.0.updated_at = now;
                        entry.1 = now;
                    }
                    broadcast_host_state(&app, &members, &txs, &snap, &room_code, &full_code, &host_pid)
                        .await;
                }
                WireMessage::Pos {
                    peer_id,
                    player_name,
                    map_id,
                    x,
                    y,
                    z,
                } => {
                    registered_peer_id = Some(peer_id.clone());
                    txs.lock().await.entry(peer_id.clone()).or_insert_with(|| tx.clone());
                    conns.lock().await.entry(peer_id.clone()).or_insert_with(|| conn.clone());
                    {
                        let mut m = members.lock().await;
                        let entry = m.entry(peer_id.clone()).or_insert_with(|| {
                            (
                                TeamMemberInfo {
                                    peer_id: peer_id.clone(),
                                    player_name: player_name.clone(),
                                    map_id: map_id.clone(),
                                    x: Some(x),
                                    y: Some(y),
                                    z: Some(z),
                                    updated_at: now,
                                },
                                now,
                            )
                        });
                        entry.0.player_name = player_name;
                        entry.0.map_id = map_id;
                        entry.0.x = Some(x);
                        entry.0.y = Some(y);
                        entry.0.z = Some(z);
                        entry.0.updated_at = now;
                        entry.1 = now;
                    }
                    broadcast_host_state(&app, &members, &txs, &snap, &room_code, &full_code, &host_pid)
                        .await;
                }
                WireMessage::Ping { peer_id } => {
                    if let Some(entry) = members.lock().await.get_mut(&peer_id) {
                        entry.1 = now;
                    }
                }
                WireMessage::Leave { peer_id } => {
                    registered_peer_id = Some(peer_id);
                    break;
                }
                _ => {}
            }
        }
        line.clear();
    }

    write_task.abort();
    if let Some(pid) = registered_peer_id {
        txs.lock().await.remove(&pid);
        conns.lock().await.remove(&pid);
        members.lock().await.remove(&pid);
        broadcast_host_state(&app, &members, &txs, &snap, &room_code, &full_code, &host_pid).await;
    }
}

async fn broadcast_host_state(
    app: &AppHandle,
    members: &Arc<Mutex<HashMap<String, (TeamMemberInfo, u64)>>>,
    txs: &Arc<Mutex<HashMap<String, mpsc::UnboundedSender<WireMessage>>>>,
    snap: &Arc<RwLock<TeamRoomState>>,
    room_code: &str,
    full_code: &str,
    host_pid: &str,
) {
    let list: Vec<TeamMemberInfo> = {
        let m = members.lock().await;
        m.values().map(|(info, _)| info.clone()).collect()
    };

    let state = TeamRoomState {
        in_room: true,
        is_host: true,
        mode: "p2p".to_string(),
        self_peer_id: host_pid.to_string(),
        room_code: room_code.to_string(),
        full_code: full_code.to_string(),
        member_count: list.len(),
        members: list.clone(),
    };
    *snap.write().await = state.clone();
    let _ = app.emit("team-state-updated", &state);

    let msg = WireMessage::RoomState {
        room_code: room_code.to_string(),
        members: list,
    };
    let senders = txs.lock().await;
    for tx in senders.values() {
        let _ = tx.send(msg.clone());
    }
}

fn generate_room_code() -> String {
    let mut rng = rand::rng();
    (0..6)
        .map(|_| {
            let idx = rng.random_range(0..CODE_CHARSET.len());
            CODE_CHARSET[idx] as char
        })
        .collect()
}

fn derive_pkarr_keypair(room_code: &str) -> PkarrKeypair {
    let mut hasher = Sha256::new();
    hasher.update(b"tkfmap-room-v1:");
    hasher.update(room_code.trim().to_ascii_uppercase().as_bytes());
    let digest: [u8; 32] = hasher.finalize().into();
    PkarrKeypair::from_secret_key(&digest)
}

async fn publish_room_to_pkarr(room_code: &str, host_id: &str) -> Result<(), String> {
    let keypair = derive_pkarr_keypair(room_code);
    let packet = SignedPacket::builder()
        .txt(
            "_tkfmap".try_into().map_err(|e| format!("{e}"))?,
            host_id.try_into().map_err(|e| format!("{e}"))?,
            600,
        )
        .sign(&keypair)
        .map_err(|e| format!("签名失败: {e}"))?;

    let client = PkarrClient::builder()
        .build()
        .map_err(|e| format!("初始化 Pkarr 失败: {e}"))?;
    client
        .publish(&packet, None)
        .await
        .map_err(|e| format!("发布房间 Code 失败: {e}"))?;
    Ok(())
}

async fn resolve_room_from_pkarr(room_code: &str) -> Result<String, String> {
    let keypair = derive_pkarr_keypair(room_code);
    let client = PkarrClient::builder()
        .build()
        .map_err(|e| format!("初始化 Pkarr 失败: {e}"))?;

    let packet = client
        .resolve(&keypair.public_key())
        .await
        .ok_or_else(|| "未找到该房间 Code（可能房主网络屏蔽了 DHT，可改用完整邀请码）".to_string())?;

    for rr in packet.resource_records("_tkfmap") {
        if let pkarr::dns::rdata::RData::TXT(txt) = &rr.rdata {
            let raw = String::from_utf8_lossy(
                &txt.attributes()
                    .into_iter()
                    .flat_map(|(k, v)| {
                        let mut s = k.into_bytes();
                        if let Some(val) = v {
                            s.push(b'=');
                            s.extend(val.into_bytes());
                        }
                        s
                    })
                    .collect::<Vec<u8>>(),
            )
            .to_string();
            if !raw.is_empty() {
                return Ok(raw);
            }
        }
    }

    Err("房间记录解析失败，请改用完整邀请码".to_string())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
