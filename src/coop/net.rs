use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use bevy::prelude::*;
use lightyear::client::config::{
    ClientConfig as LyClientConfig, NetcodeConfig as LyClientNetcodeConfig,
};
use lightyear::connection::client::{
    Authentication as LyAuthentication, NetConfig as LyClientNetConfig,
};
use lightyear::connection::server::NetConfig as LyServerNetConfig;
use lightyear::prelude::client::{
    ClientCommands as LyClientCommands, ClientPlugins as LyClientPlugins,
    ClientTransport as LyClientTransport, ConnectionManager as LyClientConnectionManager,
    InputManager as LyInputManager, IoConfig as LyClientIoConfig,
    NetworkingState as LyClientNetworkingState,
};
use lightyear::prelude::server::{
    ConnectEvent as LyServerConnectEvent, DisconnectEvent as LyServerDisconnectEvent,
    IoConfig as LyServerIoConfig, MessageEvent as LyServerMessageEvent,
    ServerCommands as LyServerCommands, ServerConfig as LyServerConfig,
    ServerPlugins as LyServerPlugins, ServerTransport as LyServerTransport,
};
use lightyear::prelude::server::{
    ControlledBy, InputEvent as LyServerInputEvent, Replicate, SyncTarget,
};
use lightyear::prelude::{
    AppChannelExt, AppComponentExt, AppMessageExt, Channel, ChannelDirection, ChannelMode,
    ChannelSettings, ClientId, CompressionConfig, InputPlugin as LyInputPlugin, NetworkTarget,
    ReliableSettings, Replicated, SharedConfig, TickConfig,
    client::ConnectEvent as LyClientConnectEvent,
    client::DisconnectEvent as LyClientDisconnectEvent,
};
use lightyear::server::config::NetcodeConfig as LyServerNetcodeConfig;
use lightyear::shared::replication::components::Controlled;
use serde::{Deserialize, Serialize};

use crate::core::input::PlayerInputState;
use crate::gameplay::combat::components::Projectile;
use crate::gameplay::enemy::components::{Enemy, EnemyKind};
use crate::gameplay::map::doors::Door;
use crate::gameplay::player::components::{
    AnimationState, FacingDirection, Gold, Health, MoveSpeed, Player,
};
use crate::states::AppState;

use super::components::{
    CoopDamageEvent, CoopDashVisualState, CoopInputState, CoopMeleeFlashState, CoopNetPosition,
    CoopNetRotation, CoopNetVelocity, CoopParticipant, CoopRewardSelectionGroup, CoopSessionState,
    GhostState, PlayerSlot,
};

pub const COOP_PORT: u16 = 3457;
const COOP_PROTOCOL_ID: u64 = 0x434F_4F50_5652_0002;
const COOP_PRIVATE_KEY: [u8; 32] = [
    0x19, 0x52, 0xA1, 0x44, 0x3E, 0x7B, 0xC0, 0x1D, 0x28, 0xB9, 0x66, 0xD3, 0x4A, 0xF5, 0x90, 0x2C,
    0x9E, 0x15, 0x6A, 0xB4, 0x07, 0xD8, 0x3F, 0x51, 0xC2, 0x6E, 0x14, 0x88, 0xAF, 0x30, 0x5D, 0x71,
];

pub const HOST_CLIENT_ID: u64 = 1;
pub const REMOTE_CLIENT_ID: u64 = 2;

