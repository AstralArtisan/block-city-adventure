use std::env;

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowMode, WindowPosition};

use crate::coop::net::{
    CoopNetConfig, CoopNetState, CoopSessionFlow, NetMode as CoopNetMode, begin_coop_lobby_session,
    normalize_coop_host_ip, reset_coop_network,
};
use crate::gameplay::enemy::systems::EnemySpawnCount;
use crate::gameplay::progression::floor::FloorNumber;
use crate::pvp::net::{
    NetMode as PvpNetMode, PvpNetConfig, PvpNetState, reset_pvp_network,
    start_client_socket as start_pvp_client_socket, start_host_socket as start_pvp_host_socket,
    start_server_socket as start_pvp_server_socket,
};
use crate::states::AppState;

pub struct LocalDebugPlugin;

impl Plugin for LocalDebugPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LocalNetDebugConfig::from_env())
            .init_resource::<LocalNetDebugRuntime>()
            .add_systems(Update, (apply_local_debug_window, auto_boot_local_debug));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalNetDebugMode {
    Coop,
    Pvp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalNetDebugRole {
    Server,
    Host,
    Client,
}

impl LocalNetDebugRole {
    fn label(self) -> &'static str {
        match self {
            Self::Server => "Server",
            Self::Host => "Host",
            Self::Client => "Client",
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct LocalNetDebugConfig {
    pub enabled: bool,
    pub mode: Option<LocalNetDebugMode>,
    pub role: Option<LocalNetDebugRole>,
    pub host_ip: String,
    pub client_id: u64,
    pub port: Option<u16>,
    pub save_suffix: Option<String>,
    pub title_suffix: Option<String>,
    pub window_pos: Option<IVec2>,
}

impl Default for LocalNetDebugConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: None,
            role: None,
            host_ip: "127.0.0.1".to_string(),
            client_id: 0,
            port: None,
            save_suffix: None,
            title_suffix: None,
            window_pos: None,
        }
    }
}

impl LocalNetDebugConfig {
    pub fn from_env() -> Self {
        let cli = CliNetBootConfig::from_args();
        let raw_mode = cli.mode.or_else(|| {
            env::var("LOCAL_NET_DEBUG_MODE")
                .ok()
                .and_then(parse_net_mode)
        });
        let raw_role = cli.role.or_else(|| {
            env::var("LOCAL_NET_DEBUG_ROLE")
                .ok()
                .and_then(parse_net_role)
        });
        let enabled =
            cli.enabled || env_flag("LOCAL_NET_DEBUG") || raw_mode.is_some() || raw_role.is_some();
        if !enabled {
            return Self::default();
        }

        let (Some(mode), Some(role)) = (raw_mode, raw_role) else {
            warn!(
                "Network boot requested but mode/role is incomplete. Use --coop-server, --coop-client <ip>, --pvp-server, or --pvp-client <ip>."
            );
            return Self::default();
        };

        let host_ip = cli
            .host_ip
            .or_else(|| env::var("LOCAL_NET_DEBUG_HOST").ok())
            .unwrap_or_else(|| "127.0.0.1".to_string());
        let client_id = cli
            .client_id
            .or_else(|| {
                env::var("LOCAL_NET_DEBUG_CLIENT_ID")
                    .ok()
                    .and_then(|value| value.trim().parse().ok())
            })
            .unwrap_or(0);
        let port = cli.port.or_else(|| {
            env::var("LOCAL_NET_DEBUG_PORT")
                .ok()
                .and_then(|value| value.trim().parse().ok())
        });
        let mode_label = match mode {
            LocalNetDebugMode::Coop => "Coop",
            LocalNetDebugMode::Pvp => "PVP",
        };
        let role_label = role.label();
        let save_suffix = env::var("LOCAL_NET_DEBUG_SAVE_SUFFIX")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or(cli.save_suffix)
            .or_else(|| {
                Some(format!(
                    "{}_{}",
                    mode_label.to_ascii_lowercase(),
                    role_label.to_ascii_lowercase()
                ))
            });

        Self {
            enabled: true,
            mode: Some(mode),
            role: Some(role),
            host_ip,
            client_id,
            port,
            save_suffix,
            title_suffix: Some(format!("[{mode_label} {role_label}]")),
            window_pos: cli.window_pos.or_else(|| {
                env_flag("LOCAL_NET_DEBUG").then_some(match role {
                    LocalNetDebugRole::Server => IVec2::new(40, 40),
                    LocalNetDebugRole::Host => IVec2::new(40, 40),
                    LocalNetDebugRole::Client => IVec2::new(980, 40),
                })
            }),
        }
    }

