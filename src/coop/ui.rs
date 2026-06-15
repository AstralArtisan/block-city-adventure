use std::net::{IpAddr, Ipv4Addr, UdpSocket};

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use lightyear::prelude::Replicated;
use lightyear::prelude::client::MessageEvent as LyClientMessageEvent;
use lightyear::shared::replication::components::Controlled;

use crate::constants::{ROOM_HALF_HEIGHT, ROOM_HALF_WIDTH, UI_Z};
use crate::core::assets::GameAssets;
use crate::core::input::PlayerInputState;
use crate::gameplay::combat::components::Projectile;
use crate::gameplay::effects::afterimage;
use crate::gameplay::effects::damage_numbers::DamageNumber;
use crate::gameplay::enemy::components::{Enemy, EnemyKind, EnemyType};
use crate::gameplay::map::InGameEntity;
use crate::gameplay::map::doors::Door;
use crate::gameplay::map::room::RoomType;
use crate::gameplay::player::combat::{melee_swing_profile, spawn_melee_slash_visual};
use crate::gameplay::player::components::{
    AnimationState, FacingDirection, Health, Player, RewardModifiers,
};
use crate::gameplay::shop::next_refresh_cost;
use crate::states::AppState;
use crate::ui::widgets;
use crate::utils::entity::safe_despawn_recursive;

use super::components::{
    CoopDamageEvent, CoopDashVisualState, CoopDoorOption, CoopHudRoot, CoopMeleeFlashState,
    CoopNetPosition, CoopNetRotation, CoopOverlayRoot, CoopParticipant, CoopPhase,
    CoopRemoteHealthBarFill, CoopRemoteHealthBarRoot, CoopRewardOption, CoopRewardSelectionGroup,
    CoopRpsChoice, CoopSessionState, CoopShopItem, CoopShopOffer, CoopVisualReady, GhostState,
    LocalAnimPrediction, LocalControlled, PlayerSlot,
};
use super::net::{
    COOP_PORT, CoopCommandMessage, CoopExitDestination, CoopExitRequest, CoopNetConfig,
    CoopNetState, CoopSessionFlow, NetMode, begin_coop_lobby_session, configured_client_id,
    normalize_coop_host_ip, queue_command, queue_exit_request, reset_coop_network,
    slot_for_client_id,
};

const REMOTE_BAR_WIDTH: f32 = 34.0;
const REMOTE_BAR_HEIGHT: f32 = 4.0;
const REPLICATED_DASH_AFTERIMAGE_INTERVAL_S: f32 = 0.05;
const REPLICATED_DASH_DEACTIVATE_GRACE_S: f32 = 0.09;

#[derive(Component, Debug, Clone)]
pub(crate) struct ReplicatedPlayerVisualState {
    last_melee_sequence: u16,
    last_dash_active: bool,
    last_effectively_alive: bool,
    dash_afterimage_timer: Timer,
    dash_deactivate_grace_s: f32,
}