#[derive(Channel)]
pub struct CoopCommandChannel;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CoopCommandMessage {
    SelectReward {
        slot: PlayerSlot,
        group: CoopRewardSelectionGroup,
        index: u8,
    },
    SelectRps {
        slot: PlayerSlot,
        choice: super::components::CoopRpsChoice,
    },
    BuyShopItem {
        slot: PlayerSlot,
        index: u8,
    },
    RefreshShop {
        slot: PlayerSlot,
    },
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum NetMode {
    #[default]
    None,
    Server,
    Host,
    Client,
}

#[derive(Resource, Debug, Clone)]
pub struct CoopNetConfig {
    pub mode: NetMode,
    pub host_ip: String,
    pub client_id: u64,
    pub port: u16,
    pub nickname: String,
    pub room_id: String,
}

impl Default for CoopNetConfig {
    fn default() -> Self {
        Self {
            mode: NetMode::None,
            host_ip: String::new(),
            client_id: 0,
            port: COOP_PORT,
            nickname: "Player".to_string(),
            room_id: "main".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoopExitDestination {
    Lobby,
    MainMenu,
    MultiplayerMenu,
}

#[derive(Debug, Clone)]
pub struct CoopExitRequest {
    pub destination: CoopExitDestination,
    pub notice: Option<String>,
    pub preserve_mode: bool,
}

#[derive(Resource, Debug, Default, Clone)]
pub struct CoopSessionFlow {
    pub pending_game_entry: bool,
    pub lobby_notice: String,
    pub pending_exit: Option<CoopExitRequest>,
}

#[derive(Resource, Debug, Default)]
pub struct CoopNetState {
    pub peer: Option<SocketAddr>,
    pub connected: bool,
    pub local_connected: bool,
    pub remote_connected: bool,
    pub server_started: bool,
    pub client_started: bool,
    pub local_client_id: Option<ClientId>,
    pub remote_client_id: Option<ClientId>,
    pub connected_clients: HashSet<ClientId>,
    pub latest_inputs: HashMap<ClientId, CoopInputState>,
    pub latest_input_ticks: HashMap<ClientId, u32>,
    pub host_frame_counter: u32,
    pub received_commands: Vec<(ClientId, CoopCommandMessage)>,
    pub pending_commands: Vec<CoopCommandMessage>,
}

#[derive(Resource, Debug, Default, Clone, Copy)]
struct PendingCoopInputEdges {
    attack_pressed: bool,
    ranged_pressed: bool,
    dash_pressed: bool,
    interact_pressed: bool,
    pause_pressed: bool,
    shop_pressed: bool,
}

impl PendingCoopInputEdges {
    fn clear(&mut self) {
        *self = Self::default();
    }

    fn capture(&mut self, input: &PlayerInputState) {
        self.attack_pressed |= input.attack_pressed;
        self.ranged_pressed |= input.ranged_pressed;
        self.dash_pressed |= input.dash_pressed;
        self.interact_pressed |= input.interact_pressed;
        self.pause_pressed |= input.pause_pressed;
        self.shop_pressed |= input.shop_pressed;
    }

    fn apply_to(self, input: &mut CoopInputState) {
        input.attack_pressed |= self.attack_pressed;
        input.ranged_pressed |= self.ranged_pressed;
        input.dash_pressed |= self.dash_pressed;
        input.interact_pressed |= self.interact_pressed;
        input.pause_pressed |= self.pause_pressed;
        input.shop_pressed |= self.shop_pressed;
        input.menu_confirm_pressed |= self.attack_pressed || self.interact_pressed;
        input.menu_cancel_pressed |= self.pause_pressed;
    }
}

pub struct CoopLightyearPlugin;

impl Plugin for CoopLightyearPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(LyClientPlugins {
            config: default_lightyear_client_config(),
        })
        .add_plugins(LyServerPlugins {
            config: default_lightyear_server_config(),
        })
        .add_plugins(LyInputPlugin::<CoopInputState>::default())
        .add_plugins(CoopProtocolPlugin)
        .init_resource::<CoopNetConfig>()
        .init_resource::<CoopNetState>()
        .init_resource::<CoopSessionFlow>()
        .init_resource::<PendingCoopInputEdges>()
        .add_systems(
            Update,
            latch_local_input_edges.after(crate::core::input::collect_player_input),
        )
        .add_systems(
            Update,
            (
                sync_coop_network_lifecycle,
                sync_client_connect_events,
                sync_server_connect_events,
                sync_server_disconnect_events,
                receive_coop_command_messages,
                flush_pending_client_commands,
                auto_advance_lobby_state,
            ),
        )
        .add_systems(FixedPreUpdate, buffer_local_inputs)
        .add_systems(FixedUpdate, capture_server_inputs);
    }
}

struct CoopProtocolPlugin;

impl Plugin for CoopProtocolPlugin {
    fn build(&self, app: &mut App) {
        app.add_channel::<CoopCommandChannel>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            send_frequency: Duration::default(),
            priority: 3.0,
        });
        app.register_message::<CoopCommandMessage>(ChannelDirection::ClientToServer);
        app.register_message::<CoopDamageEvent>(ChannelDirection::ServerToClient);

