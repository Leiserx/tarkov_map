use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{accept_async, tungstenite::Message};

const CODE_CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const ROOM_IDLE_TIMEOUT_SECS: u64 = 60;
const MEMBER_IDLE_TIMEOUT_SECS: u64 = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TeamMemberInfo {
    peer_id: String,
    player_name: String,
    map_id: String,
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    updated_at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RoomSummary {
    room_id: String,
    room_name: String,
    host_name: String,
    member_count: usize,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ServerMsg {
    RoomList {
        rooms: Vec<RoomSummary>,
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

struct RoomMember {
    info: TeamMemberInfo,
    tx: mpsc::UnboundedSender<String>,
    last_seen: u64,
}

struct ServerRoom {
    room_id: String,
    room_name: String,
    passcode_hash: String,
    host_peer_id: String,
    host_name: String,
    last_activity: u64,
    members: HashMap<String, RoomMember>,
}

struct ServerState {
    rooms: HashMap<String, ServerRoom>,
    conn_to_room: HashMap<String, String>,
}

impl ServerState {
    fn new() -> Self {
        Self {
            rooms: HashMap::new(),
            conn_to_room: HashMap::new(),
        }
    }

    fn room_summaries(&self) -> Vec<RoomSummary> {
        self.rooms
            .values()
            .map(|r| RoomSummary {
                room_id: r.room_id.clone(),
                room_name: r.room_name.clone(),
                host_name: r.host_name.clone(),
                member_count: r.members.len(),
            })
            .collect()
    }
}

#[tokio::main]
async fn main() {
    let port = parse_port();
    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("无法绑定端口 {addr}: {e}"));

    println!("========================================");
    println!(" TarkovMap 联机服务端已启动");
    println!(" 监听地址: ws://{addr}");
    println!(" 房间无人心跳超时: {ROOM_IDLE_TIMEOUT_SECS} 秒");
    println!("========================================");

    let state = Arc::new(Mutex::new(ServerState::new()));

    // 后台定时清理超时房间与失联成员
    let sweeper_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            sweep_idle_rooms(&sweeper_state).await;
        }
    });

    while let Ok((stream, peer_addr)) = listener.accept().await {
        let state_c = state.clone();
        tokio::spawn(async move {
            handle_connection(stream, peer_addr.to_string(), state_c).await;
        });
    }
}

fn parse_port() -> u16 {
    let args: Vec<String> = env::args().collect();
    for i in 0..args.len() {
        if (args[i] == "--port" || args[i] == "-p") && i + 1 < args.len() {
            if let Ok(p) = args[i + 1].parse::<u16>() {
                return p;
            }
        }
    }
    if let Ok(p_str) = env::var("TKFMAP_PORT") {
        if let Ok(p) = p_str.parse::<u16>() {
            return p;
        }
    }
    8765
}