    pub fn save_filename(&self) -> Option<String> {
        if !self.enabled {
            return None;
        }
        self.save_suffix
            .as_ref()
            .map(|suffix| format!("run_save_debug_{suffix}.ron"))
    }
}

#[derive(Debug, Default)]
struct CliNetBootConfig {
    enabled: bool,
    mode: Option<LocalNetDebugMode>,
    role: Option<LocalNetDebugRole>,
    host_ip: Option<String>,
    client_id: Option<u64>,
    port: Option<u16>,
    save_suffix: Option<String>,
    window_pos: Option<IVec2>,
}

impl CliNetBootConfig {
    fn from_args() -> Self {
        let mut config = Self::default();
        let mut raw_args = env::args();
        let exe_name = raw_args.next().unwrap_or_default();
        apply_server_binary_name_default(&mut config, &exe_name);
        let is_client_binary = is_named_binary(&exe_name, "client");
        let args = raw_args.collect::<Vec<_>>();
        let mut index = 0;

        while let Some(arg) = args.get(index) {
            match arg.as_str() {
                "--coop-server" => {
                    config.enabled = true;
                    config.mode = Some(LocalNetDebugMode::Coop);
                    config.role = Some(LocalNetDebugRole::Server);
                }
                "--coop-host" => {
                    config.enabled = true;
                    config.mode = Some(LocalNetDebugMode::Coop);
                    config.role = Some(LocalNetDebugRole::Host);
                }
                "--coop-client" => {
                    config.enabled = true;
                    config.mode = Some(LocalNetDebugMode::Coop);
                    config.role = Some(LocalNetDebugRole::Client);
                    if let Some(value) = next_arg_value(&args, &mut index) {
                        config.host_ip = Some(value);
                    }
                }
                "--pvp-server" => {
                    config.enabled = true;
                    config.mode = Some(LocalNetDebugMode::Pvp);
                    config.role = Some(LocalNetDebugRole::Server);
                }
                "--pvp-host" => {
                    config.enabled = true;
                    config.mode = Some(LocalNetDebugMode::Pvp);
                    config.role = Some(LocalNetDebugRole::Host);
                }
                "--pvp-client" => {
                    config.enabled = true;
                    config.mode = Some(LocalNetDebugMode::Pvp);
                    config.role = Some(LocalNetDebugRole::Client);
                    if let Some(value) = next_arg_value(&args, &mut index) {
                        config.host_ip = Some(value);
                    }
                }
                "--net-mode" | "--net" => {
                    if let Some(value) = next_arg_value(&args, &mut index) {
                        config.enabled = true;
                        apply_net_mode_token(&mut config, &value);
                    }
                }
                "--role" => {
                    if let Some(value) = next_arg_value(&args, &mut index).and_then(parse_net_role)
                    {
                        config.enabled = true;
                        config.role = Some(value);
                    }
                }
                "--host" | "--server" => {
                    if let Some(value) = next_arg_value(&args, &mut index) {
                        config.enabled = true;
                        config.host_ip = Some(value);
                    }
                }
                "--client-id" => {
                    if let Some(value) = next_arg_value(&args, &mut index) {
                        config.enabled = true;
                        config.client_id = value.trim().parse().ok();
                    }
                }
                "--port" => {
                    if let Some(value) = next_arg_value(&args, &mut index) {
                        config.enabled = true;
                        config.port = value.trim().parse().ok();
                    }
                }
                "--save-suffix" => {
                    config.save_suffix =
                        next_arg_value(&args, &mut index).filter(|value| !value.trim().is_empty());
                }
                "--window-pos" => {
                    config.window_pos = next_arg_value(&args, &mut index)
                        .and_then(|value| parse_window_pos(&value));
                }
                _ if is_client_binary && config.host_ip.is_none() && !arg.starts_with("--") => {
                    config.enabled = true;
                    config.mode = Some(LocalNetDebugMode::Coop);
                    config.role = Some(LocalNetDebugRole::Client);
                    config.host_ip = Some(arg.clone());
                }
                _ => {}
            }
            index += 1;
        }

        config
    }
}

fn apply_server_binary_name_default(config: &mut CliNetBootConfig, exe_name: &str) {
    if !is_named_binary(exe_name, "server") {
        return;
    }

    config.enabled = true;
    config.mode = Some(LocalNetDebugMode::Coop);
    config.role = Some(LocalNetDebugRole::Server);
}