impl Default for ReplicatedPlayerVisualState {
    fn default() -> Self {
        Self {
            last_melee_sequence: 0,
            last_dash_active: false,
            last_effectively_alive: true,
            dash_afterimage_timer: Timer::from_seconds(
                REPLICATED_DASH_AFTERIMAGE_INTERVAL_S,
                TimerMode::Repeating,
            ),
            dash_deactivate_grace_s: 0.0,
        }
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct ReplicatedDoorLabel(crate::gameplay::map::room::Direction);

#[derive(Component)]
pub struct CoopMenuUi;

#[derive(Component)]
pub struct CoopLobbyUi;

#[derive(Component)]
pub struct CoopGameUi;

#[derive(Component)]
pub struct CoopLobbyText;

#[derive(Component)]
pub struct CoopIpText;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoopMenuAction {
    Host,
    Join,
    Back,
}

#[derive(Component)]
pub(crate) struct CoopMenuButton(CoopMenuAction);

#[derive(Component)]
pub(crate) struct CoopMenuNoticeText;

#[derive(Component)]
pub(crate) struct CoopLobbyModeText;

#[derive(Component)]
pub(crate) struct CoopLobbyStatusText;

#[derive(Component)]
pub(crate) struct CoopLobbyShareText;

#[derive(Component)]
pub(crate) struct CoopLobbyBackButton;

#[derive(Component)]
pub(crate) struct CoopLobbyStartButton;

#[derive(Component)]
pub(crate) struct CoopStatusSummaryText;

#[derive(Component)]
pub(crate) struct CoopStatusPlayersText;

#[derive(Component)]
pub(crate) struct CoopStatusDetailText;

#[derive(Component)]
pub(crate) struct CoopStatusHintText;

#[derive(Component)]
pub(crate) struct CoopModalShade;

#[derive(Component)]
pub(crate) struct CoopModalTitleText;

#[derive(Component)]
pub(crate) struct CoopModalBodyText;

#[derive(Component)]
pub(crate) struct CoopModalFooterText;

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct CoopModalOptionButton {
    index: usize,
}

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct CoopModalOptionTitle {
    index: usize,
}

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct CoopModalOptionBody {
    index: usize,
}

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct CoopModalOptionMeta {
    index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoopUiAction {
    SelectReward(CoopRewardSelectionGroup, u8),
    BuyShopItem(u8),
    RefreshShop,
    SelectRps(CoopRpsChoice),
    LeaveToLobby,
    ReturnToMainMenu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CoopModalTone {
    #[default]
    Normal,
    Info,
    Positive,
    Selected,
    Disabled,
    Danger,
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub(crate) struct CoopModalButtonState {
    action: Option<CoopUiAction>,
    enabled: bool,
}

#[derive(Debug, Clone, Default)]
struct CoopModalOptionView {
    visible: bool,
    title: String,
    body: String,
    meta: String,
    action: Option<CoopUiAction>,
    enabled: bool,
    tone: CoopModalTone,
}

#[derive(Debug, Clone, Default)]
struct CoopModalView {
    visible: bool,
    title: String,
    body: String,
    footer: String,
    options: [CoopModalOptionView; 6],
}

#[derive(Resource, Debug, Default, Clone)]
pub struct CoopJoinIp {
    pub ip: String,
    pub notice: String,
}

pub fn setup_coop_menu(mut commands: Commands, assets: Res<GameAssets>) {
    commands.insert_resource(CoopJoinIp {
        ip: String::new(),
        notice: format!(
            "请输入房主的局域网 IPv4 地址，UDP 端口 {} 固定不变。",
            COOP_PORT
        ),
    });
    commands
        .spawn((widgets::root_node(), CoopMenuUi, Name::new("CoopMenuRoot")))
        .with_children(|root| {
            root.spawn(widgets::modal_panel_node(680.0))
                .with_children(|panel| {
                    panel.spawn(widgets::title_text(&assets, "联机合作", 46.0));
                    panel.spawn(widgets::muted_text(
                        &assets,
                        "与队友一起推进楼层，联机奖励、商店与协作事件都会通过专用面板同步展示。",
                        18.0,
                    ));

                    panel
                        .spawn((
                            widgets::button_bundle_sized(300.0, 56.0),
                            CoopMenuButton(CoopMenuAction::Host),
                        ))
                        .with_children(|button| {
                            button.spawn(widgets::title_text(&assets, "创建房间", 24.0));
                            button.spawn(widgets::muted_text(
                                &assets,
                                "本机将作为主机并进入联机大厅",
                                15.0,
                            ));
                        });

                    panel
                        .spawn(widgets::section_node(widgets::section_color()))
                        .with_children(|section| {
                            section.spawn(widgets::title_text(&assets, "加入房间", 24.0));
                            section.spawn(widgets::muted_text(
                                &assets,
                                "输入房主的局域网 IP，例如 192.168.1.6",
                                16.0,
                            ));
                            section
                                .spawn((widgets::input_field_node(), Name::new("CoopJoinIpField")))
                                .with_children(|field| {
                                    field.spawn((
                                        widgets::body_text(&assets, "未输入 IP", 18.0),
                                        CoopIpText,
                                    ));
                                });
                            section.spawn((
                                widgets::muted_text(
                                    &assets,
                                    "输入房主局域网 IP 后即可加入房间。",
                                    15.0,
                                ),
                                CoopMenuNoticeText,
                            ));
                            section
                                .spawn((
                                    widgets::button_bundle_sized(240.0, 48.0),
                                    CoopMenuButton(CoopMenuAction::Join),
                                ))
                                .with_children(|button| {
                                    button.spawn(widgets::title_text(&assets, "加入房间", 22.0));
                                });
                        });

                    panel
                        .spawn((
                            widgets::button_bundle_sized(220.0, 46.0),
                            CoopMenuButton(CoopMenuAction::Back),
                        ))
                        .with_children(|button| {
                            button.spawn(widgets::title_text(&assets, "返回", 20.0));
                        });

                    panel.spawn(widgets::muted_text(
                        &assets,
                        "快捷键：H 创建房间 · Enter / J 加入房间 · Esc 返回",
                        15.0,
                    ));
                });
        });
}

pub fn coop_menu_input_system(
    mut key_events: EventReader<KeyboardInput>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut ip: ResMut<CoopJoinIp>,
    mut text_q: ParamSet<(
        Query<&mut Text, With<CoopIpText>>,
        Query<&mut Text, With<CoopMenuNoticeText>>,
    )>,
    mut button_q: Query<(&Interaction, &CoopMenuButton, &mut BackgroundColor), With<Button>>,
    mut config: ResMut<CoopNetConfig>,
    mut net: ResMut<CoopNetState>,
    mut flow: ResMut<CoopSessionFlow>,
    mut next: ResMut<NextState<AppState>>,
) {
    for ev in key_events.read() {
        if !ev.state.is_pressed() {
            continue;
        }
        if let Key::Character(ref s) = ev.logical_key {
            for c in s.chars() {
                if (c.is_ascii_digit() || c == '.') && ip.ip.len() < 64 {
                    ip.ip.push(c);
                }
            }
        }
    }
    if keyboard.just_pressed(KeyCode::Backspace) {
        ip.ip.pop();
    }

    if ip.ip.trim().is_empty() {
        ip.notice = format!(
            "请输入房主的局域网 IPv4 地址，UDP 端口 {} 固定不变。",
            COOP_PORT
        );
    } else {
        ip.notice = format!("目标主机：{}（UDP {} 固定）", ip.ip.trim(), COOP_PORT);
    }

    let mut action = None;
    for (interaction, button, mut color) in &mut button_q {
        let enabled = button.0 != CoopMenuAction::Join || !ip.ip.trim().is_empty();
        *color = BackgroundColor(coop_menu_button_color(*interaction, button.0, enabled));
        if *interaction == Interaction::Pressed && enabled {
            action = Some(button.0);
        }
    }

    if keyboard.just_pressed(KeyCode::Escape) {
        action = Some(CoopMenuAction::Back);
    } else if keyboard.just_pressed(KeyCode::KeyH) {
        action = Some(CoopMenuAction::Host);
    } else if keyboard.just_pressed(KeyCode::KeyJ) || keyboard.just_pressed(KeyCode::Enter) {
        action = Some(CoopMenuAction::Join);
    }

    match action {
        Some(CoopMenuAction::Back) => {
            reset_coop_network(&mut config, &mut net);
            flow.pending_game_entry = false;
            flow.lobby_notice.clear();
            flow.pending_exit = None;
            next.set(AppState::MultiplayerMenu);
            return;
        }
        Some(CoopMenuAction::Host) => {
            config.mode = NetMode::Host;
            config.host_ip.clear();
            config.client_id = super::net::HOST_CLIENT_ID;
            match begin_coop_lobby_session(&config, &mut net, &mut flow) {
                Ok(()) => {
                    next.set(AppState::CoopLobby);
                    return;
                }
                Err(err) => ip.notice = err,
            }
        }
        Some(CoopMenuAction::Join) => {
            if ip.ip.trim().is_empty() {
                ip.notice = "请先输入房主的局域网 IPv4 地址。".to_string();
            } else {
                match normalize_coop_host_ip(ip.ip.trim()) {
                    Ok(host_ip) => {
                        config.mode = NetMode::Client;
                        config.host_ip = host_ip.clone();
                        config.client_id = super::net::REMOTE_CLIENT_ID;
                        ip.ip = host_ip;
                        match begin_coop_lobby_session(&config, &mut net, &mut flow) {
                            Ok(()) => {
                                next.set(AppState::CoopLobby);
                                return;
                            }
                            Err(err) => ip.notice = err,
                        }
                    }
                    Err(err) => ip.notice = err,
                }
            }
        }
        None => {}
    }

    if let Ok(mut ip_text) = text_q.p0().get_single_mut() {
        ip_text.sections[0].value = if ip.ip.trim().is_empty() {
            "未输入房主 IP".to_string()
        } else {
            ip.ip.clone()
        };
    }
    if let Ok(mut notice_text) = text_q.p1().get_single_mut() {
        notice_text.sections[0].value = ip.notice.clone();
    }
}

pub fn cleanup_coop_menu(mut commands: Commands, q: Query<Entity, With<CoopMenuUi>>) {
    for entity in &q {
        safe_despawn_recursive(&mut commands, entity);
    }
}

pub fn setup_coop_lobby(mut commands: Commands, assets: Res<GameAssets>) {
    commands
        .spawn((
            widgets::root_node(),
            CoopLobbyUi,
            Name::new("CoopLobbyRoot"),
        ))
        .with_children(|root| {
            root.spawn(widgets::modal_panel_node(700.0))
                .with_children(|panel| {
                    panel.spawn(widgets::title_text(&assets, "联机大厅", 44.0));
                    panel.spawn(widgets::muted_text(
                        &assets,
                        "等待双方连线完成，准备进入合作楼层。",
                        18.0,
                    ));
                    panel
                        .spawn(widgets::section_node(widgets::section_color()))
                        .with_children(|section| {
                            section.spawn((
                                widgets::body_text(&assets, "模式：联机合作", 18.0),
                                CoopLobbyModeText,
                            ));
                            section.spawn((
                                widgets::body_text(&assets, "状态：正在同步连接信息", 18.0),
                                CoopLobbyStatusText,
                            ));
                        });
                    panel
                        .spawn(widgets::section_node(widgets::section_alt_color()))
                        .with_children(|section| {
                            section.spawn(widgets::title_text(&assets, "连接说明", 22.0));
                            section.spawn((
                                widgets::muted_text(&assets, "正在读取主机分享地址…", 16.0),
                                CoopLobbyShareText,
                            ));
                        });
                    panel.spawn((
                        widgets::muted_text(
                            &assets,
                            "端口 3457 固定不变，请仅使用纯 IPv4 地址。",
                            15.0,
                        ),
                        CoopLobbyText,
                    ));
                    panel
                        .spawn(NodeBundle {
                            style: Style {
                                width: Val::Percent(100.0),
                                justify_content: JustifyContent::Center,
                                column_gap: Val::Px(14.0),
                                ..default()
                            },
                            ..default()
                        })
                        .with_children(|buttons| {
                            buttons
                                .spawn((
                                    widgets::button_bundle_sized(220.0, 46.0),
                                    CoopLobbyStartButton,
                                ))
                                .with_children(|button| {
                                    button.spawn(widgets::title_text(&assets, "开始 / 重试", 20.0));
                                });
                            buttons
                                .spawn((
                                    widgets::button_bundle_sized(220.0, 46.0),
                                    CoopLobbyBackButton,
                                ))
                                .with_children(|button| {
                                    button.spawn(widgets::title_text(&assets, "返回", 20.0));
                                });
                        });
                });
        });
}

pub fn coop_lobby_ui_system(
    config: Res<CoopNetConfig>,
    net: Res<CoopNetState>,
    flow: Res<CoopSessionFlow>,
    mut text_q: ParamSet<(
        Query<&mut Text, With<CoopLobbyModeText>>,
        Query<&mut Text, With<CoopLobbyStatusText>>,
        Query<&mut Text, With<CoopLobbyShareText>>,
        Query<&mut Text, With<CoopLobbyText>>,
    )>,
) {
    let session_armed = coop_bool_word(flow.pending_game_entry);

    if let Ok(mut mode_text) = text_q.p0().get_single_mut() {
        mode_text.sections[0].value = format!("模式：{}", coop_mode_label(config.mode));
    }

    if let Ok(mut status_text) = text_q.p1().get_single_mut() {
        status_text.sections[0].value = match config.mode {
            NetMode::Host => format!(
                "会话已就绪：{}\n本地客户端：{}\n远端玩家：{}",
                session_armed,
                coop_connection_word(net.local_connected),
                if net.remote_connected {
                    "已连接"
                } else {
                    "等待加入"
                },
            ),
            NetMode::Server => format!(
                "Session armed: {}\nP1 client: {}\nP2 client: {}",
                session_armed,
                coop_connection_word(
                    net.connected_clients
                        .contains(&super::net::host_client_id())
                ),
                coop_connection_word(
                    net.connected_clients
                        .contains(&super::net::remote_client_id())
                ),
            ),
            NetMode::Client => format!(
                "会话已就绪：{}\n目标主机：{}\n连接状态：{}",
                session_armed,
                config.host_ip,
                if net.connected {
                    "已连接"
                } else {
                    "连接中"
                },
            ),
            NetMode::None => "尚未选择联机模式".to_string(),
        };
    }

    if let Ok(mut share_text) = text_q.p2().get_single_mut() {
        share_text.sections[0].value = match config.mode {
            NetMode::Host => {
                let share_ip = best_effort_host_share_ip()
                    .map(|ip| format!("请将此局域网 IPv4 地址分享给队友：{}", ip))
                    .unwrap_or_else(|| "请将此局域网 IPv4 地址分享给队友：".to_string());
                format!(
                    "{}\n端口 {} 固定不变，请勿将 127.0.0.1 或 localhost 发给队友。",
                    share_ip, COOP_PORT
                )
            }
            NetMode::Server => {
                let share_ip = best_effort_host_share_ip()
                    .map(|ip| format!("Server IPv4: {}", ip))
                    .unwrap_or_else(|| "Server IPv4:".to_string());
                format!(
                    "{}\nUDP {}. Start clients with --coop-client <ip> --client-id 1 and --client-id 2.",
                    share_ip, COOP_PORT
                )
            }
            NetMode::Client => format!(
                "正在连接主机 {}\n端口 {} 固定不变，仅接受纯 IPv4 地址。",
                config.host_ip, COOP_PORT
            ),
            NetMode::None => "未初始化联机模式。".to_string(),
        };
    }

    if let Ok(mut hint_text) = text_q.p3().get_single_mut() {
        hint_text.sections[0].value = if !flow.lobby_notice.is_empty() {
            flow.lobby_notice.clone()
        } else if flow.pending_game_entry {
            match config.mode {
                NetMode::Host if net.remote_connected => {
                    "会话已就绪，双方同步完成后对局将自动开始。".to_string()
                }
                NetMode::Host => "等待队友加入，请分享局域网 IPv4（不含端口）。".to_string(),
                NetMode::Server
                    if net
                        .connected_clients
                        .contains(&super::net::host_client_id())
                        && net
                            .connected_clients
                            .contains(&super::net::remote_client_id()) =>
                {
                    "Two clients connected. Match will start automatically.".to_string()
                }
                NetMode::Server => {
                    "Waiting for two clients: client id 1 and client id 2.".to_string()
                }
                NetMode::Client if net.connected => {
                    "已连接主机，等待会话状态与地图复制完成。".to_string()
                }
                NetMode::Client => "正在连接主机，请稍候。".to_string(),
                NetMode::None => "请先选择房主或客户端模式。".to_string(),
            }
        } else {
            "按“开始 / 重试”激活合作会话，Esc 返回联机菜单。".to_string()
        };
    }
}

pub fn coop_lobby_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut start_q: Query<
        (&Interaction, &mut BackgroundColor),
        (With<CoopLobbyStartButton>, Without<CoopLobbyBackButton>),
    >,
    mut back_q: Query<
        (&Interaction, &mut BackgroundColor),
        (With<CoopLobbyBackButton>, Without<CoopLobbyStartButton>),
    >,
    config: ResMut<CoopNetConfig>,
    mut net: ResMut<CoopNetState>,
    mut flow: ResMut<CoopSessionFlow>,
) {
    let retry_enabled = !flow.pending_game_entry && config.mode != NetMode::None;
    let mut wants_retry = keyboard.just_pressed(KeyCode::Enter) && retry_enabled;
    let mut wants_back = keyboard.just_pressed(KeyCode::Escape);

    for (interaction, mut color) in &mut start_q {
        *color = BackgroundColor(coop_menu_button_color(
            *interaction,
            CoopMenuAction::Join,
            retry_enabled,
        ));
        if *interaction == Interaction::Pressed && retry_enabled {
            wants_retry = true;
        }
    }

    for (interaction, mut color) in &mut back_q {
        *color = BackgroundColor(coop_menu_button_color(
            *interaction,
            CoopMenuAction::Back,
            true,
        ));
        if *interaction == Interaction::Pressed {
            wants_back = true;
        }
    }

    if wants_retry {
        match begin_coop_lobby_session(&config, &mut net, &mut flow) {
            Ok(()) => {}
            Err(err) => flow.lobby_notice = err,
        }
    }

    if wants_back {
        queue_exit_request(
            &mut flow,
            CoopExitRequest {
                destination: CoopExitDestination::MultiplayerMenu,
                notice: None,
                preserve_mode: false,
            },
        );
    }
}

pub fn cleanup_coop_lobby(mut commands: Commands, q: Query<Entity, With<CoopLobbyUi>>) {
    for entity in &q {
        safe_despawn_recursive(&mut commands, entity);
    }
}

pub fn setup_coop_game_ui(mut commands: Commands, assets: Res<GameAssets>) {
    commands
        .spawn((
            widgets::root_node(),
            CoopGameUi,
            Name::new("CoopGameUiRoot"),
        ))
        .with_children(|root| {
            root.spawn((
                NodeBundle {
                    style: Style {
                        position_type: PositionType::Absolute,
                        right: Val::Px(18.0),
                        top: Val::Px(108.0),
                        width: Val::Px(320.0),
                        padding: UiRect::all(Val::Px(14.0)),
                        row_gap: Val::Px(8.0),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    background_color: BackgroundColor(widgets::section_color()),
                    ..default()
                },
                CoopHudRoot,
                CoopOverlayRoot,
                Name::new("CoopStatusCard"),
            ))
            .with_children(|panel| {
                panel.spawn(widgets::title_text(&assets, "联机状态", 24.0));
                panel.spawn((
                    widgets::body_text(&assets, "正在建立联机会话…", 17.0),
                    CoopStatusSummaryText,
                ));
                panel.spawn((
                    widgets::body_text(&assets, "我方：同步中\n队友：同步中", 16.0),
                    CoopStatusPlayersText,
                ));
                panel.spawn((widgets::muted_text(&assets, "", 14.0), CoopStatusDetailText));
                panel.spawn((
                    widgets::muted_text(&assets, "提示：保持同步推进，房门交互仍使用 E。", 15.0),
                    CoopStatusHintText,
                ));
            });

            root.spawn((
                NodeBundle {
                    style: Style {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        right: Val::Px(0.0),
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    background_color: BackgroundColor(widgets::modal_scrim_color(0.58)),
                    visibility: Visibility::Hidden,
                    ..default()
                },
                CoopModalShade,
                Name::new("CoopModalShade"),
            ))
            .with_children(|shade| {
                shade
                    .spawn((
                        widgets::modal_panel_node(760.0),
                        Name::new("CoopModalPanel"),
                    ))
                    .with_children(|panel| {
                        panel.spawn((widgets::title_text(&assets, "", 34.0), CoopModalTitleText));
                        panel.spawn((widgets::body_text(&assets, "", 18.0), CoopModalBodyText));
                        panel.spawn((widgets::muted_text(&assets, "", 15.0), CoopModalFooterText));
                        panel
                            .spawn((
                                NodeBundle {
                                    style: Style {
                                        width: Val::Percent(100.0),
                                        column_gap: Val::Px(12.0),
                                        row_gap: Val::Px(12.0),
                                        flex_wrap: FlexWrap::Wrap,
                                        justify_content: JustifyContent::Center,
                                        ..default()
                                    },
                                    ..default()
                                },
                                Name::new("CoopModalOptions"),
                            ))
                            .with_children(|options| {
                                for index in 0..6 {
                                    options
                                        .spawn((
                                            widgets::button_bundle_sized(300.0, 92.0),
                                            CoopModalOptionButton { index },
                                            CoopModalButtonState::default(),
                                            Name::new(format!("CoopModalOption{}", index + 1)),
                                        ))
                                        .with_children(|button| {
                                            button.spawn((
                                                widgets::title_text(&assets, "", 21.0),
                                                CoopModalOptionTitle { index },
                                            ));
                                            button.spawn((
                                                widgets::body_text(&assets, "", 16.0),
                                                CoopModalOptionBody { index },
                                            ));
                                            button.spawn((
                                                widgets::muted_text(&assets, "", 14.0),
                                                CoopModalOptionMeta { index },
                                            ));
                                        });
                                }
                            });
                    });
            });
        });
}

pub fn ensure_local_control_marker(
    config: Res<CoopNetConfig>,
    mut commands: Commands,
    player_q: Query<
        (
            Entity,
            Option<&PlayerSlot>,
            Option<&Replicated>,
            Option<&Controlled>,
            Option<&CoopParticipant>,
            Option<&LocalControlled>,
            Option<&GhostState>,
            Option<&Health>,
        ),
        With<Player>,
    >,
) {
    let local_slot = slot_for_config(&config);
    let preferred_client_entity = if config.mode == NetMode::Client {
        player_q
            .iter()
            .filter(|(_, slot, replicated, _, _, _, _, _)| {
                replicated.is_some() && slot.is_some_and(|slot| *slot == local_slot)
            })
            .max_by_key(
                |(entity, slot, _, controlled, _, local_controlled, ghost, health)| {
                    (
                        client_replicated_player_score(
                            slot.copied(),
                            local_slot,
                            controlled.is_some(),
                            local_controlled.is_some(),
                            ghost.copied(),
                            *health,
                        ),
                        entity.index(),
                    )
                },
            )
            .map(|(entity, _, _, _, _, _, _, _)| entity)
    } else {
        None
    };

    for (entity, slot, replicated, _controlled, coop_participant, local_controlled, _, _) in
        &player_q
    {
        let should_control = match config.mode {
            NetMode::Server => false,
            NetMode::Host => {
                replicated.is_none()
                    && coop_participant.is_some()
                    && slot.is_some_and(|slot| *slot == local_slot)
            }
            NetMode::Client => Some(entity) == preferred_client_entity,
            NetMode::None => false,
        };

        match (should_control, local_controlled.is_some()) {
            (true, false) => {
                commands
                    .entity(entity)
                    .insert((LocalControlled, LocalAnimPrediction::default()));
            }
            (false, true) => {
                commands.entity(entity).remove::<LocalControlled>();
            }
            _ => {}
        }
    }
}

pub fn filter_replicated_player_duplicates(
    mut commands: Commands,
    config: Res<CoopNetConfig>,
    players: Query<
        (
            Entity,
            &PlayerSlot,
            Option<&Controlled>,
            Option<&GhostState>,
            Option<&Health>,
            Option<&LocalControlled>,
        ),
        (With<Player>, With<Replicated>, With<CoopVisualReady>),
    >,
) {
    if config.mode != NetMode::Client {
        return;
    }

    let local_slot = slot_for_config(&config);
    let mut best_by_slot = [None; 2];
    let mut best_score_by_slot = [i32::MIN; 2];

    for (entity, slot, controlled, ghost, health, local_controlled) in &players {
        let score = client_replicated_player_score(
            Some(*slot),
            local_slot,
            controlled.is_some(),
            local_controlled.is_some(),
            ghost.copied(),
            health,
        );
        let index = slot.index();
        if score > best_score_by_slot[index] {
            best_score_by_slot[index] = score;
            best_by_slot[index] = Some(entity);
        }
    }

    // Despawn 非最佳的重复实体，防止隐藏实体无限累积导致性能退化
    for (entity, slot, ..) in &players {
        if best_by_slot[slot.index()] != Some(entity) {
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn sync_host_authority_visibility(
    config: Res<CoopNetConfig>,
    mut q: Query<
        (
            &mut Visibility,
            Option<&PlayerSlot>,
            Option<&Replicated>,
            Option<&CoopParticipant>,
            Option<&Enemy>,
            Option<&Projectile>,
        ),
        Without<Door>,
    >,
) {
    if !matches!(config.mode, NetMode::Host | NetMode::Server) {
        return;
    }

    for (mut current, slot, replicated, coop_participant, enemy, projectile) in &mut q {
        if coop_participant.is_none() && enemy.is_none() && projectile.is_none() {
            continue;
        }

        let visibility = if coop_participant.is_some() {
            match (replicated.is_some(), slot.copied()) {
                (false, Some(PlayerSlot::P1)) => Visibility::Inherited,
                (true, Some(PlayerSlot::P1)) => Visibility::Hidden,
                (true, Some(PlayerSlot::P2)) => Visibility::Inherited,
                _ => Visibility::Hidden,
            }
        } else if replicated.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };

        if *current != visibility {
            *current = visibility;
        }
    }
}

pub fn attach_replicated_visuals(
    mut commands: Commands,
    assets: Res<GameAssets>,
    q_players: Query<
        (Entity, &PlayerSlot, Option<&CoopNetPosition>),
        (With<Replicated>, With<Player>, Without<CoopVisualReady>),
    >,
    q_enemies: Query<
        (Entity, &EnemyKind, Option<&CoopNetPosition>),
        (
            With<Replicated>,
            With<Enemy>,
            Without<CoopVisualReady>,
            Without<Player>,
        ),
    >,
    q_projectiles: Query<
        (
            Entity,
            &Projectile,
            Option<&CoopNetPosition>,
            Option<&CoopNetRotation>,
        ),
        (
            With<Replicated>,
            Without<CoopVisualReady>,
            Without<Player>,
            Without<Enemy>,
        ),
    >,
    q_doors: Query<(Entity, &Door), (With<Replicated>, With<Door>, Without<CoopVisualReady>)>,
) {
    for (entity, slot, pos) in &q_players {
        commands.entity(entity).insert((
            SpriteBundle {
                texture: assets.textures.player.clone(),
                transform: Transform::from_translation(
                    pos.map(|value| value.0.extend(50.0))
                        .unwrap_or(Vec3::new(0.0, 0.0, 50.0)),
                ),
                sprite: Sprite {
                    color: player_slot_color(*slot),
                    custom_size: Some(Vec2::new(74.0, 60.0)),
                    ..default()
                },
                ..default()
            },
            CoopVisualReady,
            ReplicatedPlayerVisualState::default(),
            Name::new(format!("Replicated{}", slot.label())),
        ));
        commands.entity(entity).with_children(|parent| {
            parent
                .spawn((
                    SpriteBundle {
                        texture: assets.textures.white.clone(),
                        transform: Transform::from_translation(Vec3::new(0.0, 46.0, 2.0)),
                        sprite: Sprite {
                            color: Color::srgba(0.04, 0.06, 0.05, 0.84),
                            custom_size: Some(Vec2::new(
                                REMOTE_BAR_WIDTH + 2.0,
                                REMOTE_BAR_HEIGHT + 2.0,
                            )),
                            ..default()
                        },
                        visibility: Visibility::Hidden,
                        ..default()
                    },
                    CoopRemoteHealthBarRoot,
                    Name::new(format!("{}RemoteHealthBar", slot.label())),
                ))
                .with_children(|background| {
                    background.spawn((
                        SpriteBundle {
                            texture: assets.textures.white.clone(),
                            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 1.0)),
                            sprite: Sprite {
                                color: Color::srgb(0.26, 0.86, 0.42),
                                custom_size: Some(Vec2::new(REMOTE_BAR_WIDTH, REMOTE_BAR_HEIGHT)),
                                ..default()
                            },
                            ..default()
                        },
                        CoopRemoteHealthBarFill,
                    ));
                });
        });
    }

    for (entity, kind, pos) in &q_enemies {
        let color = match kind.0 {
            EnemyType::MeleeChaser => Color::srgb(0.95, 0.45, 0.45),
            EnemyType::Lobber => Color::srgb(0.74, 0.56, 0.96),
            EnemyType::RangedShooter => Color::srgb(0.55, 0.65, 0.95),
            EnemyType::Charger => Color::srgb(0.95, 0.75, 0.25),
            EnemyType::Flanker => Color::srgb(0.96, 0.56, 0.78),
            EnemyType::Sniper => Color::srgb(0.70, 0.82, 1.0),
            EnemyType::SupportCaster => Color::srgb(0.55, 0.95, 0.80),
            EnemyType::Bomber => Color::srgb(0.98, 0.38, 0.22),
            EnemyType::Shielder => Color::srgb(0.36, 0.56, 0.78),
            EnemyType::Summoner => Color::srgb(0.64, 0.38, 0.90),
            EnemyType::Boss => Color::srgb(0.85, 0.25, 0.95),
        };
        commands.entity(entity).insert((
            SpriteBundle {
                texture: assets.textures.white.clone(),
                transform: Transform::from_translation(
                    pos.map(|value| value.0.extend(20.0))
                        .unwrap_or(Vec3::new(0.0, 0.0, 20.0)),
                ),
                sprite: Sprite {
                    color,
                    custom_size: Some(Vec2::splat(30.0)),
                    ..default()
                },
                ..default()
            },
            CoopVisualReady,
        ));
    }

    for (entity, projectile, pos, rotation) in &q_projectiles {
        let color = match projectile.team {
            crate::gameplay::combat::components::Team::Player => Color::srgb(0.2, 0.85, 1.0),
            _ => Color::srgb(1.0, 0.35, 0.25),
        };
        commands.entity(entity).insert((
            SpriteBundle {
                texture: assets.textures.white.clone(),
                transform: Transform {
                    translation: pos
                        .map(|value| value.0.extend(20.0))
                        .unwrap_or(Vec3::new(0.0, 0.0, 20.0)),
                    rotation: Quat::from_rotation_z(rotation.map(|value| value.0).unwrap_or(0.0)),
                    ..default()
                },
                sprite: Sprite {
                    color,
                    custom_size: Some(Vec2::new(16.0, 8.0)),
                    ..default()
                },
                ..default()
            },
            CoopVisualReady,
        ));
    }

    for (entity, door) in &q_doors {
        commands.entity(entity).insert((
            SpriteBundle {
                texture: assets.textures.white.clone(),
                transform: Transform::from_translation(door_position(door.dir)),
                sprite: Sprite {
                    color: Color::srgb(0.65, 0.50, 0.20),
                    custom_size: Some(door_size(door.dir)),
                    ..default()
                },
                visibility: Visibility::Hidden,
                ..default()
            },
            CoopVisualReady,
            Name::new(format!("ReplicatedDoor{:?}", door.dir)),
        ));
        commands.entity(entity).with_children(|parent| {
            parent.spawn((
                Text2dBundle {
                    text: Text::from_section(
                        "交互 (E)",
                        TextStyle {
                            font: assets.font.clone(),
                            font_size: 22.0,
                            color: Color::WHITE,
                        },
                    ),
                    transform: Transform::from_translation(replicated_door_label_offset(door.dir)),
                    visibility: Visibility::Hidden,
                    ..default()
                },
                ReplicatedDoorLabel(door.dir),
                Name::new(format!("ReplicatedDoorLabel{:?}", door.dir)),
            ));
        });
    }
}

pub fn sync_replicated_visuals(
    mut commands: Commands,
    assets: Res<GameAssets>,
    time: Res<Time>,
    mut player_q: Query<
        (
            &PlayerSlot,
            &mut Transform,
            &Handle<Image>,
            &mut Sprite,
            &Visibility,
            &mut ReplicatedPlayerVisualState,
            Option<&LocalControlled>,
            Option<&CoopNetPosition>,
            Option<&Health>,
            Option<&GhostState>,
            Option<&FacingDirection>,
            Option<&AnimationState>,
            Option<&CoopMeleeFlashState>,
            Option<&CoopDashVisualState>,
            Option<&LocalAnimPrediction>,
        ),
        (With<CoopVisualReady>, With<Player>, With<Replicated>),
    >,
    mut other_q: Query<
        (
            &mut Transform,
            Option<&CoopNetPosition>,
            Option<&CoopNetRotation>,
            Option<&Projectile>,
        ),
        (With<CoopVisualReady>, Without<Player>),
    >,
) {
    // 远程玩家：系数 14 平滑抖动；本地玩家：系数 55 近似即时跟随，消除视觉滞后
    let smooth_remote = 1.0 - (-14.0 * time.delta_seconds()).exp();
    let smooth_local = 1.0 - (-55.0 * time.delta_seconds()).exp();
    let swing = melee_swing_profile(RewardModifiers::default());
    for (
        _slot,
        mut transform,
        texture,
        mut sprite,
        visibility,
        mut cache,
        local_controlled,
        pos,
        health,
        ghost,
        facing,
        anim,
        melee_flash,
        dash_visual,
        local_anim_pred,
    ) in &mut player_q
    {
        let effectively_alive = coop_player_is_alive(ghost.copied(), health);
        let revived_this_frame = effectively_alive && !cache.last_effectively_alive;

        if let Some(pos) = pos {
            if revived_this_frame {
                transform.translation.x = pos.0.x;
                transform.translation.y = pos.0.y;
            } else {
                let smooth = if local_controlled.is_some() {
                    smooth_local
                } else {
                    smooth_remote
                };
                let current = transform.translation.truncate();
                let next = current.lerp(pos.0, smooth);
                transform.translation.x = next.x;
                transform.translation.y = next.y;
            }
        }

        transform.rotation = Quat::IDENTITY;
        if let Some(facing) = facing {
            sprite.flip_x = facing.0.x < -0.15;
        }

        let host_anim = anim.copied().unwrap_or(AnimationState::Idle);
        let visual_anim = if let Some(pred) = local_anim_pred {
            if pred.override_timer_s > 0.0 {
                pred.predicted_anim
            } else {
                host_anim
            }
        } else {
            host_anim
        };
        let visual_anim = match visual_anim {
            AnimationState::Dead if effectively_alive => AnimationState::Idle,
            state => state,
        };

        sprite.color = match visual_anim {
            AnimationState::Idle => Color::WHITE,
            AnimationState::Move => Color::srgb(0.98, 0.99, 1.0),
            AnimationState::Attack => Color::srgb(1.0, 0.95, 0.92),
            AnimationState::Dash => Color::srgb(0.84, 0.94, 1.0),
            AnimationState::Hurt => Color::srgb(1.0, 0.82, 0.82),
            AnimationState::Dead => Color::srgb(0.40, 0.40, 0.40),
        };
        if !effectively_alive {
            sprite.color.set_alpha(0.42);
        }

        if revived_this_frame {
            cache.last_melee_sequence = melee_flash.map_or(0, |state| state.sequence);
            cache.last_dash_active = false;
            cache.dash_deactivate_grace_s = 0.0;
            cache.dash_afterimage_timer.reset();
        }

        if let Some(melee_flash) = melee_flash {
            if effectively_alive
                && *visibility != Visibility::Hidden
                && melee_flash.sequence != 0
                && melee_flash.sequence != cache.last_melee_sequence
            {
                let direction = Vec2::from_angle(melee_flash.slash_angle_rad);
                let auth_pos = pos
                    .map(|position| position.0)
                    .unwrap_or(transform.translation.truncate());
                let slash_origin = auth_pos + direction * swing.center_offset;
                spawn_melee_slash_visual(
                    &mut commands,
                    &assets,
                    slash_origin,
                    Quat::from_rotation_z(melee_flash.slash_angle_rad),
                    swing.slash_size,
                    Color::srgba(0.84, 0.98, 0.96, 0.84),
                    61.0,
                    Vec3::ONE,
                    if local_controlled.is_some() {
                        0.90
                    } else {
                        0.84
                    },
                );
            }
            cache.last_melee_sequence = melee_flash.sequence;
        }

        if effectively_alive {
            if let Some(dash_visual) = dash_visual {
                let dash_started = dash_visual.active && !cache.last_dash_active;
                let dash_effectively_active = if dash_visual.active {
                    cache.dash_deactivate_grace_s = 0.0;
                    true
                } else if cache.last_dash_active {
                    cache.dash_deactivate_grace_s += time.delta_seconds();
                    if cache.dash_deactivate_grace_s < REPLICATED_DASH_DEACTIVATE_GRACE_S {
                        true
                    } else {
                        cache.dash_deactivate_grace_s = 0.0;
                        cache.dash_afterimage_timer.reset();
                        false
                    }
                } else {
                    false
                };

                if dash_started {
                    cache.dash_afterimage_timer.reset();
                }
                if dash_effectively_active {
                    cache.dash_afterimage_timer.tick(time.delta());
                    if *visibility != Visibility::Hidden
                        && (dash_started || cache.dash_afterimage_timer.just_finished())
                    {
                        afterimage::spawn_afterimage(
                            &mut commands,
                            texture.clone(),
                            transform.translation.truncate(),
                            sprite.color.with_alpha(0.28),
                            sprite.custom_size.unwrap_or(Vec2::splat(32.0)),
                            sprite.flip_x,
                        );
                    }
                }
                cache.last_dash_active = dash_effectively_active;
            } else {
                cache.last_dash_active = false;
                cache.dash_deactivate_grace_s = 0.0;
                cache.dash_afterimage_timer.reset();
            }
        } else {
            cache.last_dash_active = false;
            cache.dash_deactivate_grace_s = 0.0;
            cache.dash_afterimage_timer.reset();
        }

        cache.last_effectively_alive = effectively_alive;
    }

    for (mut transform, pos, rotation, projectile) in &mut other_q {
        if let Some(pos) = pos {
            let current = transform.translation.truncate();
            let next = current.lerp(pos.0, smooth_remote);
            transform.translation.x = next.x;
            transform.translation.y = next.y;
        }
        if projectile.is_some() {
            if let Some(rotation) = rotation {
                transform.rotation = Quat::from_rotation_z(rotation.0);
            }
        } else {
            transform.rotation = Quat::IDENTITY;
        }
    }
}

pub fn update_remote_health_bars(
    local_q: Query<&PlayerSlot, (With<Player>, With<LocalControlled>)>,
    player_q: Query<
        (&PlayerSlot, &GhostState, &Health, &Children),
        (With<Player>, With<Replicated>),
    >,
    mut bar_q: Query<(&Children, &mut Visibility), With<CoopRemoteHealthBarRoot>>,
    mut fill_q: Query<(&mut Transform, &mut Sprite), With<CoopRemoteHealthBarFill>>,
) {
    let local_slot = local_q.iter().next().copied();

    for (slot, ghost, health, children) in &player_q {
        for child in children {
            let Ok((bar_children, mut visibility)) = bar_q.get_mut(*child) else {
                continue;
            };

            let should_show =
                Some(*slot) != local_slot && coop_player_is_alive(Some(*ghost), Some(health));
            *visibility = if should_show {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };

            if !should_show {
                continue;
            }

            let ratio = if health.max > 0.0 {
                (health.current / health.max).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let width = REMOTE_BAR_WIDTH * ratio;
            for fill in bar_children {
                let Ok((mut transform, mut sprite)) = fill_q.get_mut(*fill) else {
                    continue;
                };
                sprite.custom_size = Some(Vec2::new(width, REMOTE_BAR_HEIGHT));
                transform.translation.x = -(REMOTE_BAR_WIDTH - width) * 0.5;
            }
        }
    }
}

pub fn update_replicated_door_visuals(
    config: Res<CoopNetConfig>,
    session_q: Query<&CoopSessionState, With<Replicated>>,
    mut door_q: Query<(&Door, &mut Sprite, &mut Visibility), (With<Door>, With<Replicated>)>,
    mut label_q: Query<(&ReplicatedDoorLabel, &mut Text, &mut Visibility), Without<Door>>,
) {
    let session = session_q.get_single().ok();
    let room_state = session
        .map(|value| value.room_state)
        .unwrap_or(crate::states::RoomState::Idle);

    for (door, mut sprite, mut visibility) in &mut door_q {
        let next_visibility = if matches!(config.mode, NetMode::Host | NetMode::Server) {
            Visibility::Hidden
        } else if door.active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next_visibility {
            *visibility = next_visibility;
        }
        sprite.color = match room_state {
            crate::states::RoomState::Locked | crate::states::RoomState::BossFight => {
                Color::srgb(0.65, 0.18, 0.12)
            }
            crate::states::RoomState::Cleared | crate::states::RoomState::Idle => {
                Color::srgb(0.65, 0.50, 0.20)
            }
        };
    }

    for (label, mut text, mut visibility) in &mut label_q {
        let next = if matches!(config.mode, NetMode::Host | NetMode::Server) {
            None
        } else {
            session.and_then(|value| {
                value
                    .door_choice
                    .options
                    .iter()
                    .find(|option| option.dir == label.0)
            })
        };

        if let Some(option) = next {
            set_text_if_changed(
                &mut text,
                &format!("{} (E)", coop_room_type_brief_label(option.room_type)),
            );
            if *visibility != Visibility::Inherited {
                *visibility = Visibility::Inherited;
            }
        } else if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
    }
}

pub fn update_coop_overlay(
    config: Res<CoopNetConfig>,
    net: Res<CoopNetState>,
    player_q: Query<
        (
            &PlayerSlot,
            &Health,
            &GhostState,
            Option<&LocalControlled>,
            Option<&Replicated>,
            Option<&CoopParticipant>,
        ),
        With<Player>,
    >,
    session_q: Query<&CoopSessionState, With<Replicated>>,
    mut text_q: Query<
        (
            &mut Text,
            Option<&CoopStatusSummaryText>,
            Option<&CoopStatusPlayersText>,
            Option<&CoopStatusDetailText>,
            Option<&CoopStatusHintText>,
            Option<&CoopModalTitleText>,
            Option<&CoopModalBodyText>,
            Option<&CoopModalFooterText>,
            Option<&CoopModalOptionTitle>,
            Option<&CoopModalOptionBody>,
            Option<&CoopModalOptionMeta>,
        ),
        Or<(
            With<CoopStatusSummaryText>,
            With<CoopStatusPlayersText>,
            With<CoopStatusDetailText>,
            With<CoopStatusHintText>,
            With<CoopModalTitleText>,
            With<CoopModalBodyText>,
            With<CoopModalFooterText>,
            With<CoopModalOptionTitle>,
            With<CoopModalOptionBody>,
            With<CoopModalOptionMeta>,
        )>,
    >,
    mut shade_q: Query<&mut Visibility, (With<CoopModalShade>, Without<Button>)>,
    mut option_q: Query<
        (
            &CoopModalOptionButton,
            &Interaction,
            &mut Visibility,
            &mut BackgroundColor,
            &mut CoopModalButtonState,
        ),
        (With<Button>, Without<CoopModalShade>),
    >,
) {
    let slot = slot_for_config(&config);
    let session = session_q.get_single().ok();
    let view = coop_build_modal_view(session, slot);
    let summary_value = coop_status_summary(config.mode, &net, session);
    let players_value = coop_status_players(&config, &player_q);
    let detail_value = coop_status_detail(session, slot);
    let hint_value = coop_status_hint(session).to_string();

    for (
        mut text,
        summary_marker,
        players_marker,
        detail_marker,
        hint_marker,
        title_marker,
        body_marker,
        footer_marker,
        option_title_marker,
        option_body_marker,
        option_meta_marker,
    ) in &mut text_q
    {
        if summary_marker.is_some() {
            set_text_if_changed(&mut text, &summary_value);
        } else if players_marker.is_some() {
            set_text_if_changed(&mut text, &players_value);
        } else if detail_marker.is_some() {
            set_text_if_changed(&mut text, &detail_value);
        } else if hint_marker.is_some() {
            set_text_if_changed(&mut text, &hint_value);
        } else if title_marker.is_some() {
            set_text_if_changed(&mut text, &view.title);
        } else if body_marker.is_some() {
            set_text_if_changed(&mut text, &view.body);
        } else if footer_marker.is_some() {
            set_text_if_changed(&mut text, &view.footer);
        } else if let Some(marker) = option_title_marker {
            set_text_if_changed(&mut text, &view.options[marker.index].title);
        } else if let Some(marker) = option_body_marker {
            set_text_if_changed(&mut text, &view.options[marker.index].body);
        } else if let Some(marker) = option_meta_marker {
            set_text_if_changed(&mut text, &view.options[marker.index].meta);
        }
    }

    if let Ok(mut shade) = shade_q.get_single_mut() {
        let next_visibility = if view.visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *shade != next_visibility {
            *shade = next_visibility;
        }
    }

    for (button, interaction, mut visibility, mut color, mut state) in &mut option_q {
        let option = &view.options[button.index];
        let next_visibility = if view.visible && option.visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next_visibility {
            *visibility = next_visibility;
        }
        *state = CoopModalButtonState {
            action: option.action,
            enabled: option.enabled,
        };
        let next_color = coop_modal_button_color(*interaction, option.tone, option.enabled);
        if color.0 != next_color {
            *color = BackgroundColor(next_color);
        }
    }
}

pub fn handle_coop_overlay_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    config: Res<CoopNetConfig>,
    button_q: Query<
        (&Interaction, &CoopModalButtonState),
        (
            Changed<Interaction>,
            With<CoopModalOptionButton>,
            With<Button>,
        ),
    >,
    mut net: ResMut<CoopNetState>,
    mut flow: ResMut<CoopSessionFlow>,
    session_q: Query<&CoopSessionState, With<Replicated>>,
) {
    let slot = slot_for_config(&config);
    let mut action = None;
    for (interaction, state) in &button_q {
        if *interaction == Interaction::Pressed && state.enabled {
            action = state.action;
        }
    }

    let Ok(session) = session_q.get_single() else {
        return;
    };

    match session.phase {
        CoopPhase::Paused => {
            if keyboard.just_pressed(KeyCode::Enter) {
                action = Some(CoopUiAction::LeaveToLobby);
            }
        }
        CoopPhase::Reward => {
            if let Some(index) = coop_pressed_digit(&keyboard) {
                action = coop_reward_action_for_digit(session, slot, index);
            }
        }
        CoopPhase::DoorChoice => {}
        CoopPhase::Rps => {
            let choice = coop_pressed_rps_choice(&keyboard);
            if let Some(choice) = choice {
                action = Some(CoopUiAction::SelectRps(choice));
            }
        }
        CoopPhase::Shop => {
            if let Some(index) = coop_pressed_digit(&keyboard) {
                action = if index < 3 {
                    Some(CoopUiAction::BuyShopItem(index as u8))
                } else if index == 3 {
                    Some(CoopUiAction::RefreshShop)
                } else {
                    None
                };
            } else if keyboard.just_pressed(KeyCode::KeyR) {
                action = Some(CoopUiAction::RefreshShop);
            }
        }
        CoopPhase::MatchOver => {
            if keyboard.just_pressed(KeyCode::Enter) {
                action = Some(CoopUiAction::ReturnToMainMenu);
            }
        }
        CoopPhase::None => {}
    }

    if let Some(action) = action {
        coop_apply_action(action, slot, &mut net, &mut flow);
    }
}

pub fn cleanup_coop_game_ui(mut commands: Commands, q: Query<Entity, With<CoopGameUi>>) {
    for entity in &q {
        safe_despawn_recursive(&mut commands, entity);
    }
}

fn coop_apply_action(
    action: CoopUiAction,
    slot: PlayerSlot,
    net: &mut CoopNetState,
    flow: &mut CoopSessionFlow,
) {
    match action {
        CoopUiAction::SelectReward(group, index) => {
            queue_command(net, CoopCommandMessage::SelectReward { slot, group, index });
        }
        CoopUiAction::BuyShopItem(index) => {
            queue_command(net, CoopCommandMessage::BuyShopItem { slot, index });
        }
        CoopUiAction::RefreshShop => {
            queue_command(net, CoopCommandMessage::RefreshShop { slot });
        }
        CoopUiAction::SelectRps(choice) => {
            queue_command(net, CoopCommandMessage::SelectRps { slot, choice });
        }
        CoopUiAction::LeaveToLobby => {
            queue_exit_request(
                flow,
                CoopExitRequest {
                    destination: CoopExitDestination::Lobby,
                    notice: Some("你已离开合作会话。".to_string()),
                    preserve_mode: true,
                },
            );
        }
        CoopUiAction::ReturnToMainMenu => {
            queue_exit_request(
                flow,
                CoopExitRequest {
                    destination: CoopExitDestination::MainMenu,
                    notice: None,
                    preserve_mode: false,
                },
            );
        }
    }
}

fn coop_pressed_digit(keyboard: &ButtonInput<KeyCode>) -> Option<usize> {
    if keyboard.just_pressed(KeyCode::Digit1) || keyboard.just_pressed(KeyCode::Numpad1) {
        Some(0)
    } else if keyboard.just_pressed(KeyCode::Digit2) || keyboard.just_pressed(KeyCode::Numpad2) {
        Some(1)
    } else if keyboard.just_pressed(KeyCode::Digit3) || keyboard.just_pressed(KeyCode::Numpad3) {
        Some(2)
    } else if keyboard.just_pressed(KeyCode::Digit4) || keyboard.just_pressed(KeyCode::Numpad4) {
        Some(3)
    } else if keyboard.just_pressed(KeyCode::Digit5) || keyboard.just_pressed(KeyCode::Numpad5) {
        Some(4)
    } else if keyboard.just_pressed(KeyCode::Digit6) || keyboard.just_pressed(KeyCode::Numpad6) {
        Some(5)
    } else {
        None
    }
}

fn coop_pressed_rps_choice(keyboard: &ButtonInput<KeyCode>) -> Option<CoopRpsChoice> {
    match coop_pressed_digit(keyboard) {
        Some(0) => Some(CoopRpsChoice::Rock),
        Some(1) => Some(CoopRpsChoice::Paper),
        Some(2) => Some(CoopRpsChoice::Scissors),
        _ => None,
    }
}

fn coop_reward_action_for_digit(
    session: &CoopSessionState,
    slot: PlayerSlot,
    index: usize,
) -> Option<CoopUiAction> {
    let player_state = &session.reward.players[slot.index()];
    if !player_state.can_interact {
        return None;
    }

    match player_state.mode {
        super::components::CoopRewardMode::HealOrBuff => match index {
            0 if !player_state.primary_options.is_empty() => Some(CoopUiAction::SelectReward(
                CoopRewardSelectionGroup::Heal,
                0,
            )),
            1..=3 if player_state.primary_options.len() > index => Some(
                CoopUiAction::SelectReward(CoopRewardSelectionGroup::Primary, index as u8),
            ),
            _ => None,
        },
        super::components::CoopRewardMode::DualBuff => match index {
            0 | 2 | 4 => {
                let option_index = index / 2;
                (player_state.primary_options.len() > option_index).then_some(
                    CoopUiAction::SelectReward(
                        CoopRewardSelectionGroup::Primary,
                        option_index as u8,
                    ),
                )
            }
            1 | 3 | 5 => {
                let option_index = index / 2;
                (player_state.secondary_options.len() > option_index).then_some(
                    CoopUiAction::SelectReward(
                        CoopRewardSelectionGroup::Secondary,
                        option_index as u8,
                    ),
                )
            }
            _ => None,
        },
        _ => (player_state.primary_options.len() > index).then_some(CoopUiAction::SelectReward(
            CoopRewardSelectionGroup::Primary,
            index as u8,
        )),
    }
}

fn coop_menu_button_color(
    interaction: Interaction,
    action: CoopMenuAction,
    enabled: bool,
) -> Color {
    if !enabled {
        return widgets::button_disabled_color();
    }

    match action {
        CoopMenuAction::Host => match interaction {
            Interaction::Hovered => widgets::button_info_hover_color(),
            _ => widgets::button_info_color(),
        },
        CoopMenuAction::Back => match interaction {
            Interaction::Hovered => widgets::button_danger_hover_color(),
            _ => widgets::button_danger_color(),
        },
        CoopMenuAction::Join => match interaction {
            Interaction::Hovered => widgets::button_hover_color(),
            _ => widgets::button_base_color(),
        },
    }
}

fn coop_modal_button_color(interaction: Interaction, tone: CoopModalTone, enabled: bool) -> Color {
    if !enabled || tone == CoopModalTone::Disabled {
        return widgets::button_disabled_color();
    }

    match tone {
        CoopModalTone::Info => match interaction {
            Interaction::Hovered => widgets::button_info_hover_color(),
            _ => widgets::button_info_color(),
        },
        CoopModalTone::Positive | CoopModalTone::Selected => widgets::button_selected_color(),
        CoopModalTone::Danger => match interaction {
            Interaction::Hovered => widgets::button_danger_hover_color(),
            _ => widgets::button_danger_color(),
        },
        CoopModalTone::Normal | CoopModalTone::Disabled => match interaction {
            Interaction::Hovered => widgets::button_hover_color(),
            _ => widgets::button_base_color(),
        },
    }
}

fn coop_connection_word(connected: bool) -> &'static str {
    if connected { "已连接" } else { "连接中" }
}

fn coop_bool_word(value: bool) -> &'static str {
    if value { "是" } else { "否" }
}

fn coop_mode_label(mode: NetMode) -> &'static str {
    match mode {
        NetMode::Server => "服务器",
        NetMode::Host => "房主",
        NetMode::Client => "客户端",
        NetMode::None => "未连接",
    }
}

fn coop_phase_label(phase: CoopPhase) -> &'static str {
    match phase {
        CoopPhase::None => "自由推进",
        CoopPhase::Paused => "同步暂停",
        CoopPhase::Reward => "奖励选择",
        CoopPhase::DoorChoice => "路线协商",
        CoopPhase::Rps => "猜拳决胜",
        CoopPhase::Shop => "商店阶段",
        CoopPhase::MatchOver => "结算",
    }
}

fn coop_status_summary(
    mode: NetMode,
    net: &CoopNetState,
    session: Option<&CoopSessionState>,
) -> String {
    let connection = match mode {
        NetMode::Server => {
            if net
                .connected_clients
                .contains(&super::net::host_client_id())
                && net
                    .connected_clients
                    .contains(&super::net::remote_client_id())
            {
                "两个客户端已连接"
            } else {
                "等待两个客户端"
            }
        }
        NetMode::Host => {
            if net.remote_connected {
                "双方已连接"
            } else {
                "等待队友加入"
            }
        }
        NetMode::Client => {
            if net.connected {
                "已连接主机"
            } else {
                "连接中"
            }
        }
        NetMode::None => "未连接",
    };
    let phase = session
        .map(|value| coop_phase_label(value.phase))
        .unwrap_or("同步中");

    format!(
        "模式：{}\n连接：{}\n当前阶段：{}",
        coop_mode_label(mode),
        connection,
        phase
    )
}

fn coop_status_hint(session: Option<&CoopSessionState>) -> &'static str {
    match session.map(|value| value.phase).unwrap_or(CoopPhase::None) {
        CoopPhase::None => "提示：默认只显示精简状态卡，房门交互仍使用 E。",
        CoopPhase::Paused => "提示：Esc 恢复同步推进，Enter 离开到大厅。",
        CoopPhase::Reward => "提示：数字键或鼠标点击卡片都可以完成奖励选择。",
        CoopPhase::DoorChoice => "提示：走到真实存在的房门旁按 E 锁定路线。",
        CoopPhase::Rps => "提示：数字键 1 / 2 / 3 或鼠标点击都可以出拳。",
        CoopPhase::Shop => "提示：数字键购买，R 刷新自己的商店，E / Esc 离开商店。",
        CoopPhase::MatchOver => "提示：按 Enter 或点击按钮返回主菜单。",
    }
}

fn coop_status_detail(session: Option<&CoopSessionState>, slot: PlayerSlot) -> String {
    let Some(session) = session else {
        return String::new();
    };

    match session.phase {
        CoopPhase::DoorChoice => coop_door_status_detail(session, slot),
        _ => String::new(),
    }
}

fn coop_status_players(
    config: &CoopNetConfig,
    player_q: &Query<
        (
            &PlayerSlot,
            &Health,
            &GhostState,
            Option<&LocalControlled>,
            Option<&Replicated>,
            Option<&CoopParticipant>,
        ),
        With<Player>,
    >,
) -> String {
    let mode = config.mode;
    let local_slot = slot_for_config(config);
    let teammate_slot = coop_other_slot(local_slot);

    format!(
        "{}\n{}",
        coop_format_slot_status(
            "我方",
            local_slot,
            coop_slot_snapshot(mode, local_slot, local_slot, player_q)
        ),
        coop_format_slot_status(
            "队友",
            teammate_slot,
            coop_slot_snapshot(mode, local_slot, teammate_slot, player_q)
        ),
    )
}

fn coop_slot_snapshot(
    mode: NetMode,
    local_slot: PlayerSlot,
    target: PlayerSlot,
    player_q: &Query<
        (
            &PlayerSlot,
            &Health,
            &GhostState,
            Option<&LocalControlled>,
            Option<&Replicated>,
            Option<&CoopParticipant>,
        ),
        With<Player>,
    >,
) -> Option<(f32, f32, bool)> {
    let mut best_score = i32::MIN;
    let mut best = None;

    for (slot, health, ghost, local_controlled, replicated, coop_participant) in player_q.iter() {
        if *slot != target {
            continue;
        }

        let mut score = 0;
        if local_controlled.is_some() {
            score += 100;
        }
        if target == local_slot {
            match mode {
                NetMode::Host | NetMode::Server
                    if coop_participant.is_some() && replicated.is_none() =>
                {
                    score += 80
                }
                NetMode::Client if replicated.is_some() => score += 80,
                _ => {}
            }
        } else if replicated.is_some() {
            score += 90;
        } else if coop_participant.is_some() {
            score += 20;
        }

        if score > best_score {
            best_score = score;
            best = Some((
                health.current,
                health.max,
                !coop_player_is_alive(Some(*ghost), Some(health)),
            ));
        }
    }

    best
}

fn coop_format_slot_status(
    label: &str,
    slot: PlayerSlot,
    snapshot: Option<(f32, f32, bool)>,
) -> String {
    match snapshot {
        Some((current, max, true)) => {
            format!(
                "{label}（{}）：幽灵 · HP {:.0}/{:.0}",
                slot.label(),
                current,
                max
            )
        }
        Some((current, max, false)) => {
            format!(
                "{label}（{}）：存活 · HP {:.0}/{:.0}",
                slot.label(),
                current,
                max
            )
        }
        None => format!("{label}（{}）：同步中", slot.label()),
    }
}

fn coop_build_modal_view(session: Option<&CoopSessionState>, slot: PlayerSlot) -> CoopModalView {
    let Some(session) = session else {
        return CoopModalView::default();
    };

    match session.phase {
        CoopPhase::None => CoopModalView::default(),
        CoopPhase::Paused => coop_pause_modal_view(),
        CoopPhase::Reward => coop_reward_modal_view(session, slot),
        CoopPhase::DoorChoice => CoopModalView::default(),
        CoopPhase::Rps => coop_rps_modal_view(session, slot),
        CoopPhase::Shop => coop_shop_modal_view(session, slot),
        CoopPhase::MatchOver => coop_match_over_modal_view(session),
    }
}

fn coop_pause_modal_view() -> CoopModalView {
    let mut view = CoopModalView {
        visible: true,
        title: "同步暂停".to_string(),
        body: "任一玩家按下暂停后，双方战斗都会一起暂停，直到恢复或离开会话。".to_string(),
        footer: "Esc：恢复推进 · Enter：离开到大厅".to_string(),
        ..default()
    };
    view.options[0] = coop_modal_option(
        "离开到大厅",
        "断开当前合作会话并返回合作大厅。",
        "也可直接按 Enter",
        Some(CoopUiAction::LeaveToLobby),
        true,
        CoopModalTone::Danger,
    );
    view
}

fn coop_modal_option(
    title: impl Into<String>,
    body: impl Into<String>,
    meta: impl Into<String>,
    action: Option<CoopUiAction>,
    enabled: bool,
    tone: CoopModalTone,
) -> CoopModalOptionView {
    CoopModalOptionView {
        visible: true,
        title: title.into(),
        body: body.into(),
        meta: meta.into(),
        action,
        enabled,
        tone,
    }
}

fn coop_other_slot(slot: PlayerSlot) -> PlayerSlot {
    match slot {
        PlayerSlot::P1 => PlayerSlot::P2,
        PlayerSlot::P2 => PlayerSlot::P1,
    }
}

fn coop_reward_modal_view(session: &CoopSessionState, slot: PlayerSlot) -> CoopModalView {
    let player_state = &session.reward.players[slot.index()];
    let mut view = CoopModalView {
        visible: true,
        title: "房间奖励".to_string(),
        body: if player_state.can_interact {
            match player_state.mode {
                _ if session.reward.lone_survivor == Some(slot) => {
                    "你是当前唯一存活者，可以在休整、复活与强化中选择一项。".to_string()
                }
                super::components::CoopRewardMode::HealOrBuff => {
                    "选择治疗，或从三项强化里选择一项。".to_string()
                }
                super::components::CoopRewardMode::DualBuff => {
                    "Boss 奖励：左列选第一项，右列选第二项。".to_string()
                }
                _ => "从下列强化中选择一项。".to_string(),
            }
        } else if session.reward.lone_survivor.is_some() {
            "你当前处于幽灵状态，奖励由存活队友决定。".to_string()
        } else {
            "你已完成奖励阶段，等待队友确认。".to_string()
        },
        footer: if player_state.can_interact {
            match player_state.mode {
                super::components::CoopRewardMode::HealOrBuff => {
                    "1: 治疗  2-4: 选择强化".to_string()
                }
                super::components::CoopRewardMode::DualBuff => {
                    "1/3/5: 左列  2/4/6: 右列".to_string()
                }
                _ => "数字键或直接点击卡片进行选择。".to_string(),
            }
        } else {
            "等待队友完成后会自动继续推进。".to_string()
        },
        ..default()
    };

    let push_reward_option = |view: &mut CoopModalView,
                              view_index: usize,
                              group: CoopRewardSelectionGroup,
                              option_index: usize,
                              option: CoopRewardOption,
                              selected: bool| {
        let (title, body, meta) = coop_reward_option_copy(option);
        view.options[view_index] = coop_modal_option(
            title,
            body,
            if selected {
                "已选中".to_string()
            } else {
                meta
            },
            player_state
                .can_interact
                .then_some(CoopUiAction::SelectReward(group, option_index as u8)),
            player_state.can_interact,
            if selected {
                CoopModalTone::Selected
            } else if matches!(option, CoopRewardOption::Rest | CoopRewardOption::Revive) {
                CoopModalTone::Positive
            } else if player_state.can_interact {
                CoopModalTone::Normal
            } else {
                CoopModalTone::Disabled
            },
        );
    };

    match player_state.mode {
        super::components::CoopRewardMode::DualBuff => {
            for index in 0..3 {
                if let Some(option) = player_state.primary_options.get(index).copied() {
                    push_reward_option(
                        &mut view,
                        index * 2,
                        CoopRewardSelectionGroup::Primary,
                        index,
                        option,
                        player_state.selected_primary == Some(option),
                    );
                }
                if let Some(option) = player_state.secondary_options.get(index).copied() {
                    push_reward_option(
                        &mut view,
                        index * 2 + 1,
                        CoopRewardSelectionGroup::Secondary,
                        index,
                        option,
                        player_state.selected_secondary == Some(option),
                    );
                }
            }
        }
        super::components::CoopRewardMode::HealOrBuff => {
            if let Some(option) = player_state.primary_options.first().copied() {
                push_reward_option(
                    &mut view,
                    0,
                    CoopRewardSelectionGroup::Heal,
                    0,
                    option,
                    player_state.selected_primary == Some(option),
                );
            }
            for (index, option) in player_state
                .primary_options
                .iter()
                .copied()
                .skip(1)
                .enumerate()
            {
                push_reward_option(
                    &mut view,
                    index + 1,
                    CoopRewardSelectionGroup::Primary,
                    index + 1,
                    option,
                    player_state.selected_primary == Some(option),
                );
            }
        }
        _ => {
            for (index, option) in player_state.primary_options.iter().copied().enumerate() {
                push_reward_option(
                    &mut view,
                    index,
                    CoopRewardSelectionGroup::Primary,
                    index,
                    option,
                    player_state.selected_primary == Some(option),
                );
            }
        }
    }

    view
}

fn coop_door_status_detail(session: &CoopSessionState, slot: PlayerSlot) -> String {
    let my_choice = coop_selected_door_for_slot(session, slot);
    let teammate_choice = coop_selected_door_for_slot(session, coop_other_slot(slot));
    let mut lines = vec![match (my_choice, teammate_choice) {
        (Some(my), Some(teammate)) if my != teammate => {
            "路线冲突：双方选了不同房门，系统将进入猜拳决胜。".to_string()
        }
        (Some(_), Some(_)) => "路线已同步：双方都锁定了同一扇门。".to_string(),
        (Some(_), None) => "你已锁定路线，等待队友靠近房门并按 E。".to_string(),
        (None, Some(_)) => "队友已锁定路线，轮到你走到房门旁按 E。".to_string(),
        (None, None) => "路线待定：靠近想走的房门并按 E 锁定。".to_string(),
    }];

    for (index, option) in session.door_choice.options.iter().take(4).enumerate() {
        let status = if my_choice == Some(option.index) && teammate_choice == Some(option.index) {
            "双方锁定"
        } else if my_choice == Some(option.index) {
            "我方锁定"
        } else if teammate_choice == Some(option.index) {
            "队友锁定"
        } else {
            "可选择"
        };
        lines.push(format!(
            "{}. {} → {} · {}",
            index + 1,
            coop_direction_brief_label(option),
            coop_room_type_brief_label(option.room_type),
            status
        ));
    }

    if my_choice.is_some() ^ teammate_choice.is_some() {
        lines.push(format!(
            "Confirm timeout: {:.1}s",
            session.door_choice.input_timeout_s.max(0.0)
        ));
    }

    lines.join("\n")
}

fn coop_rps_modal_view(session: &CoopSessionState, slot: PlayerSlot) -> CoopModalView {
    let my_choice = coop_selected_rps_for_slot(session, slot);
    let teammate_choice = coop_selected_rps_for_slot(session, coop_other_slot(slot));
    let timeout_text = format!(
        "等待出拳（{:.0} 秒后自动随机）",
        session.rps.input_timeout_s.max(0.0)
    );
    let mut view = CoopModalView {
        visible: true,
        title: "猜拳决胜".to_string(),
        body: "双方路线冲突时，通过猜拳决定进入的房门。".to_string(),
        footer: if let Some(winner) = session.rps.winner {
            format!(
                "我方：{} · 队友：{} · 胜者：{}",
                my_choice.map(rps_choice_label).unwrap_or("未出拳"),
                teammate_choice.map(rps_choice_label).unwrap_or("未出拳"),
                winner.label()
            )
        } else if let Some(choice) = my_choice {
            format!("你已提交：{}，等待队友出拳。", rps_choice_label(choice))
        } else {
            "数字键 1 / 2 / 3 或点击卡片进行出拳。".to_string()
        },
        ..default()
    };

    if session.rps.winner.is_none() {
        view.footer = if let Some(choice) = my_choice {
            format!(
                "你已提交：{}，等待队友出拳。\n{}",
                rps_choice_label(choice),
                timeout_text
            )
        } else {
            format!("数字键 1 / 2 / 3 或点击卡片进行出拳。\n{}", timeout_text)
        };
    }

    for (index, choice) in [
        CoopRpsChoice::Rock,
        CoopRpsChoice::Paper,
        CoopRpsChoice::Scissors,
    ]
    .into_iter()
    .enumerate()
    {
        let selected = my_choice == Some(choice);
        let locked = my_choice.is_some() || session.rps.winner.is_some();
        view.options[index] = coop_modal_option(
            format!("{}. {}", index + 1, rps_choice_label(choice)),
            coop_rps_choice_hint(choice),
            if selected {
                "已选择".to_string()
            } else if locked {
                "等待结果".to_string()
            } else {
                "点击或按数字键选择".to_string()
            },
            (!locked).then_some(CoopUiAction::SelectRps(choice)),
            !locked,
            if selected {
                CoopModalTone::Selected
            } else if locked {
                CoopModalTone::Disabled
            } else {
                CoopModalTone::Normal
            },
        );
    }

    view
}

fn coop_shop_modal_view(session: &CoopSessionState, slot: PlayerSlot) -> CoopModalView {
    let player_state = &session.shop.players[slot.index()];
    let mut view = CoopModalView {
        visible: true,
        title: "联机商店".to_string(),
        body: if player_state.can_interact {
            "每名玩家拥有自己的商店列表与刷新次数；按 E 或 Esc 可以结束自己的商店阶段。".to_string()
        } else {
            "你已离开商店，等待队友完成购买。".to_string()
        },
        footer: if player_state.can_interact {
            "1-3 购买物品，4 或 R 刷新自己的商店。".to_string()
        } else {
            "队友完成后会自动结束商店阶段。".to_string()
        },
        ..default()
    };

    for (index, offer) in player_state.offers.iter().take(3).enumerate() {
        let (title, description) = coop_shop_offer_copy(offer);
        let (action, enabled, tone, meta) = if offer.purchased {
            (None, false, CoopModalTone::Positive, "已购买".to_string())
        } else if !player_state.can_interact {
            (
                None,
                false,
                CoopModalTone::Disabled,
                "等待队友完成".to_string(),
            )
        } else {
            (
                Some(CoopUiAction::BuyShopItem(index as u8)),
                true,
                CoopModalTone::Normal,
                format!("价格：{} 金币", offer.cost),
            )
        };

        view.options[index] = coop_modal_option(title, description, meta, action, enabled, tone);
    }

    // Coop UI uses the default shop config for display only; the host's
    // try_refresh_shop is the authoritative cost source on purchase.
    let shop_cfg = crate::data::definitions::ShopConfig::default();
    let refresh_cost = next_refresh_cost(player_state.refresh_count, &shop_cfg);
    let (refresh_meta, refresh_enabled, refresh_tone) = if player_state.can_interact {
        (
            if refresh_cost == 0 {
                "价格：免费".to_string()
            } else {
                format!("价格：{} 金币", refresh_cost)
            },
            true,
            CoopModalTone::Info,
        )
    } else {
        ("等待队友完成".to_string(), false, CoopModalTone::Disabled)
    };
    view.options[3] = coop_modal_option(
        "刷新商店",
        "重新随机你的三件商品，刷新价格随次数递增。",
        refresh_meta,
        refresh_enabled.then_some(CoopUiAction::RefreshShop),
        refresh_enabled,
        refresh_tone,
    );

    view
}

fn coop_match_over_modal_view(session: &CoopSessionState) -> CoopModalView {
    let victory = session.match_victory;
    let mut view = CoopModalView {
        visible: true,
        title: if victory {
            "合作通关".to_string()
        } else {
            "挑战失败".to_string()
        },
        body: if victory {
            "你们已经完成当前联机挑战，可以返回主菜单开始下一局。".to_string()
        } else {
            "本次合作挑战已经结束，整理一下状态后再重新出发。".to_string()
        },
        footer: "按 Enter 或点击下方按钮返回主菜单。".to_string(),
        ..default()
    };

    view.options[0] = coop_modal_option(
        "返回主菜单",
        "结束当前联机会话并回到主菜单",
        "确认后将离开本局游戏",
        Some(CoopUiAction::ReturnToMainMenu),
        true,
        if victory {
            CoopModalTone::Positive
        } else {
            CoopModalTone::Danger
        },
    );

    view
}

fn coop_selected_door_for_slot(session: &CoopSessionState, slot: PlayerSlot) -> Option<u8> {
    match slot {
        PlayerSlot::P1 => session.door_choice.p1_choice,
        PlayerSlot::P2 => session.door_choice.p2_choice,
    }
}

fn coop_selected_rps_for_slot(
    session: &CoopSessionState,
    slot: PlayerSlot,
) -> Option<CoopRpsChoice> {
    match slot {
        PlayerSlot::P1 => session.rps.p1_choice,
        PlayerSlot::P2 => session.rps.p2_choice,
    }
}

fn coop_reward_option_copy(option: CoopRewardOption) -> (String, String, String) {
    match option {
        CoopRewardOption::Rest => (
            "休整".to_string(),
            "立即恢复一段生命，稳住当前战斗节奏。".to_string(),
            "偏向即时续航".to_string(),
        ),
        CoopRewardOption::Revive => (
            "复活队友".to_string(),
            "将倒下的队友重新拉回战斗。".to_string(),
            "偏向队伍容错".to_string(),
        ),
        CoopRewardOption::Buff(buff) => coop_reward_buff_copy(buff),
    }
}

fn coop_reward_buff_copy(
    buff: crate::gameplay::rewards::data::RewardType,
) -> (String, String, String) {
    use crate::gameplay::rewards::data::RewardType;

    match buff {
        RewardType::RecoverHealth => (
            "恢复生命".to_string(),
            "立即恢复一截生命值。".to_string(),
            "偏向即时续航".to_string(),
        ),
        RewardType::EnhanceMeleeWeapon => (
            "近战精通".to_string(),
            "强化近战伤害与成长节点。".to_string(),
            "偏向贴身压制".to_string(),
        ),
        RewardType::IncreaseAttackSpeed => (
            "攻速强化".to_string(),
            "缩短近战与远程的出手间隔。".to_string(),
            "偏向节奏提升".to_string(),
        ),
        RewardType::IncreaseAttackPower => (
            "攻击强化".to_string(),
            "稳定提高基础伤害。".to_string(),
            "偏向直接增伤".to_string(),
        ),
        RewardType::IncreaseMaxHealth => (
            "生命强化".to_string(),
            "提高生命上限并顺带回复状态。".to_string(),
            "偏向容错与续航".to_string(),
        ),
        RewardType::ReduceDashCooldown => (
            "冲刺强化".to_string(),
            "更快恢复冲刺，提升走位容错。".to_string(),
            "偏向机动生存".to_string(),
        ),
        RewardType::LifeStealOnKill => (
            "击杀回血".to_string(),
            "击败敌人时恢复生命。".to_string(),
            "偏向连续推进".to_string(),
        ),
        RewardType::IncreaseCritChance => (
            "暴击强化".to_string(),
            "提高暴击率，提升爆发上限。".to_string(),
            "偏向高爆发输出".to_string(),
        ),
        RewardType::IncreaseMoveSpeed => (
            "移速强化".to_string(),
            "提升整体走位速度。".to_string(),
            "偏向拉扯与规避".to_string(),
        ),
        RewardType::DashDamageTrail => (
            "冲刺残影".to_string(),
            "冲刺时留下伤害轨迹。".to_string(),
            "偏向贴身压制".to_string(),
        ),
        RewardType::EnhanceRangedWeapon => (
            "远程改装".to_string(),
            "强化远程伤害、节奏与弹道。".to_string(),
            "偏向安全输出".to_string(),
        ),
    }
}

fn coop_shop_offer_copy(offer: &CoopShopOffer) -> (String, String) {
    match offer.item {
        CoopShopItem::Heal => ("治疗".to_string(), "立即恢复 35 点生命。".to_string()),
        CoopShopItem::IncreaseMaxHealth => ("强健".to_string(), "提高生命上限。".to_string()),
        CoopShopItem::IncreaseAttackPower => ("锋刃".to_string(), "提高攻击伤害。".to_string()),
        CoopShopItem::ReduceDashCooldown => ("迅捷".to_string(), "缩短冲刺冷却。".to_string()),
        CoopShopItem::IncreaseMoveSpeed => ("轻灵".to_string(), "提高移动速度。".to_string()),
        CoopShopItem::IncreaseEnergyMax => ("充能".to_string(), "提高能量上限。".to_string()),
        CoopShopItem::IncreaseCritChance => ("锐眼".to_string(), "提高暴击率。".to_string()),
        CoopShopItem::IncreaseAttackSpeed => ("连击".to_string(), "提高攻击节奏。".to_string()),
    }
}

fn coop_direction_brief_label(option: &CoopDoorOption) -> &'static str {
    match option.dir {
        crate::gameplay::map::room::Direction::Up => "上门",
        crate::gameplay::map::room::Direction::Down => "下门",
        crate::gameplay::map::room::Direction::Left => "左门",
        crate::gameplay::map::room::Direction::Right => "右门",
    }
}