        app.register_component::<Player>(ChannelDirection::ServerToClient);
        app.register_component::<Health>(ChannelDirection::ServerToClient);
        app.register_component::<Gold>(ChannelDirection::ServerToClient);
        app.register_component::<MoveSpeed>(ChannelDirection::ServerToClient);
        app.register_component::<FacingDirection>(ChannelDirection::ServerToClient);
        app.register_component::<AnimationState>(ChannelDirection::ServerToClient);
        app.register_component::<CoopParticipant>(ChannelDirection::ServerToClient);
        app.register_component::<PlayerSlot>(ChannelDirection::ServerToClient);
        app.register_component::<GhostState>(ChannelDirection::ServerToClient);
        app.register_component::<CoopNetPosition>(ChannelDirection::ServerToClient);
        app.register_component::<CoopNetVelocity>(ChannelDirection::ServerToClient);
        app.register_component::<CoopNetRotation>(ChannelDirection::ServerToClient);
        app.register_component::<CoopMeleeFlashState>(ChannelDirection::ServerToClient);
        app.register_component::<CoopDashVisualState>(ChannelDirection::ServerToClient);
        app.register_component::<CoopSessionState>(ChannelDirection::ServerToClient);
        app.register_component::<Enemy>(ChannelDirection::ServerToClient);
        app.register_component::<EnemyKind>(ChannelDirection::ServerToClient);
        app.register_component::<Projectile>(ChannelDirection::ServerToClient);
        app.register_component::<Door>(ChannelDirection::ServerToClient);
    }
}

pub fn is_coop_authority(config: Res<CoopNetConfig>) -> bool {
    matches!(config.mode, NetMode::Host | NetMode::Server)
}

#[allow(dead_code)]
pub fn local_client_id(mode: NetMode) -> ClientId {
    match mode {
        NetMode::Host => ClientId::Netcode(HOST_CLIENT_ID),
        NetMode::Server => ClientId::Netcode(HOST_CLIENT_ID),
        NetMode::Client => ClientId::Netcode(REMOTE_CLIENT_ID),
        NetMode::None => ClientId::Netcode(HOST_CLIENT_ID),
    }
}

fn local_player_slot(config: &CoopNetConfig) -> PlayerSlot {
    slot_for_client_id(configured_client_id(config)).unwrap_or(PlayerSlot::P2)
}

fn client_game_entry_ready(session_ready: bool, door_ready: bool, local_slot_ready: bool) -> bool {
    session_ready && door_ready && local_slot_ready
}

pub fn remote_client_id() -> ClientId {
    ClientId::Netcode(REMOTE_CLIENT_ID)
}

pub fn host_client_id() -> ClientId {
    ClientId::Netcode(HOST_CLIENT_ID)
}

pub fn configured_client_id(config: &CoopNetConfig) -> u64 {
    if config.client_id != 0 {
        config.client_id
    } else {
        match config.mode {
            NetMode::Host => HOST_CLIENT_ID,
            NetMode::Client => REMOTE_CLIENT_ID,
            NetMode::Server | NetMode::None => HOST_CLIENT_ID,
        }
    }
}

pub fn client_id_for_slot(slot: PlayerSlot) -> ClientId {
    match slot {
        PlayerSlot::P1 => host_client_id(),
        PlayerSlot::P2 => remote_client_id(),
    }
}

pub fn slot_for_client_id(client_id: u64) -> Option<PlayerSlot> {
    match client_id {
        HOST_CLIENT_ID => Some(PlayerSlot::P1),
        REMOTE_CLIENT_ID => Some(PlayerSlot::P2),
        _ => None,
    }
}

fn required_player_clients_connected(net: &CoopNetState) -> bool {
    net.connected_clients.contains(&host_client_id())
        && net.connected_clients.contains(&remote_client_id())
}

pub fn build_player_replication(controlled_by: ClientId) -> Replicate {
    Replicate {
        target: lightyear::prelude::ReplicationTarget {
            target: NetworkTarget::All,
        },
        sync: SyncTarget {
            prediction: NetworkTarget::Single(controlled_by),
            interpolation: NetworkTarget::AllExceptSingle(controlled_by),
        },
        controlled_by: ControlledBy {
            target: NetworkTarget::Single(controlled_by),
            lifetime: lightyear::prelude::server::Lifetime::SessionBased,
        },
        ..default()
    }
}

