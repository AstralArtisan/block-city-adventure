use std::net::{SocketAddr, UdpSocket};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::states::AppState;

pub const PVP_PORT: u16 = 3456;
const HEARTBEAT_INTERVAL_S: f64 = 5.0;
const DISCONNECT_TIMEOUT_S: f64 = 15.0;

#[derive(Resource, Debug, Clone)]
pub struct PvpNetConfig {
    pub mode: NetMode,
    pub host_ip: String,
    pub port: u16,
    pub nickname: String,
    pub room_id: String,
}

impl Default for PvpNetConfig {
    fn default() -> Self {
        Self {
            mode: NetMode::None,
            host_ip: String::new(),
            port: PVP_PORT,
            nickname: "Player".to_string(),
            room_id: "main".to_string(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum NetMode {
    #[default]
    None,
    Server,
    Host,
    Client,
}

#[derive(Resource, Debug, Default)]
pub struct PvpNetState {
    pub socket: Option<UdpSocket>,
    pub peer: Option<SocketAddr>,
    pub peers: [Option<SocketAddr>; 2],
    pub connected: bool,
    pub my_id: Option<u8>,
    pub last_input_from_client: Option<PvpInputMsg>,
    pub latest_inputs: [PvpInputMsg; 2],
    pub last_state: Option<PvpStateMsg>,
    pub fire_events: Vec<PvpFireMsg>,
    pub winner: Option<u8>,
    pub last_heard_s: f64,
    pub last_heard_by_id_s: [f64; 2],
    pub last_heartbeat_sent_s: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PvpMsg {
    Hello,
    Welcome { your_id: u8 },
    Input(PvpInputMsg),
    State(PvpStateMsg),
    Fire(PvpFireMsg),
    Result { winner: u8 },
    Heartbeat,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
pub struct PvpInputMsg {
    pub move_axis: (f32, f32),
    pub melee: bool,
    pub ranged: bool,
    pub dash: bool,
    pub aim: (f32, f32),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct PvpPlayerStateMsg {
    pub id: u8,
    pub pos: (f32, f32),
    pub hp: f32,
    pub lives: u8,
    pub melee_flash: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PvpStateMsg {
    pub tick: u32,
    pub p1: PvpPlayerStateMsg,
    pub p2: PvpPlayerStateMsg,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct PvpFireMsg {
    pub shooter_id: u8,
    pub origin: (f32, f32),
    pub dir: (f32, f32),
    pub melee: bool,
}

fn bind_socket(bind: &str) -> anyhow::Result<UdpSocket> {
    let socket = UdpSocket::bind(bind)?;
    socket.set_nonblocking(true)?;
    Ok(socket)
}

pub fn start_host_socket(state: &mut PvpNetState, port: u16) -> anyhow::Result<()> {
    let sock = bind_socket(&format!("0.0.0.0:{port}"))?;
    state.socket = Some(sock);
    state.peer = None;
    state.peers = [None, None];
    state.connected = false;
    state.my_id = Some(1);
    state.last_input_from_client = None;
    state.latest_inputs = [PvpInputMsg::default(); 2];
    state.last_state = None;
    state.fire_events.clear();
    state.winner = None;
    state.last_heard_s = 0.0;
    state.last_heard_by_id_s = [0.0, 0.0];
    state.last_heartbeat_sent_s = 0.0;
    Ok(())
}

pub fn start_server_socket(state: &mut PvpNetState, port: u16) -> anyhow::Result<()> {
    let sock = bind_socket(&format!("0.0.0.0:{port}"))?;
    state.socket = Some(sock);
    state.peer = None;
    state.peers = [None, None];
    state.connected = false;
    state.my_id = None;
    state.last_input_from_client = None;
    state.latest_inputs = [PvpInputMsg::default(); 2];
    state.last_state = None;
    state.fire_events.clear();
    state.winner = None;
    state.last_heard_s = 0.0;
    state.last_heard_by_id_s = [0.0, 0.0];
    state.last_heartbeat_sent_s = 0.0;
    Ok(())
}

pub fn start_client_socket(state: &mut PvpNetState) -> anyhow::Result<()> {
    let sock = bind_socket("0.0.0.0:0")?;
    state.socket = Some(sock);
    state.peer = None;
    state.peers = [None, None];
    state.connected = false;
    state.my_id = None;
    state.last_input_from_client = None;
    state.latest_inputs = [PvpInputMsg::default(); 2];
    state.last_state = None;
    state.fire_events.clear();
    state.winner = None;
    state.last_heard_s = 0.0;
    state.last_heard_by_id_s = [0.0, 0.0];
    state.last_heartbeat_sent_s = 0.0;
    Ok(())
}

fn try_send(state: &PvpNetState, msg: &PvpMsg) {
    let Some(sock) = state.socket.as_ref() else {
        return;
    };
    let Ok(payload) = bincode::serialize(msg) else {
        return;
    };
    if state.peers.iter().any(Option::is_some) {
        for peer in state.peers.iter().flatten() {
            let _ = sock.send_to(&payload, peer);
        }
    } else if let Some(peer) = state.peer {
        let _ = sock.send_to(&payload, peer);
    }
}

fn try_send_to(sock: &UdpSocket, peer: SocketAddr, msg: &PvpMsg) {
    let Ok(payload) = bincode::serialize(msg) else {
        return;
    };
    let _ = sock.send_to(&payload, peer);
}

fn peer_index_for_id(id: u8) -> Option<usize> {
    match id {
        1 => Some(0),
        2 => Some(1),
        _ => None,
    }
}

fn peer_id_for_addr(net: &PvpNetState, addr: SocketAddr) -> Option<u8> {
    net.peers
        .iter()
        .position(|peer| *peer == Some(addr))
        .map(|idx| (idx + 1) as u8)
}

fn assign_server_peer(net: &mut PvpNetState, addr: SocketAddr, now: f64) -> Option<u8> {
    if let Some(id) = peer_id_for_addr(net, addr) {
        if let Some(idx) = peer_index_for_id(id) {
            net.last_heard_by_id_s[idx] = now;
        }
        return Some(id);
    }

    let idx = net.peers.iter().position(Option::is_none)?;
    net.peers[idx] = Some(addr);
    net.last_heard_by_id_s[idx] = now;
    Some((idx + 1) as u8)
}

fn server_peers_ready(net: &PvpNetState) -> bool {
    net.peers.iter().all(Option::is_some)
}

fn clear_server_peer(net: &mut PvpNetState, idx: usize) {
    net.peers[idx] = None;
    net.latest_inputs[idx] = PvpInputMsg::default();
    net.last_heard_by_id_s[idx] = 0.0;
    net.connected = false;
}

fn reset_server_session(net: &mut PvpNetState) {
    net.peer = None;
    net.peers = [None, None];
    net.connected = false;
    net.my_id = None;
    net.latest_inputs = [PvpInputMsg::default(); 2];
    net.last_heard_by_id_s = [0.0, 0.0];
    net.clear_runtime();
}

pub fn pvp_net_tick_system(
    config: ResMut<PvpNetConfig>,
    mut net: ResMut<PvpNetState>,
    mut next: ResMut<NextState<AppState>>,
    state: Res<State<AppState>>,
    time: Res<Time>,
) {
    let Some(sock) = net.socket.as_ref().and_then(|s| s.try_clone().ok()) else {
        return;
    };

    // Client side: ensure we keep pinging Hello until connected.
    if *state.get() == AppState::PvpLobby
        && config.mode == NetMode::Client
        && !net.connected
        && let Some(peer) = net.peer
    {
        try_send_to(&sock, peer, &PvpMsg::Hello);
    }

    let now = time.elapsed_seconds_f64();
    if net.connected && now - net.last_heartbeat_sent_s >= HEARTBEAT_INTERVAL_S {
        try_send(&net, &PvpMsg::Heartbeat);
        net.last_heartbeat_sent_s = now;
    }
    if config.mode == NetMode::Server {
        let mut timed_out = false;
        for idx in 0..net.peers.len() {
            if net.peers[idx].is_some()
                && net.last_heard_by_id_s[idx] > 0.0
                && now - net.last_heard_by_id_s[idx] > DISCONNECT_TIMEOUT_S
            {
                clear_server_peer(&mut net, idx);
                timed_out = true;
            }
        }
        if timed_out && matches!(*state.get(), AppState::PvpGame) {
            reset_server_session(&mut net);
            next.set(AppState::PvpLobby);
        }
    } else if net.connected
        && net.last_heard_s > 0.0
        && now - net.last_heard_s > DISCONNECT_TIMEOUT_S
    {
        net.connected = false;
        net.my_id = (config.mode == NetMode::Host).then_some(1);
        net.last_input_from_client = None;
        if matches!(*state.get(), AppState::PvpGame) {
            next.set(AppState::PvpLobby);
        }
    }

    let mut buf = [0u8; 2048];
    while let Ok((n, from)) = sock.recv_from(&mut buf) {
        let Ok(msg) = bincode::deserialize::<PvpMsg>(&buf[..n]) else {
            continue;
        };
        net.last_heard_s = now;
        match msg {
            PvpMsg::Hello => {
                if config.mode == NetMode::Host {
                    net.peer = Some(from);
                    net.connected = true;
                    net.my_id = Some(1);
                    net.last_heard_s = now;
                    try_send_to(&sock, from, &PvpMsg::Welcome { your_id: 2 });
                    if *state.get() == AppState::PvpLobby {
                        next.set(AppState::PvpGame);
                    }
                } else if config.mode == NetMode::Server
                    && let Some(your_id) = assign_server_peer(&mut net, from, now)
                {
                    net.connected = server_peers_ready(&net);
                    try_send_to(&sock, from, &PvpMsg::Welcome { your_id });
                    if net.connected && *state.get() == AppState::PvpLobby {
                        next.set(AppState::PvpGame);
                    }
                }
            }
            PvpMsg::Welcome { your_id } => {
                if config.mode == NetMode::Client {
                    net.peer = Some(from);
                    net.connected = true;
                    net.my_id = Some(your_id);
                    net.last_heard_s = now;
                    if *state.get() == AppState::PvpLobby {
                        next.set(AppState::PvpGame);
                    }
                }
            }
            PvpMsg::Input(input) => {
                if config.mode == NetMode::Server {
                    if let Some(id) = peer_id_for_addr(&net, from)
                        && let Some(idx) = peer_index_for_id(id)
                    {
                        net.latest_inputs[idx] = input;
                        net.last_heard_by_id_s[idx] = now;
                    }
                } else {
                    // Host consumes this in pvp_host_simulation_system.
                    net.last_input_from_client = Some(input);
                }
            }
            PvpMsg::State(st) => {
                if config.mode == NetMode::Client {
                    net.last_state = Some(st);
                }
            }
            PvpMsg::Fire(ev) => {
                net.fire_events.push(ev);
            }
            PvpMsg::Result { winner } => {
                net.winner = Some(winner);
                if *state.get() != AppState::PvpResult {
                    next.set(AppState::PvpResult);
                }
            }
            PvpMsg::Heartbeat => {}
        }
    }

    // Keep local config sane.
    if config.mode == NetMode::None && net.socket.is_some() {
        net.socket = None;
    }
}

// Additional mutable field for host input capture.
impl PvpNetState {
    pub fn clear_runtime(&mut self) {
        self.last_input_from_client = None;
        self.latest_inputs = [PvpInputMsg::default(); 2];
        self.last_state = None;
        self.fire_events.clear();
        self.winner = None;
    }

    pub fn send_state(&self, st: &PvpStateMsg) {
        try_send(self, &PvpMsg::State(st.clone()));
    }

    pub fn send_fire(&self, fire: PvpFireMsg) {
        try_send(self, &PvpMsg::Fire(fire));
    }

    pub fn send_result(&self, winner: u8) {
        try_send(self, &PvpMsg::Result { winner });
    }

    pub fn send_input(&self, input: PvpInputMsg) {
        try_send(self, &PvpMsg::Input(input));
    }
}

// Host-only: last received client input (updated in net tick).
// Kept as a free field to avoid extra resources.
impl PvpNetState {
    pub(crate) fn client_input(&self) -> PvpInputMsg {
        self.last_input_from_client.unwrap_or_default()
    }

    pub(crate) fn input_for_player(&self, player_id: u8) -> PvpInputMsg {
        peer_index_for_id(player_id)
            .map(|idx| self.latest_inputs[idx])
            .unwrap_or_default()
    }
}

pub fn reset_pvp_network(config: &mut PvpNetConfig, net: &mut PvpNetState) {
    config.mode = NetMode::None;
    config.host_ip.clear();
    config.port = PVP_PORT;
    config.nickname = "Player".to_string();
    config.room_id = "main".to_string();
    net.socket = None;
    net.peer = None;
    net.peers = [None, None];
    net.connected = false;
    net.my_id = None;
    net.last_heard_by_id_s = [0.0, 0.0];
    net.clear_runtime();
}

// Hidden field (Rust requires it declared on struct; keep at end of file with Update File patch in-place).