fn coop_room_type_brief_label(room_type: RoomType) -> &'static str {
    match room_type {
        RoomType::Start => "起始",
        RoomType::Normal => "战斗",
        RoomType::Shop => "商店",
        RoomType::Reward => "奖励",
        RoomType::Event => "事件",
        RoomType::Elite => "精英",
        RoomType::Boss => "首领",
    }
}

fn replicated_door_label_offset(dir: crate::gameplay::map::room::Direction) -> Vec3 {
    match dir {
        crate::gameplay::map::room::Direction::Up => Vec3::new(0.0, -54.0, 11.0),
        crate::gameplay::map::room::Direction::Down => Vec3::new(0.0, 54.0, 11.0),
        crate::gameplay::map::room::Direction::Left
        | crate::gameplay::map::room::Direction::Right => Vec3::new(0.0, -74.0, 11.0),
    }
}

fn coop_rps_choice_hint(choice: CoopRpsChoice) -> &'static str {
    match choice {
        CoopRpsChoice::Rock => "稳扎稳打，克制剪刀。",
        CoopRpsChoice::Paper => "以柔克刚，克制石头。",
        CoopRpsChoice::Scissors => "果断出击，克制布。",
    }
}

fn door_position(dir: crate::gameplay::map::room::Direction) -> Vec3 {
    match dir {
        crate::gameplay::map::room::Direction::Right => {
            Vec3::new(ROOM_HALF_WIDTH - 10.0, 0.0, 10.0)
        }
        crate::gameplay::map::room::Direction::Left => {
            Vec3::new(-(ROOM_HALF_WIDTH - 10.0), 0.0, 10.0)
        }
        crate::gameplay::map::room::Direction::Up => Vec3::new(0.0, ROOM_HALF_HEIGHT - 10.0, 10.0),
        crate::gameplay::map::room::Direction::Down => {
            Vec3::new(0.0, -(ROOM_HALF_HEIGHT - 10.0), 10.0)
        }
    }
}