pub fn build_replicate_all() -> Replicate {
    Replicate {
        target: lightyear::prelude::ReplicationTarget {
            target: NetworkTarget::All,
        },
        ..default()
    }
}

pub fn take_received_commands(net: &mut CoopNetState) -> Vec<(ClientId, CoopCommandMessage)> {
    std::mem::take(&mut net.received_commands)
}

pub fn latest_input_for(net: &CoopNetState, client_id: ClientId) -> CoopInputState {
    net.latest_inputs
        .get(&client_id)
        .copied()
        .unwrap_or_default()
}

pub fn queue_command(net: &mut CoopNetState, command: CoopCommandMessage) {
    net.pending_commands.push(command);
}

pub fn queue_exit_request(flow: &mut CoopSessionFlow, request: CoopExitRequest) {
    if flow.pending_exit.is_none() {
        flow.pending_exit = Some(request);
    }
}

pub fn take_exit_request(flow: &mut CoopSessionFlow) -> Option<CoopExitRequest> {
    flow.pending_exit.take()
}

pub fn reset_coop_flow(flow: &mut CoopSessionFlow) {
    flow.pending_game_entry = false;
    flow.lobby_notice.clear();
    flow.pending_exit = None;
}

pub fn clear_coop_network_runtime(net: &mut CoopNetState) {
    net.peer = None;
    net.connected = false;
    net.local_connected = false;
    net.remote_connected = false;
    net.server_started = false;
    net.client_started = false;
    net.local_client_id = None;
    net.remote_client_id = None;
    net.connected_clients.clear();
    net.latest_inputs.clear();
    net.latest_input_ticks.clear();
    net.received_commands.clear();
    net.pending_commands.clear();
}

pub fn reset_coop_network(config: &mut CoopNetConfig, net: &mut CoopNetState) {
    config.mode = NetMode::None;
    config.host_ip.clear();
    config.client_id = 0;
    config.port = COOP_PORT;
    config.nickname = "Player".to_string();
    config.room_id = "main".to_string();
    clear_coop_network_runtime(net);
}

pub fn validate_coop_host_ip(host_ip: &str) -> Result<Ipv4Addr, String> {
    let trimmed = host_ip.trim();
    if trimmed.is_empty() {
        return Err("请输入房主局域网 IPv4 地址。".to_string());
    }
    if trimmed.contains(':') {
        return Err(format!("联机地址只接受裸 IPv4，端口固定为 {}。", COOP_PORT));
    }
    trimmed
        .parse::<Ipv4Addr>()
        .map_err(|_| "请输入有效的局域网 IPv4 地址，例如 192.168.1.6。".to_string())
}

pub fn normalize_coop_host_ip(host_ip: &str) -> Result<String, String> {
    validate_coop_host_ip(host_ip).map(|ip| ip.to_string())
}

pub fn begin_coop_lobby_session(
    config: &CoopNetConfig,
    net: &mut CoopNetState,
    flow: &mut CoopSessionFlow,
) -> Result<(), String> {
    flow.pending_game_entry = true;
    flow.pending_exit = None;
    flow.lobby_notice.clear();

    match config.mode {
        NetMode::Host => start_host_socket(net).map_err(|err| err.to_string()),
        NetMode::Server => start_server_socket(net).map_err(|err| err.to_string()),
        NetMode::Client => {
            let ip = validate_coop_host_ip(&config.host_ip)?;
            start_client_socket(net, configured_client_id(config))
                .map_err(|err| err.to_string())?;
            net.peer = Some(SocketAddr::new(ip.into(), config.port));
            Ok(())
        }
        NetMode::None => Err("尚未选择联机模式。".to_string()),
    }
}

pub fn start_host_socket(state: &mut CoopNetState) -> anyhow::Result<()> {
    state.peer = Some(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), COOP_PORT));
    state.connected = false;
    state.local_connected = false;
    state.remote_connected = false;
    state.server_started = false;
    state.client_started = false;
    state.local_client_id = Some(host_client_id());
    state.remote_client_id = Some(remote_client_id());
    state.connected_clients.clear();
    state.latest_inputs.clear();
    state.latest_input_ticks.clear();
    state.host_frame_counter = 0;
    state.received_commands.clear();
    state.pending_commands.clear();
    Ok(())
}