fn is_named_binary(exe_name: &str, expected: &str) -> bool {
    let stem = std::path::Path::new(exe_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    stem == expected
}

fn next_arg_value(args: &[String], index: &mut usize) -> Option<String> {
    let value = args.get(*index + 1)?;
    if value.starts_with("--") {
        return None;
    }
    *index += 1;
    Some(value.clone())
}

fn apply_net_mode_token(config: &mut CliNetBootConfig, value: &str) {
    let normalized = value.trim().to_ascii_lowercase();
    let (mode_token, role_token) = normalized
        .split_once([':', '-', '/'])
        .unwrap_or((normalized.as_str(), ""));

    if let Some(mode) = parse_net_mode(mode_token) {
        config.mode = Some(mode);
    }
    if let Some(role) = parse_net_role(role_token) {
        config.role = Some(role);
    }
}

fn parse_net_mode(value: impl AsRef<str>) -> Option<LocalNetDebugMode> {
    match value.as_ref().trim().to_ascii_lowercase().as_str() {
        "coop" | "co-op" => Some(LocalNetDebugMode::Coop),
        "pvp" => Some(LocalNetDebugMode::Pvp),
        _ => None,
    }
}

fn parse_net_role(value: impl AsRef<str>) -> Option<LocalNetDebugRole> {
    match value.as_ref().trim().to_ascii_lowercase().as_str() {
        "server" => Some(LocalNetDebugRole::Server),
        "host" => Some(LocalNetDebugRole::Host),
        "client" => Some(LocalNetDebugRole::Client),
        _ => None,
    }
}

fn parse_window_pos(value: &str) -> Option<IVec2> {
    let (x, y) = value.split_once(',')?;
    Some(IVec2::new(x.trim().parse().ok()?, y.trim().parse().ok()?))
}

#[derive(Resource, Debug, Default, Clone, Copy)]
struct LocalNetDebugRuntime {
    bootstrapped: bool,
    window_applied: bool,
}

pub fn debug_save_filename() -> Option<String> {
    LocalNetDebugConfig::from_env().save_filename()
}

fn apply_local_debug_window(
    config: Res<LocalNetDebugConfig>,
    mut runtime: ResMut<LocalNetDebugRuntime>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
) {
    if !config.enabled || runtime.window_applied {
        return;
    }

    let Ok(mut window) = window_q.get_single_mut() else {
        return;
    };

    if let Some(suffix) = config.title_suffix.as_ref()
        && !window.title.contains(suffix)
    {
        window.title = format!("{} {}", window.title, suffix);
    }
    if let Some(pos) = config.window_pos {
        window.mode = WindowMode::Windowed;
        window.position = WindowPosition::At(pos);
    }
    runtime.window_applied = true;
}

fn auto_boot_local_debug(
    mut commands: Commands,
    state: Res<State<AppState>>,
    config: Res<LocalNetDebugConfig>,
    mut runtime: ResMut<LocalNetDebugRuntime>,
    mut coop_config: ResMut<CoopNetConfig>,
    mut coop_net: ResMut<CoopNetState>,
    mut coop_flow: ResMut<CoopSessionFlow>,
    mut pvp_config: ResMut<PvpNetConfig>,
    mut pvp_net: ResMut<PvpNetState>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if !config.enabled || runtime.bootstrapped || *state.get() != AppState::MainMenu {
        return;
    }

    reset_coop_network(&mut coop_config, &mut coop_net);
    reset_pvp_network(&mut pvp_config, &mut pvp_net);

    let Some(mode) = config.mode else {
        return;
    };
    let Some(role) = config.role else {
        return;
    };

    let boot_ok = match (mode, role) {
        (LocalNetDebugMode::Coop, LocalNetDebugRole::Server) => {
            coop_config.mode = CoopNetMode::Server;
            coop_config.host_ip.clear();
            coop_config.client_id = 0;
            if let Some(port) = config.port {
                coop_config.port = port;
            }
            if let Err(err) = begin_coop_lobby_session(&coop_config, &mut coop_net, &mut coop_flow)
            {
                warn!("Local coop server startup failed: {err:?}");
                false
            } else {
                println!(
                    "Block City coop server started successfully on UDP port {}.",
                    coop_config.port
                );
                next_state.set(AppState::CoopLobby);
                true
            }
        }
        (LocalNetDebugMode::Coop, LocalNetDebugRole::Host) => {
            coop_config.mode = CoopNetMode::Host;
            coop_config.host_ip.clear();
            coop_config.client_id = crate::coop::net::HOST_CLIENT_ID;
            if let Some(port) = config.port {
                coop_config.port = port;
            }
            if let Err(err) = begin_coop_lobby_session(&coop_config, &mut coop_net, &mut coop_flow)
            {
                warn!("Local coop debug host startup failed: {err:?}");
                false
            } else {
                commands.insert_resource(FloorNumber(1));
                commands.insert_resource(EnemySpawnCount { current: 0 });
                next_state.set(AppState::CoopLobby);
                true
            }
        }
        (LocalNetDebugMode::Coop, LocalNetDebugRole::Client) => {
            match normalize_coop_host_ip(&config.host_ip) {
                Ok(host_ip) => {
                    coop_config.mode = CoopNetMode::Client;
                    coop_config.host_ip = host_ip;
                    coop_config.client_id = if config.client_id == 0 {
                        crate::coop::net::REMOTE_CLIENT_ID
                    } else {
                        config.client_id
                    };
                    if let Some(port) = config.port {
                        coop_config.port = port;
                    }
                    if let Err(err) =
                        begin_coop_lobby_session(&coop_config, &mut coop_net, &mut coop_flow)
                    {
                        warn!("Local coop debug client startup failed: {err:?}");
                        false
                    } else {
                        next_state.set(AppState::CoopLobby);
                        true
                    }
                }
                Err(err) => {
                    warn!("Invalid local coop debug host address: {err}");
                    false
                }
            }
        }
        (LocalNetDebugMode::Pvp, LocalNetDebugRole::Server) => {
            pvp_config.mode = PvpNetMode::Server;
            if let Some(port) = config.port {
                pvp_config.port = port;
            }
            if let Err(err) = start_pvp_server_socket(&mut pvp_net, pvp_config.port) {
                warn!("Local pvp server startup failed: {err:?}");
                false
            } else {
                println!(
                    "Block City pvp server started successfully on UDP port {}.",
                    pvp_config.port
                );
                next_state.set(AppState::PvpLobby);
                true
            }
        }
        (LocalNetDebugMode::Pvp, LocalNetDebugRole::Host) => {
            pvp_config.mode = PvpNetMode::Host;
            if let Some(port) = config.port {
                pvp_config.port = port;
            }
            if let Err(err) = start_pvp_host_socket(&mut pvp_net, pvp_config.port) {
                warn!("Local pvp debug host startup failed: {err:?}");
                false
            } else {
                next_state.set(AppState::PvpLobby);
                true
            }
        }
        (LocalNetDebugMode::Pvp, LocalNetDebugRole::Client) => {
            pvp_config.mode = PvpNetMode::Client;
            pvp_config.host_ip = config.host_ip.clone();
            if let Some(port) = config.port {
                pvp_config.port = port;
            }
            if let Err(err) = start_pvp_client_socket(&mut pvp_net) {
                warn!("Local pvp debug client startup failed: {err:?}");
                false
            } else if let Ok(addr) = format!("{}:{}", config.host_ip, pvp_config.port).parse() {
                pvp_net.peer = Some(addr);
                next_state.set(AppState::PvpLobby);
                true
            } else {
                warn!("Invalid local pvp debug host address: {}", config.host_ip);
                false
            }
        }
    };

    if boot_ok {
        runtime.bootstrapped = true;
        info!("Local net debug bootstrapped: {:?} {:?}", mode, role);
    }
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .ok()
        .map(|value| {
            let trimmed = value.trim();
            !trimmed.is_empty() && trimmed != "0" && !trimmed.eq_ignore_ascii_case("false")
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_host_and_client_roles_are_distinct() {
        assert_eq!(parse_net_role("server"), Some(LocalNetDebugRole::Server));
        assert_eq!(parse_net_role("host"), Some(LocalNetDebugRole::Host));
        assert_eq!(parse_net_role("client"), Some(LocalNetDebugRole::Client));
    }

    #[test]
    fn generic_net_mode_token_sets_mode_and_role() {
        let mut config = CliNetBootConfig::default();

        apply_net_mode_token(&mut config, "coop-server");

        assert_eq!(config.mode, Some(LocalNetDebugMode::Coop));
        assert_eq!(config.role, Some(LocalNetDebugRole::Server));
    }

    #[test]
    fn server_binary_boots_dedicated_server() {
        let mut server = CliNetBootConfig::default();
        apply_server_binary_name_default(&mut server, "target/debug/server.exe");
        assert_eq!(server.mode, Some(LocalNetDebugMode::Coop));
        assert_eq!(server.role, Some(LocalNetDebugRole::Server));
    }

    #[test]
    fn client_binary_does_not_auto_join_without_address() {
        let mut client = CliNetBootConfig::default();
        apply_server_binary_name_default(&mut client, "target/debug/client.exe");
        assert_eq!(client.mode, None);
        assert_eq!(client.role, None);
        assert!(!client.enabled);
    }

    #[test]
    fn window_position_uses_comma_separated_coordinates() {
        assert_eq!(parse_window_pos("40,980"), Some(IVec2::new(40, 980)));
        assert_eq!(parse_window_pos("bad"), None);
    }
}