fn door_size(dir: crate::gameplay::map::room::Direction) -> Vec2 {
    match dir {
        crate::gameplay::map::room::Direction::Up | crate::gameplay::map::room::Direction::Down => {
            Vec2::new(96.0, 46.0)
        }
        crate::gameplay::map::room::Direction::Left
        | crate::gameplay::map::room::Direction::Right => Vec2::new(46.0, 96.0),
    }
}

fn rps_choice_label(choice: super::components::CoopRpsChoice) -> &'static str {
    match choice {
        super::components::CoopRpsChoice::Rock => "石头",
        super::components::CoopRpsChoice::Paper => "布",
        super::components::CoopRpsChoice::Scissors => "剪刀",
    }
}

fn set_text_if_changed(text: &mut Text, value: &str) {
    if text.sections[0].value != value {
        text.sections[0].value = value.to_string();
    }
}

fn client_replicated_player_score(
    slot: Option<PlayerSlot>,
    local_slot: PlayerSlot,
    controlled: bool,
    local_controlled: bool,
    ghost: Option<GhostState>,
    health: Option<&Health>,
) -> i32 {
    let mut score = 0;
    if slot == Some(local_slot) {
        score += 200;
    }
    if controlled {
        score += 500;
    }
    if local_controlled {
        score += 120;
    }
    if coop_player_is_alive(ghost, health) {
        score += 80;
    } else if matches!(ghost, Some(GhostState::Ghost)) {
        score -= 40;
    }
    if let Some(health) = health {
        score += health.current.round() as i32;
    }
    score
}