pub fn start_server_socket(state: &mut CoopNetState) -> anyhow::Result<()> {
    state.peer = None;
    state.connected = false;
    state.local_connected = false;
    state.remote_connected = false;
    state.server_started = false;
    state.client_started = false;
    state.local_client_id = None;
    state.remote_client_id = None;
    state.connected_clients.clear();
    state.latest_inputs.clear();
    state.latest_input_ticks.clear();
    state.host_frame_counter = 0;
    state.received_commands.clear();
    state.pending_commands.clear();
    Ok(())
}

pub fn start_client_socket(state: &mut CoopNetState, client_id: u64) -> anyhow::Result<()> {
    state.connected = false;
    state.local_connected = false;
    state.remote_connected = false;
    state.server_started = false;
    state.client_started = false;
    state.local_client_id = Some(ClientId::Netcode(client_id));
    state.remote_client_id = Some(host_client_id());
    state.connected_clients.clear();
    state.latest_inputs.clear();
    state.latest_input_ticks.clear();
    state.host_frame_counter = 0;
    state.received_commands.clear();
    state.pending_commands.clear();
    Ok(())
}

fn coop_shared_config() -> SharedConfig {
    SharedConfig {
        // 与 tick 频率对齐（60 Hz），避免 tick/replication 不对齐导致内部缓冲区漂移
        server_replication_send_interval: Duration::from_secs_f64(1.0 / 60.0),
        tick: TickConfig::new(Duration::from_secs_f64(1.0 / 60.0)),
        ..default()
    }
}

fn build_lightyear_client_net_config(
    host_ip: &str,
    client_id: u64,
    port: u16,
) -> Result<LyClientNetConfig, String> {
    let server_addr = SocketAddr::new(validate_coop_host_ip(host_ip)?.into(), port);
    Ok(LyClientNetConfig::Netcode {
        auth: LyAuthentication::Manual {
            server_addr,
            client_id,
            private_key: COOP_PRIVATE_KEY,
            protocol_id: COOP_PROTOCOL_ID,
        },
        config: LyClientNetcodeConfig::default(),
        io: LyClientIoConfig {
            transport: LyClientTransport::UdpSocket(SocketAddr::new(
                Ipv4Addr::UNSPECIFIED.into(),
                0,
            )),
            conditioner: None,
            compression: CompressionConfig::None,
        },
    })
}

fn build_lightyear_server_config(port: u16) -> LyServerConfig {
    LyServerConfig {
        shared: coop_shared_config(),
        net: vec![LyServerNetConfig::Netcode {
            config: LyServerNetcodeConfig::default()
                .with_protocol_id(COOP_PROTOCOL_ID)
                .with_key(COOP_PRIVATE_KEY),
            io: LyServerIoConfig {
                transport: LyServerTransport::UdpSocket(SocketAddr::new(
                    Ipv4Addr::UNSPECIFIED.into(),
                    port,
                )),
                conditioner: None,
                compression: CompressionConfig::None,
            },
        }],
        ..default()
    }
}

fn default_lightyear_client_config() -> LyClientConfig {
    LyClientConfig {
        shared: coop_shared_config(),
        net: build_lightyear_client_net_config("127.0.0.1", REMOTE_CLIENT_ID, COOP_PORT)
            .expect("loopback IPv4 must be valid"),
        ..default()
    }
}

fn default_lightyear_server_config() -> LyServerConfig {
    build_lightyear_server_config(COOP_PORT)
}

