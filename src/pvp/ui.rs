use bevy::app::AppExit;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use crate::coop::net::{
    COOP_PORT, CoopNetConfig, CoopNetState, CoopSessionFlow, HOST_CLIENT_ID,
    NetMode as CoopNetMode, REMOTE_CLIENT_ID, begin_coop_lobby_session, normalize_coop_host_ip,
};
use crate::core::assets::GameAssets;
use crate::states::AppState;
use crate::ui::widgets;

use super::components::PvpEntity;
use super::net::{
    NetMode, PVP_PORT, PvpNetConfig, PvpNetState, start_client_socket, start_host_socket,
};

#[derive(Component)]
pub struct PvpMenuUi;

#[derive(Component)]
pub struct MultiplayerMenuUi;

#[derive(Component)]
pub struct MultiplayerMenuContent;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiplayerMenuButton {
    CreateRoom,
    JoinRoom,
    SelectMode(MultiplayerTargetMode),
    SelectField(RoomFormField),
    Connect,
    Back,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RoomFormFieldText(RoomFormField);

#[derive(Component)]
pub struct RoomFormNoticeText;

#[derive(Component)]
pub struct PvpLobbyUi;

#[derive(Component)]
pub struct PvpResultUi;

#[derive(Component)]
pub struct PvpLobbyText;

#[derive(Component)]
pub struct PvpIpText;

#[derive(Resource, Debug, Default, Clone)]
pub struct PvpJoinIp {
    pub ip: String,
}

#[derive(Resource, Debug, Clone)]
pub struct MultiplayerRoomForm {
    page: MultiplayerMenuPage,
    intent: RoomIntent,
    target_mode: Option<MultiplayerTargetMode>,
    active_field: RoomFormField,
    ip: String,
    port: String,
    nickname: String,
    room_id: String,
    notice: String,
}

impl Default for MultiplayerRoomForm {
    fn default() -> Self {
        Self {
            page: MultiplayerMenuPage::Entry,
            intent: RoomIntent::Join,
            target_mode: None,
            active_field: RoomFormField::Ip,
            ip: "127.0.0.1".to_string(),
            port: COOP_PORT.to_string(),
            nickname: "Player".to_string(),
            room_id: "main".to_string(),
            notice: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MultiplayerMenuPage {
    Entry,
    CreateMode,
    ConnectionForm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoomIntent {
    Create,
    Join,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiplayerTargetMode {
    Coop,
    Pvp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomFormField {
    Ip,
    Port,
    Nickname,
    RoomId,
}

pub fn setup_multiplayer_menu(mut commands: Commands, assets: Res<GameAssets>) {
    let form = MultiplayerRoomForm::default();
    commands.insert_resource(form.clone());
    commands
        .spawn((
            widgets::root_node(),
            MultiplayerMenuUi,
            PvpEntity,
            Name::new("MultiplayerMenuRoot"),
        ))
        .with_children(|root| {
            spawn_multiplayer_menu_content(root, &assets, &form);
        });
}

pub fn multiplayer_menu_button_system(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut key_events: EventReader<KeyboardInput>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut interaction_q: Query<
        (&Interaction, &MultiplayerMenuButton, &mut BackgroundColor),
        With<Button>,
    >,
    mut form: ResMut<MultiplayerRoomForm>,
    root_q: Query<Entity, (With<MultiplayerMenuUi>, Without<MultiplayerMenuContent>)>,
    content_q: Query<Entity, With<MultiplayerMenuContent>>,
    mut text_q: ParamSet<(
        Query<(&RoomFormFieldText, &mut Text)>,
        Query<&mut Text, With<RoomFormNoticeText>>,
    )>,
    mut coop_config: ResMut<CoopNetConfig>,
    mut coop_net: ResMut<CoopNetState>,
    mut coop_flow: ResMut<CoopSessionFlow>,
    mut pvp_config: ResMut<PvpNetConfig>,
    mut pvp_net: ResMut<PvpNetState>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let mut selected_action = None;
    for (interaction, action, mut color) in &mut interaction_q {
        color.0 = match *interaction {
            Interaction::Hovered => widgets::button_hover_color(),
            Interaction::Pressed => widgets::button_selected_color(),
            Interaction::None => widgets::button_base_color(),
        };
        if *interaction == Interaction::Pressed {
            selected_action = Some(*action);
        }
    }

    if form.page == MultiplayerMenuPage::ConnectionForm {
        edit_room_form(&mut key_events, &keyboard, &mut form);
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        selected_action = Some(MultiplayerMenuButton::Back);
    }
    if keyboard.just_pressed(KeyCode::Enter) && form.page == MultiplayerMenuPage::ConnectionForm {
        selected_action = Some(MultiplayerMenuButton::Connect);
    }

    let mut rerender = false;
    match selected_action {
        Some(MultiplayerMenuButton::CreateRoom) => {
            form.page = MultiplayerMenuPage::CreateMode;
            form.intent = RoomIntent::Create;
            form.target_mode = None;
            form.notice.clear();
            rerender = true;
        }
        Some(MultiplayerMenuButton::JoinRoom) => {
            form.page = MultiplayerMenuPage::ConnectionForm;
            form.intent = RoomIntent::Join;
            form.target_mode = None;
            form.port = COOP_PORT.to_string();
            form.notice = "加入房间：端口 3457 连接协同，端口 3456 连接对抗。".to_string();
            rerender = true;
        }
        Some(MultiplayerMenuButton::SelectMode(mode)) => {
            form.page = MultiplayerMenuPage::ConnectionForm;
            form.intent = RoomIntent::Create;
            form.target_mode = Some(mode);
            form.port = match mode {
                MultiplayerTargetMode::Coop => COOP_PORT,
                MultiplayerTargetMode::Pvp => PVP_PORT,
            }
            .to_string();
            form.notice.clear();
            rerender = true;
        }
        Some(MultiplayerMenuButton::SelectField(field)) => {
            form.active_field = field;
            rerender = true;
        }
        Some(MultiplayerMenuButton::Connect) => {
            connect_room_form(
                &mut form,
                &mut coop_config,
                &mut coop_net,
                &mut coop_flow,
                &mut pvp_config,
                &mut pvp_net,
                &mut next_state,
            );
        }
        Some(MultiplayerMenuButton::Back) => match form.page {
            MultiplayerMenuPage::Entry => next_state.set(AppState::MainMenu),
            MultiplayerMenuPage::CreateMode => {
                form.page = MultiplayerMenuPage::Entry;
                form.notice.clear();
                rerender = true;
            }
            MultiplayerMenuPage::ConnectionForm => {
                form.page = if form.intent == RoomIntent::Create {
                    MultiplayerMenuPage::CreateMode
                } else {
                    MultiplayerMenuPage::Entry
                };
                form.notice.clear();
                rerender = true;
            }
        },
        None => {}
    }

    update_room_form_text(&form, &mut text_q);

    if rerender {
        for entity in &content_q {
            commands.entity(entity).despawn_recursive();
        }
        if let Ok(root) = root_q.get_single() {
            commands
                .entity(root)
                .with_children(|root| spawn_multiplayer_menu_content(root, &assets, &form));
        }
    }
}

pub fn cleanup_multiplayer_menu(mut commands: Commands, q: Query<Entity, With<MultiplayerMenuUi>>) {
    for e in &q {
        commands.entity(e).despawn_recursive();
    }
    commands.remove_resource::<MultiplayerRoomForm>();
}

pub fn setup_pvp_menu(mut commands: Commands, assets: Res<GameAssets>) {
    commands.init_resource::<PvpJoinIp>();
    commands
        .spawn((
            widgets::root_node(),
            PvpMenuUi,
            PvpEntity,
            Name::new("PvpMenuRoot"),
        ))
        .with_children(|root| {
            root.spawn(widgets::panel_node(Color::srgba(0.05, 0.06, 0.10, 0.9)))
                .with_children(|panel| {
                    panel.spawn(widgets::title_text(&assets, "局域网 2P 对抗（PVP）", 42.0));
                    panel.spawn(widgets::title_text(
                        &assets,
                        "H=创建主机  J/Enter=加入  Esc=返回",
                        18.0,
                    ));
                    panel.spawn((widgets::title_text(&assets, "房主IP：", 18.0), PvpIpText));
                });
        });
}

pub fn pvp_menu_input_system(
    mut key_events: EventReader<KeyboardInput>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut ip: ResMut<PvpJoinIp>,
    mut ip_text_q: Query<&mut Text, With<PvpIpText>>,
    mut config: ResMut<PvpNetConfig>,
    mut net: ResMut<PvpNetState>,
    mut next: ResMut<NextState<AppState>>,
) {
    let Ok(mut ip_text) = ip_text_q.get_single_mut() else {
        return;
    };

    if keyboard.just_pressed(KeyCode::Escape) {
        super::net::reset_pvp_network(&mut config, &mut net);
        next.set(AppState::MultiplayerMenu);
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyH) {
        config.mode = NetMode::Host;
        let _ = start_host_socket(&mut net, config.port);
        next.set(AppState::PvpLobby);
        return;
    }

    for ev in key_events.read() {
        if !ev.state.is_pressed() {
            continue;
        }
        if let Key::Character(ref s) = ev.logical_key {
            for c in s.chars() {
                if (c.is_ascii_digit() || c == '.' || c == ':') && ip.ip.len() < 64 {
                    ip.ip.push(c);
                }
            }
        }
    }
    if keyboard.just_pressed(KeyCode::Backspace) {
        ip.ip.pop();
    }

    if keyboard.just_pressed(KeyCode::KeyJ) || keyboard.just_pressed(KeyCode::Enter) {
        let host = ip.ip.trim();
        if !host.is_empty() {
            config.mode = NetMode::Client;
            config.host_ip = host.to_string();
            let _ = start_client_socket(&mut net);
            if let Ok(addr) = format!("{}:{}", host, config.port).parse() {
                net.peer = Some(addr);
            }
            next.set(AppState::PvpLobby);
        }
    }

    ip_text.sections[0].value = format!("房主IP：{}", ip.ip);
}

pub fn cleanup_pvp_menu(mut commands: Commands, q: Query<Entity, With<PvpMenuUi>>) {
    for e in &q {
        commands.entity(e).despawn_recursive();
    }
}

pub fn setup_pvp_lobby(mut commands: Commands, assets: Res<GameAssets>) {
    commands
        .spawn((
            widgets::root_node(),
            PvpLobbyUi,
            PvpEntity,
            Name::new("PvpLobbyRoot"),
        ))
        .with_children(|root| {
            root.spawn(widgets::panel_node(Color::srgba(0.05, 0.06, 0.10, 0.9)))
                .with_children(|panel| {
                    panel.spawn(widgets::title_text(&assets, "PVP 联机大厅", 46.0));
                    panel.spawn((
                        widgets::title_text(&assets, "连接中...", 18.0),
                        PvpLobbyText,
                    ));
                    panel.spawn(widgets::title_text(&assets, "Esc=取消并返回菜单", 18.0));
                });
        });
}

pub fn pvp_lobby_ui_system(
    config: Res<PvpNetConfig>,
    net: Res<PvpNetState>,
    mut q: Query<&mut Text, With<PvpLobbyText>>,
) {
    let Ok(mut text) = q.get_single_mut() else {
        return;
    };
    let status = match config.mode {
        NetMode::Host => {
            if net.connected {
                format!(
                    "已连接：客户端 {}",
                    net.peer.map(|p| p.to_string()).unwrap_or_default()
                )
            } else {
                format!(
                    "房主已启动，等待客户端连接（端口 {}，房间 {}）",
                    config.port, config.room_id
                )
            }
        }
        NetMode::Client => {
            if net.connected {
                format!("已连接到服务器：{}:{}", config.host_ip, config.port)
            } else {
                format!(
                    "正在连接：{}:{}，如断开会自动重试",
                    config.host_ip, config.port
                )
            }
        }
        NetMode::Server => {
            if net.connected {
                format!("PVP 服务器已开始对战（端口 {}）", config.port)
            } else {
                format!("PVP 服务器等待两个客户端连接（端口 {}）", config.port)
            }
        }
        NetMode::None => "尚未选择模式".to_string(),
    };
    text.sections[0].value = status;
}

pub fn pvp_lobby_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<PvpNetConfig>,
    mut net: ResMut<PvpNetState>,
    mut next: ResMut<NextState<AppState>>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    super::net::reset_pvp_network(&mut config, &mut net);
    next.set(AppState::MultiplayerMenu);
}

pub fn cleanup_pvp_lobby(mut commands: Commands, q: Query<Entity, With<PvpLobbyUi>>) {
    for e in &q {
        commands.entity(e).despawn_recursive();
    }
}

pub fn setup_pvp_result(mut commands: Commands, assets: Res<GameAssets>, net: Res<PvpNetState>) {
    let winner = net.winner.unwrap_or(0);
    let title = if winner == 1 || winner == 2 {
        format!("P{winner} 获胜！")
    } else {
        "对局结束".to_string()
    };

    commands
        .spawn((
            widgets::root_node(),
            PvpResultUi,
            PvpEntity,
            Name::new("PvpResultRoot"),
        ))
        .with_children(|root| {
            root.spawn(widgets::panel_node(Color::srgba(0.05, 0.06, 0.10, 0.9)))
                .with_children(|panel| {
                    panel.spawn(widgets::title_text(&assets, title, 56.0));
                    panel.spawn(widgets::title_text(&assets, "Enter=返回联机菜单", 22.0));
                    panel.spawn(widgets::title_text(&assets, "Esc=退出游戏", 18.0));
                });
        });
}

pub fn pvp_result_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<AppState>>,
    mut exit: EventWriter<AppExit>,
    mut config: ResMut<PvpNetConfig>,
    mut net: ResMut<PvpNetState>,
) {
    if keyboard.just_pressed(KeyCode::Enter) {
        super::net::reset_pvp_network(&mut config, &mut net);
        next.set(AppState::MultiplayerMenu);
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        let _ = exit.send(AppExit::Success);
    }
}

pub fn cleanup_pvp_result(mut commands: Commands, q: Query<Entity, With<PvpResultUi>>) {
    for e in &q {
        commands.entity(e).despawn_recursive();
    }
}

fn spawn_multiplayer_menu_content(
    parent: &mut ChildBuilder,
    assets: &GameAssets,
    form: &MultiplayerRoomForm,
) {
    parent
        .spawn((
            widgets::modal_panel_node(720.0),
            MultiplayerMenuContent,
            Name::new("MultiplayerMenuContent"),
        ))
        .with_children(|panel| match form.page {
            MultiplayerMenuPage::Entry => spawn_room_entry_page(panel, assets),
            MultiplayerMenuPage::CreateMode => spawn_create_mode_page(panel, assets),
            MultiplayerMenuPage::ConnectionForm => spawn_connection_form(panel, assets, form),
        });
}

fn spawn_room_entry_page(parent: &mut ChildBuilder, assets: &GameAssets) {
    parent.spawn(widgets::title_text(assets, "联机游戏", 52.0));
    parent.spawn(widgets::muted_text(
        assets,
        "请选择创建房间或加入房间。",
        18.0,
    ));
    spawn_large_menu_button(
        parent,
        assets,
        "创建房间",
        "选择联机模式后连接服务器",
        MultiplayerMenuButton::CreateRoom,
    );
    spawn_large_menu_button(
        parent,
        assets,
        "加入房间",
        "输入服务器信息并加入房间",
        MultiplayerMenuButton::JoinRoom,
    );
    spawn_plain_button(parent, assets, "返回", MultiplayerMenuButton::Back);
}

fn spawn_create_mode_page(parent: &mut ChildBuilder, assets: &GameAssets) {
    parent.spawn(widgets::title_text(assets, "创建房间", 48.0));
    parent.spawn(widgets::muted_text(
        assets,
        "请选择房间使用的联机模式。",
        18.0,
    ));
    spawn_large_menu_button(
        parent,
        assets,
        "联机协同",
        "两名客户端连接到独立协同服务器",
        MultiplayerMenuButton::SelectMode(MultiplayerTargetMode::Coop),
    );
    spawn_large_menu_button(
        parent,
        assets,
        "联机对抗",
        "启动对抗主机并等待另一名玩家连接",
        MultiplayerMenuButton::SelectMode(MultiplayerTargetMode::Pvp),
    );
    spawn_plain_button(parent, assets, "返回", MultiplayerMenuButton::Back);
}

fn spawn_connection_form(
    parent: &mut ChildBuilder,
    assets: &GameAssets,
    form: &MultiplayerRoomForm,
) {
    parent.spawn(widgets::title_text(assets, "连接服务器", 46.0));
    parent.spawn(widgets::muted_text(assets, room_form_subtitle(form), 16.0));
    parent
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
            ..default()
        })
        .with_children(|fields| {
            spawn_field_button(fields, assets, form, RoomFormField::Ip);
            spawn_field_button(fields, assets, form, RoomFormField::Port);
            spawn_field_button(fields, assets, form, RoomFormField::Nickname);
            spawn_field_button(fields, assets, form, RoomFormField::RoomId);
        });
    parent.spawn((
        widgets::muted_text(assets, room_form_notice(form), 15.0),
        RoomFormNoticeText,
    ));
    spawn_plain_button(parent, assets, "连接", MultiplayerMenuButton::Connect);
    spawn_plain_button(parent, assets, "返回", MultiplayerMenuButton::Back);
}

fn spawn_large_menu_button(
    parent: &mut ChildBuilder,
    assets: &GameAssets,
    title: &str,
    hint: &str,
    action: MultiplayerMenuButton,
) {
    parent
        .spawn((widgets::button_bundle_sized(360.0, 62.0), action))
        .with_children(|button| {
            button.spawn(widgets::title_text(assets, title, 22.0));
            button.spawn(widgets::muted_text(assets, hint, 14.0));
        });
}

fn spawn_plain_button(
    parent: &mut ChildBuilder,
    assets: &GameAssets,
    label: &str,
    action: MultiplayerMenuButton,
) {
    parent
        .spawn((widgets::button_bundle_sized(240.0, 46.0), action))
        .with_children(|button| {
            button.spawn(widgets::title_text(assets, label, 20.0));
        });
}

fn spawn_field_button(
    parent: &mut ChildBuilder,
    assets: &GameAssets,
    form: &MultiplayerRoomForm,
    field: RoomFormField,
) {
    let selected = form.active_field == field;
    parent
        .spawn((
            ButtonBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(42.0),
                    justify_content: JustifyContent::FlexStart,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                    border: UiRect::all(Val::Px(if selected { 2.0 } else { 1.0 })),
                    ..default()
                },
                background_color: BackgroundColor(if selected {
                    widgets::button_info_color()
                } else {
                    widgets::input_color()
                }),
                border_color: BorderColor(if selected {
                    Color::srgb(0.55, 0.78, 1.0)
                } else {
                    Color::srgba(0.35, 0.42, 0.54, 0.6)
                }),
                ..default()
            },
            MultiplayerMenuButton::SelectField(field),
        ))
        .with_children(|button| {
            button.spawn((
                widgets::body_text(assets, room_field_line(form, field), 18.0),
                RoomFormFieldText(field),
            ));
        });
}

fn edit_room_form(
    key_events: &mut EventReader<KeyboardInput>,
    keyboard: &ButtonInput<KeyCode>,
    form: &mut MultiplayerRoomForm,
) {
    for ev in key_events.read() {
        if !ev.state.is_pressed() {
            continue;
        }
        if let Key::Character(ref s) = ev.logical_key {
            for c in s.chars() {
                push_room_form_char(form, c);
            }
        }
    }
    if keyboard.just_pressed(KeyCode::Backspace) {
        room_form_value_mut(form, form.active_field).pop();
    }
    if keyboard.just_pressed(KeyCode::Tab) {
        form.active_field = match form.active_field {
            RoomFormField::Ip => RoomFormField::Port,
            RoomFormField::Port => RoomFormField::Nickname,
            RoomFormField::Nickname => RoomFormField::RoomId,
            RoomFormField::RoomId => RoomFormField::Ip,
        };
    }
}

fn push_room_form_char(form: &mut MultiplayerRoomForm, c: char) {
    let field = form.active_field;
    let value = room_form_value_mut(form, field);
    if value.len() >= 64 {
        return;
    }
    let allowed = match field {
        RoomFormField::Ip => c.is_ascii_digit() || c == '.',
        RoomFormField::Port => c.is_ascii_digit(),
        RoomFormField::Nickname | RoomFormField::RoomId => {
            !c.is_control() && c != ':' && c != '/' && c != '\\'
        }
    };
    if allowed {
        value.push(c);
    }
}

fn connect_room_form(
    form: &mut MultiplayerRoomForm,
    coop_config: &mut CoopNetConfig,
    coop_net: &mut CoopNetState,
    coop_flow: &mut CoopSessionFlow,
    pvp_config: &mut PvpNetConfig,
    pvp_net: &mut PvpNetState,
    next_state: &mut NextState<AppState>,
) {
    let Ok(port) = form.port.trim().parse::<u16>() else {
        form.notice = "端口必须是 1-65535 之间的数字。".to_string();
        return;
    };
    if port == 0 {
        form.notice = "端口必须大于 0。".to_string();
        return;
    }

    let mode = form.target_mode.unwrap_or({
        if port == PVP_PORT {
            MultiplayerTargetMode::Pvp
        } else {
            MultiplayerTargetMode::Coop
        }
    });
    match mode {
        MultiplayerTargetMode::Coop => {
            let Ok(host_ip) = normalize_coop_host_ip(&form.ip) else {
                form.notice = "IP 必须是纯 IPv4 地址，例如 127.0.0.1。".to_string();
                return;
            };
            coop_config.mode = CoopNetMode::Client;
            coop_config.host_ip = host_ip;
            coop_config.client_id = if form.intent == RoomIntent::Create {
                HOST_CLIENT_ID
            } else {
                REMOTE_CLIENT_ID
            };
            coop_config.port = port;
            coop_config.nickname = clean_or_default(&form.nickname, "Player");
            coop_config.room_id = clean_or_default(&form.room_id, "main");
            match begin_coop_lobby_session(coop_config, coop_net, coop_flow) {
                Ok(()) => next_state.set(AppState::CoopLobby),
                Err(err) => form.notice = err,
            }
        }
        MultiplayerTargetMode::Pvp => {
            let room_id = clean_or_default(&form.room_id, "main");
            pvp_config.port = port;
            pvp_config.nickname = clean_or_default(&form.nickname, "Player");
            pvp_config.room_id = room_id;
            if form.intent == RoomIntent::Create {
                pvp_config.mode = NetMode::Host;
                match start_host_socket(pvp_net, port) {
                    Ok(()) => next_state.set(AppState::PvpLobby),
                    Err(err) => form.notice = format!("PVP 房间启动失败：{err}"),
                }
            } else {
                pvp_config.mode = NetMode::Client;
                pvp_config.host_ip = form.ip.trim().to_string();
                match start_client_socket(pvp_net) {
                    Ok(()) => {
                        if let Ok(addr) = format!("{}:{}", pvp_config.host_ip, port).parse() {
                            pvp_net.peer = Some(addr);
                            next_state.set(AppState::PvpLobby);
                        } else {
                            form.notice = "服务器地址格式不正确。".to_string();
                        }
                    }
                    Err(err) => form.notice = format!("PVP 客户端启动失败：{err}"),
                }
            }
        }
    }
}

fn update_room_form_text(
    form: &MultiplayerRoomForm,
    text_q: &mut ParamSet<(
        Query<(&RoomFormFieldText, &mut Text)>,
        Query<&mut Text, With<RoomFormNoticeText>>,
    )>,
) {
    for (field, mut text) in &mut text_q.p0() {
        text.sections[0].value = room_field_line(form, field.0);
    }
    if let Ok(mut text) = text_q.p1().get_single_mut() {
        text.sections[0].value = room_form_notice(form);
    }
}

fn room_form_value_mut(form: &mut MultiplayerRoomForm, field: RoomFormField) -> &mut String {
    match field {
        RoomFormField::Ip => &mut form.ip,
        RoomFormField::Port => &mut form.port,
        RoomFormField::Nickname => &mut form.nickname,
        RoomFormField::RoomId => &mut form.room_id,
    }
}

fn room_form_value(form: &MultiplayerRoomForm, field: RoomFormField) -> &str {
    match field {
        RoomFormField::Ip => &form.ip,
        RoomFormField::Port => &form.port,
        RoomFormField::Nickname => &form.nickname,
        RoomFormField::RoomId => &form.room_id,
    }
}

fn room_field_line(form: &MultiplayerRoomForm, field: RoomFormField) -> String {
    let marker = if form.active_field == field {
        "> "
    } else {
        "  "
    };
    format!(
        "{}{}    {}",
        marker,
        room_field_label(field),
        room_form_value(form, field)
    )
}

fn room_field_label(field: RoomFormField) -> &'static str {
    match field {
        RoomFormField::Ip => "IP",
        RoomFormField::Port => "端口",
        RoomFormField::Nickname => "昵称",
        RoomFormField::RoomId => "房间",
    }
}

fn room_form_subtitle(form: &MultiplayerRoomForm) -> &'static str {
    match (
        form.intent,
        form.target_mode.unwrap_or(MultiplayerTargetMode::Coop),
    ) {
        (RoomIntent::Create, MultiplayerTargetMode::Coop) => {
            "创建协同房间：请先运行 server，再以 1 号客户端连接。"
        }
        (RoomIntent::Create, MultiplayerTargetMode::Pvp) => {
            "创建对抗房间：本机作为对抗主机等待连接。"
        }
        (RoomIntent::Join, _) => "加入房间：输入服务器信息后连接，端口决定协同或对抗。",
    }
}

fn room_form_notice(form: &MultiplayerRoomForm) -> String {
    if !form.notice.is_empty() {
        return form.notice.clone();
    }
    "点击字段后输入；Tab 切换字段；Enter 连接；Esc 返回。".to_string()
}

fn clean_or_default(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.chars().take(32).collect()
    }
}