fn coop_player_is_alive(ghost: Option<GhostState>, health: Option<&Health>) -> bool {
    match (ghost, health) {
        (Some(GhostState::Ghost), Some(health)) => health.current > 0.0,
        (Some(GhostState::Ghost), None) => false,
        (_, Some(health)) if health.current > 0.0 => true,
        (Some(GhostState::Alive), _) => true,
        (None, Some(health)) => health.current > 0.0,
        (None, None) => true,
    }
}

fn slot_for_mode(mode: NetMode) -> PlayerSlot {
    match mode {
        NetMode::Server | NetMode::Host | NetMode::None => PlayerSlot::P1,
        NetMode::Client => PlayerSlot::P2,
    }
}

fn slot_for_config(config: &CoopNetConfig) -> PlayerSlot {
    if config.mode == NetMode::Client {
        slot_for_client_id(configured_client_id(config)).unwrap_or(PlayerSlot::P2)
    } else {
        slot_for_mode(config.mode)
    }
}

fn best_effort_host_share_ip() -> Option<Ipv4Addr> {
    let candidates = [
        "192.168.0.1:80",
        "10.0.0.1:80",
        "172.16.0.1:80",
        "8.8.8.8:80",
    ];

    for candidate in candidates {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
        if socket.connect(candidate).is_err() {
            continue;
        }
        let Ok(local_addr) = socket.local_addr() else {
            continue;
        };
        let IpAddr::V4(ipv4) = local_addr.ip() else {
            continue;
        };
        if !ipv4.is_loopback() && !ipv4.is_unspecified() {
            return Some(ipv4);
        }
    }

    None
}