fn sync_coop_network_lifecycle(
    mut commands: Commands,
    state: Res<State<AppState>>,
    config: Res<CoopNetConfig>,
    flow: Res<CoopSessionFlow>,
    mut net: ResMut<CoopNetState>,
    mut client_config: ResMut<LyClientConfig>,
    mut server_config: ResMut<LyServerConfig>,
    client_state: Res<State<LyClientNetworkingState>>,
) {
    let wants_lobby_runtime = *state.get() == AppState::CoopLobby && flow.pending_game_entry;
    let wants_game_runtime = *state.get() == AppState::CoopGame;
    let wants_authority_runtime = matches!(config.mode, NetMode::Host | NetMode::Server)
        && (wants_lobby_runtime || wants_game_runtime);
    let wants_client_runtime = matches!(config.mode, NetMode::Host | NetMode::Client)
        && (wants_lobby_runtime || wants_game_runtime);

    if wants_authority_runtime && !net.server_started {
        *server_config = build_lightyear_server_config(config.port);
        commands.start_server();
        net.server_started = true;
    } else if !wants_authority_runtime && net.server_started {
        commands.stop_server();
        net.server_started = false;
        net.remote_connected = false;
        net.local_connected = false;
        net.connected_clients.clear();
    }

    if wants_client_runtime && !net.client_started {
        let host_ip = if config.mode == NetMode::Host {
            "127.0.0.1".to_string()
        } else {
            config.host_ip.clone()
        };
        let client_id = configured_client_id(&config);
        match build_lightyear_client_net_config(&host_ip, client_id, config.port) {
            Ok(net_config) => {
                client_config.net = net_config;
                commands.connect_client();
                net.client_started = true;
            }
            Err(err) => {
                warn!("Unable to start coop client runtime: {err}");
                net.client_started = false;
                net.connected = false;
            }
        }
    } else if !wants_client_runtime && net.client_started {
        commands.disconnect_client();
        net.client_started = false;
        net.connected = false;
    }

    if *client_state.get() == LyClientNetworkingState::Disconnected && !wants_client_runtime {
        net.connected = false;
    }
}

fn sync_client_connect_events(
    mut connect_events: EventReader<LyClientConnectEvent>,
    mut disconnect_events: EventReader<LyClientDisconnectEvent>,
    state: Res<State<AppState>>,
    config: Res<CoopNetConfig>,
    mut net: ResMut<CoopNetState>,
    mut flow: ResMut<CoopSessionFlow>,
    session_q: Query<&CoopSessionState, With<Replicated>>,
) {
    for event in connect_events.read() {
        net.connected = true;
        net.local_connected = true;
        net.local_client_id = Some(event.client_id());
        if config.mode == NetMode::Client {
            net.remote_client_id = Some(host_client_id());
        }
    }

    for _ in disconnect_events.read() {
        net.connected = false;
        net.local_connected = false;
        net.latest_inputs.clear();
        net.pending_commands.clear();

        if config.mode == NetMode::Client
            && matches!(*state.get(), AppState::CoopLobby | AppState::CoopGame)
        {
            let ended_match = *state.get() == AppState::CoopGame
                && session_q
                    .get_single()
                    .map(|session| session.match_over)
                    .unwrap_or(false);
            queue_exit_request(
                &mut flow,
                CoopExitRequest {
                    destination: if ended_match {
                        CoopExitDestination::MainMenu
                    } else {
                        CoopExitDestination::Lobby
                    },
                    notice: (!ended_match)
                        .then_some("与房主断开连接，已退出合作会话。".to_string()),
                    preserve_mode: !ended_match,
                },
            );
        }
    }
}

fn sync_server_connect_events(
    mut connect_events: EventReader<LyServerConnectEvent>,
    config: Res<CoopNetConfig>,
    mut net: ResMut<CoopNetState>,
) {
    if !matches!(config.mode, NetMode::Host | NetMode::Server) {
        return;
    }

    for event in connect_events.read() {
        let client_id = event.client_id;
        net.connected_clients.insert(client_id);
        match config.mode {
            NetMode::Host => {
                if client_id == host_client_id() {
                    net.local_connected = true;
                    net.local_client_id = Some(client_id);
                }
                if client_id == remote_client_id() {
                    net.remote_connected = true;
                    net.remote_client_id = Some(client_id);
                }
            }
            NetMode::Server => {
                net.local_connected = net.connected_clients.contains(&host_client_id());
                net.remote_connected = net.connected_clients.contains(&remote_client_id());
                net.local_client_id = None;
                net.remote_client_id = None;
            }
            NetMode::Client | NetMode::None => {}
        }
    }
}