async fn handle_connection(stream: TcpStream, peer_addr: String, state: Arc<Mutex<ServerState>>) {
    let Ok(ws_stream) = accept_async(stream).await else {
        return;
    };
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    let conn_id = generate_id(10);
    println!("[连接] {peer_addr} -> {conn_id}");

    let write_task = tokio::spawn(async move {
        while let Some(text) = rx.recv().await {
            if ws_write.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = ws_read.next().await {
        let Message::Text(text) = msg else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<ClientMsg>(&text) else {
            send_msg(
                &tx,
                &ServerMsg::Error {
                    message: "无效的消息格式".to_string(),
                },
            );
            continue;
        };

        let now = now_secs();
        let mut st = state.lock().await;

        match parsed {
            ClientMsg::ListRooms => {
                send_msg(
                    &tx,
                    &ServerMsg::RoomList {
                        rooms: st.room_summaries(),
                    },
                );
            }
            ClientMsg::CreateRoom {
                room_name,
                passcode,
                player_name,
                map_id,
            } => {
                let code_clean = passcode.trim();
                if code_clean.is_empty() {
                    send_msg(
                        &tx,
                        &ServerMsg::Error {
                            message: "创建房间必须设置房间口令".to_string(),
                        },
                    );
                    continue;
                }

                remove_conn_from_current_room(&mut st, &conn_id);

                let room_id = generate_id(6);
                let display_name = if room_name.trim().is_empty() {
                    format!("{player_name} 的房间")
                } else {
                    room_name.trim().to_string()
                };

                let host_info = TeamMemberInfo {
                    peer_id: conn_id.clone(),
                    player_name: player_name.clone(),
                    map_id,
                    x: None,
                    y: None,
                    z: None,
                    updated_at: now,
                };

                let mut members = HashMap::new();
                members.insert(
                    conn_id.clone(),
                    RoomMember {
                        info: host_info.clone(),
                        tx: tx.clone(),
                        last_seen: now,
                    },
                );

                let room = ServerRoom {
                    room_id: room_id.clone(),
                    room_name: display_name.clone(),
                    passcode_hash: hash_passcode(code_clean),
                    host_peer_id: conn_id.clone(),
                    host_name: player_name,
                    last_activity: now,
                    members,
                };

                st.rooms.insert(room_id.clone(), room);
                st.conn_to_room.insert(conn_id.clone(), room_id.clone());

                println!("[建房] {room_id} ({display_name})");
                send_msg(
                    &tx,
                    &ServerMsg::Joined {
                        room_id,
                        room_name: display_name,
                        is_host: true,
                        self_peer_id: conn_id.clone(),
                        members: vec![host_info],
                    },
                );
            }
            ClientMsg::JoinRoom {
                room_id,
                passcode,
                player_name,
                map_id,
            } => {
                let rid = room_id.trim().to_uppercase();
                let valid = match st.rooms.get(&rid) {
                    Some(r) => r.passcode_hash == hash_passcode(passcode.trim()),
                    None => {
                        send_msg(
                            &tx,
                            &ServerMsg::Error {
                                message: "房间不存在或已关闭".to_string(),
                            },
                        );
                        continue;
                    }
                };

                if !valid {
                    send_msg(
                        &tx,
                        &ServerMsg::Error {
                            message: "房间口令错误".to_string(),
                        },
                    );
                    continue;
                }

                remove_conn_from_current_room(&mut st, &conn_id);

                if let Some(room) = st.rooms.get_mut(&rid) {
                    room.last_activity = now;
                    let member_info = TeamMemberInfo {
                        peer_id: conn_id.clone(),
                        player_name,
                        map_id,
                        x: None,
                        y: None,
                        z: None,
                        updated_at: now,
                    };
                    room.members.insert(
                        conn_id.clone(),
                        RoomMember {
                            info: member_info,
                            tx: tx.clone(),
                            last_seen: now,
                        },
                    );

                    let list: Vec<TeamMemberInfo> =
                        room.members.values().map(|m| m.info.clone()).collect();
                    send_msg(
                        &tx,
                        &ServerMsg::Joined {
                            room_id: room.room_id.clone(),
                            room_name: room.room_name.clone(),
                            is_host: false,
                            self_peer_id: conn_id.clone(),
                            members: list.clone(),
                        },
                    );
                    broadcast_room_state(room, &list);
                    st.conn_to_room.insert(conn_id.clone(), rid);
                }
            }
            ClientMsg::Pos {
                player_name,
                map_id,
                x,
                y,
                z,
            } => {
                if let Some(rid) = st.conn_to_room.get(&conn_id).cloned() {
                    if let Some(room) = st.rooms.get_mut(&rid) {
                        room.last_activity = now;
                        if let Some(member) = room.members.get_mut(&conn_id) {
                            member.last_seen = now;
                            member.info.player_name = player_name;
                            member.info.map_id = map_id;
                            if x.is_some() && z.is_some() {
                                member.info.x = x;
                                member.info.y = y;
                                member.info.z = z;
                            }
                            member.info.updated_at = now;
                        }
                        let list: Vec<TeamMemberInfo> =
                            room.members.values().map(|m| m.info.clone()).collect();
                        broadcast_room_state(room, &list);
                    }
                }
            }
            ClientMsg::Ping => {
                if let Some(rid) = st.conn_to_room.get(&conn_id).cloned() {
                    if let Some(room) = st.rooms.get_mut(&rid) {
                        room.last_activity = now;
                        if let Some(member) = room.members.get_mut(&conn_id) {
                            member.last_seen = now;
                        }
                    }
                }
                send_msg(&tx, &ServerMsg::Pong);
            }
            ClientMsg::LeaveRoom => {
                remove_conn_from_current_room(&mut st, &conn_id);
            }
        }
    }

    write_task.abort();
    let mut st = state.lock().await;
    remove_conn_from_current_room(&mut st, &conn_id);
    println!("[断开] {conn_id}");
}

fn remove_conn_from_current_room(st: &mut ServerState, conn_id: &str) {
    let Some(rid) = st.conn_to_room.remove(conn_id) else {
        return;
    };
    let Some(mut room) = st.rooms.remove(&rid) else {
        return;
    };

    if room.host_peer_id == conn_id {
        // 房主退出/断开 -> 解散房间并通知所有队员
        for (pid, member) in &room.members {
            if pid != conn_id {
                send_msg(
                    &member.tx,
                    &ServerMsg::RoomClosed {
                        reason: "房主已退出，房间已关闭".to_string(),
                    },
                );
                st.conn_to_room.remove(pid);
            }
        }
        println!("[解散] 房间 {rid} (房主退出)");
        return;
    }

    room.members.remove(conn_id);
    if room.members.is_empty() {
        println!("[关闭] 房间 {rid} (已空)");
        return;
    }

    let list: Vec<TeamMemberInfo> = room.members.values().map(|m| m.info.clone()).collect();
    broadcast_room_state(&room, &list);
    st.rooms.insert(rid, room);
}

async fn sweep_idle_rooms(state: &Arc<Mutex<ServerState>>) {
    let now = now_secs();
    let mut st = state.lock().await;
    let mut closed_rooms = Vec::new();

    for (rid, room) in st.rooms.iter_mut() {
        // 整个房间超过 60 秒无人心跳/活动 -> 自动关闭房间
        if now.saturating_sub(room.last_activity) > ROOM_IDLE_TIMEOUT_SECS {
            closed_rooms.push(rid.clone());
            continue;
        }

        // 清理单个失联队员
        let mut stale_peers = Vec::new();
        for (pid, member) in &room.members {
            if now.saturating_sub(member.last_seen) > MEMBER_IDLE_TIMEOUT_SECS {
                stale_peers.push(pid.clone());
            }
        }
        if stale_peers.contains(&room.host_peer_id) {
            closed_rooms.push(rid.clone());
            continue;
        }
        if !stale_peers.is_empty() {
            for pid in &stale_peers {
                room.members.remove(pid);
            }
            let list: Vec<TeamMemberInfo> = room.members.values().map(|m| m.info.clone()).collect();
            broadcast_room_state(room, &list);
        }
    }

    for rid in closed_rooms {
        if let Some(room) = st.rooms.remove(&rid) {
            for (pid, member) in &room.members {
                st.conn_to_room.remove(pid);
                send_msg(
                    &member.tx,
                    &ServerMsg::RoomClosed {
                        reason: "房间长时间无人心跳，已自动关闭".to_string(),
                    },
                );
            }
            println!("[超时关闭] 房间 {rid}");
        }
    }
}

fn broadcast_room_state(room: &ServerRoom, list: &[TeamMemberInfo]) {
    let msg = ServerMsg::RoomState {
        room_id: room.room_id.clone(),
        room_name: room.room_name.clone(),
        members: list.to_vec(),
    };
    for member in room.members.values() {
        send_msg(&member.tx, &msg);
    }
}

fn send_msg(tx: &mpsc::UnboundedSender<String>, msg: &ServerMsg) {
    if let Ok(json) = serde_json::to_string(msg) {
        let _ = tx.send(json);
    }
}

fn hash_passcode(passcode: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tkfmap-srv-pass-v1:");
    hasher.update(passcode.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn generate_id(len: usize) -> String {
    let mut rng = rand::rng();
    (0..len)
        .map(|_| {
            let idx = rng.random_range(0..CODE_CHARSET.len());
            CODE_CHARSET[idx] as char
        })
        .collect()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