fn player_slot_color(slot: PlayerSlot) -> Color {
    match slot {
        PlayerSlot::P1 => Color::srgb(0.92, 1.0, 0.94),
        PlayerSlot::P2 => Color::srgb(0.90, 0.98, 1.0),
    }
}

pub fn predict_local_player_animation(
    input: Res<PlayerInputState>,
    time: Res<Time>,
    mut q: Query<&mut LocalAnimPrediction, (With<Player>, With<LocalControlled>, With<Replicated>)>,
) {
    for mut pred in &mut q {
        pred.override_timer_s = (pred.override_timer_s - time.delta_seconds()).max(0.0);

        if input.dash_pressed {
            pred.predicted_anim = AnimationState::Dash;
            pred.override_timer_s = 0.3;
        } else if input.attack_pressed || input.ranged_pressed {
            pred.predicted_anim = AnimationState::Attack;
            pred.override_timer_s = 0.25;
        } else if input.move_axis.length_squared() > 0.01 && pred.override_timer_s <= 0.0 {
            pred.predicted_anim = AnimationState::Move;
            pred.override_timer_s = 0.12;
        }
    }
}

pub fn client_receive_damage_events(
    config: Res<CoopNetConfig>,
    mut commands: Commands,
    assets: Option<Res<GameAssets>>,
    mut events: EventReader<LyClientMessageEvent<CoopDamageEvent>>,
) {
    if config.mode != NetMode::Client {
        events.clear();
        return;
    }
    let Some(assets) = assets else {
        events.clear();
        return;
    };
    for ev in events.read() {
        let msg = ev.message();
        let text_color = if msg.is_crit {
            Color::srgb(1.0, 0.95, 0.35)
        } else if msg.attacker_is_player {
            Color::srgb(0.85, 1.0, 0.85)
        } else {
            Color::srgb(1.0, 0.75, 0.75)
        };
        commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    format!("{:.0}", msg.amount.max(0.0)),
                    TextStyle {
                        font: assets.font.clone(),
                        font_size: if msg.is_crit { 28.0 } else { 22.0 },
                        color: text_color,
                    },
                ),
                transform: Transform::from_translation(
                    (msg.pos + Vec2::new(0.0, 18.0)).extend(UI_Z - 6.0),
                ),
                ..default()
            },
            DamageNumber {
                timer: Timer::from_seconds(0.75, TimerMode::Once),
                velocity: Vec2::new(0.0, 80.0),
            },
            InGameEntity,
            Name::new("DamageNumber"),
        ));
    }
}