fn sync_server_disconnect_events(
    mut disconnect_events: EventReader<LyServerDisconnectEvent>,
    config: Res<CoopNetConfig>,
    mut net: ResMut<CoopNetState>,
) {
    if !matches!(config.mode, NetMode::Host | NetMode::Server) {
        return;
    }

    for event in disconnect_events.read() {
        net.connected_clients.remove(&event.client_id);
        net.latest_inputs.remove(&event.client_id);
        match config.mode {
            NetMode::Host => {
                if Some(event.client_id) == net.remote_client_id {
                    net.remote_connected = false;
                }
                if Some(event.client_id) == net.local_client_id {
                    net.local_connected = false;
                }
            }
            NetMode::Server => {
                net.local_connected = net.connected_clients.contains(&host_client_id());
                net.remote_connected = net.connected_clients.contains(&remote_client_id());
            }
            NetMode::Client | NetMode::None => {}
        }
    }
}

fn auto_advance_lobby_state(
    state: Res<State<AppState>>,
    config: Res<CoopNetConfig>,
    flow: Res<CoopSessionFlow>,
    net: Res<CoopNetState>,
    session_q: Query<(), (With<CoopSessionState>, With<Replicated>)>,
    door_q: Query<(), (With<Door>, With<Replicated>)>,
    player_q: Query<(&PlayerSlot, Option<&Controlled>), (With<Player>, With<Replicated>)>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if *state.get() != AppState::CoopLobby || !flow.pending_game_entry {
        return;
    }

    match config.mode {
        NetMode::Host if net.local_connected && net.remote_connected => {
            next_state.set(AppState::CoopGame);
        }
        NetMode::Server if required_player_clients_connected(&net) => {
            next_state.set(AppState::CoopGame);
        }
        NetMode::Client if net.connected => {
            let local_slot = local_player_slot(&config);
            let local_slot_ready = player_q
                .iter()
                .any(|(slot, controlled)| *slot == local_slot && controlled.is_some());
            let session_ready = session_q.iter().next().is_some();
            let door_ready = door_q.iter().next().is_some();
            if client_game_entry_ready(session_ready, door_ready, local_slot_ready) {
                next_state.set(AppState::CoopGame);
            }
        }
        NetMode::None => {}
        _ => {}
    }
}

fn latch_local_input_edges(
    input: Res<PlayerInputState>,
    config: Res<CoopNetConfig>,
    state: Res<State<AppState>>,
    mut pending: ResMut<PendingCoopInputEdges>,
) {
    if !matches!(config.mode, NetMode::Host | NetMode::Client)
        || !matches!(*state.get(), AppState::CoopLobby | AppState::CoopGame)
    {
        pending.clear();
        return;
    }

    pending.capture(&input);
}

fn buffer_local_inputs(
    input: Res<PlayerInputState>,
    config: Res<CoopNetConfig>,
    state: Res<State<AppState>>,
    tick_manager: Res<lightyear::prelude::TickManager>,
    mut pending: ResMut<PendingCoopInputEdges>,
    mut input_manager: ResMut<LyInputManager<CoopInputState>>,
) {
    if !matches!(config.mode, NetMode::Host | NetMode::Client)
        || !matches!(*state.get(), AppState::CoopLobby | AppState::CoopGame)
    {
        pending.clear();
        return;
    }

    let mut buffered_input = build_input_state(&input);
    pending.apply_to(&mut buffered_input);
    input_manager.add_input(buffered_input, tick_manager.tick());
    pending.clear();
}

fn capture_server_inputs(
    config: Res<CoopNetConfig>,
    mut input_events: EventReader<LyServerInputEvent<CoopInputState>>,
    mut net: ResMut<CoopNetState>,
) {
    if !matches!(config.mode, NetMode::Host | NetMode::Server) {
        input_events.clear();
        return;
    }

    for event in input_events.read() {
        if let Some(input) = event.input() {
            let cid = *event.context();
            let frame = net.host_frame_counter;
            net.latest_input_ticks.insert(cid, frame);
            // 使用 merge_incoming 而非 insert：
            // 当帧率低于 60fps 时 FixedUpdate 在同一帧内执行多次，
            // 后面 tick 的 false 会覆盖前面 tick 的 true，导致冲刺/交互丢失。
            // OR 累积边缘事件确保任意 tick 内的按键都不会丢失。
            net.latest_inputs
                .entry(cid)
                .and_modify(|prev| prev.merge_incoming(input))
                .or_insert(*input);
        }
    }
}

fn receive_coop_command_messages(
    config: Res<CoopNetConfig>,
    mut events: EventReader<LyServerMessageEvent<CoopCommandMessage>>,
    mut net: ResMut<CoopNetState>,
) {
    if !matches!(config.mode, NetMode::Host | NetMode::Server) {
        events.clear();
        return;
    }

    for event in events.read() {
        net.received_commands
            .push((*event.context(), event.message().clone()));
    }
}

fn flush_pending_client_commands(
    config: Res<CoopNetConfig>,
    mut net: ResMut<CoopNetState>,
    mut connection: ResMut<LyClientConnectionManager>,
) {
    if !matches!(config.mode, NetMode::Host | NetMode::Client)
        || net.pending_commands.is_empty()
        || !net.connected
    {
        return;
    }

    for mut command in std::mem::take(&mut net.pending_commands) {
        let _ = connection.send_message::<CoopCommandChannel, _>(&mut command);
    }
}

pub fn build_input_state(input: &PlayerInputState) -> CoopInputState {
    CoopInputState {
        move_axis: input.move_axis,
        aim_world: input.aim_world,
        attack_pressed: input.attack_pressed,
        attack_held: input.attack_held,
        ranged_pressed: input.ranged_pressed,
        ranged_held: input.ranged_held,
        dash_pressed: input.dash_pressed,
        interact_pressed: input.interact_pressed,
        pause_pressed: input.pause_pressed,
        shop_pressed: input.shop_pressed,
        menu_confirm_pressed: input.attack_pressed || input.interact_pressed,
        menu_cancel_pressed: input.pause_pressed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use lightyear::prelude::Replicated;
    use lightyear::shared::replication::components::Controlled;

    use crate::gameplay::map::doors::Door;
    use crate::gameplay::map::room::{Direction, RoomId};
    use crate::gameplay::player::components::Player;

    #[test]
    fn bare_ipv4_is_accepted_and_normalized() {
        assert_eq!(
            validate_coop_host_ip("192.168.1.6").unwrap(),
            Ipv4Addr::new(192, 168, 1, 6)
        );
        assert_eq!(
            normalize_coop_host_ip(" 10.0.0.25 ").unwrap(),
            "10.0.0.25".to_string()
        );
    }

    #[test]
    fn host_port_and_invalid_text_are_rejected() {
        assert!(validate_coop_host_ip("192.168.1.6:3457").is_err());
        assert!(validate_coop_host_ip("localhost").is_err());
        assert!(validate_coop_host_ip("").is_err());
    }

    #[test]
    fn client_lobby_waits_for_full_replicated_world_before_entering_game() {
        let mut world = World::new();
        world.insert_resource(State::new(AppState::CoopLobby));
        world.insert_resource(NextState::<AppState>::default());
        world.insert_resource(CoopNetConfig {
            mode: NetMode::Client,
            host_ip: "127.0.0.1".to_string(),
            client_id: REMOTE_CLIENT_ID,
            ..default()
        });
        world.insert_resource(CoopSessionFlow {
            pending_game_entry: true,
            ..default()
        });
        world.insert_resource(CoopNetState {
            connected: true,
            ..default()
        });

        world.run_system_once(auto_advance_lobby_state);

        assert!(matches!(
            world.resource::<NextState<AppState>>(),
            NextState::Unchanged
        ));
    }

    #[test]
    fn client_lobby_enters_game_once_session_player_and_door_are_ready() {
        let mut world = World::new();
        world.insert_resource(State::new(AppState::CoopLobby));
        world.insert_resource(NextState::<AppState>::default());
        world.insert_resource(CoopNetConfig {
            mode: NetMode::Client,
            host_ip: "127.0.0.1".to_string(),
            client_id: REMOTE_CLIENT_ID,
            ..default()
        });
        world.insert_resource(CoopSessionFlow {
            pending_game_entry: true,
            ..default()
        });
        world.insert_resource(CoopNetState {
            connected: true,
            ..default()
        });

        world.spawn((CoopSessionState::default(), Replicated { from: None }));
        world.spawn((
            Door {
                to: RoomId(1),
                dir: Direction::Right,
                active: true,
            },
            Replicated { from: None },
        ));
        world.spawn((
            Player,
            PlayerSlot::P2,
            Controlled,
            Replicated { from: None },
        ));

        world.run_system_once(auto_advance_lobby_state);

        assert!(matches!(
            world.resource::<NextState<AppState>>(),
            NextState::Pending(AppState::CoopGame)
        ));
    }
}
