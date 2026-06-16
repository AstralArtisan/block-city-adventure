use std::collections::HashSet;

use bevy::prelude::*;
use lightyear::prelude::ClientId;
use lightyear::prelude::Replicated;
use lightyear::prelude::Replicating;
use lightyear::prelude::client::ClientCommands as LyClientCommands;
use lightyear::prelude::server::ConnectionManager as LyServerConnectionManager;
use lightyear::prelude::server::ServerCommands as LyServerCommands;

use crate::constants::{ROOM_HALF_HEIGHT, ROOM_HALF_WIDTH};
use crate::core::assets::GameAssets;
use crate::core::events::{DamageAppliedEvent, DeathEvent, RoomClearedEvent};
use crate::core::input::PlayerInputState;
use crate::data::definitions::RewardScalingConfig;
use crate::data::registry::GameDataRegistry;
use crate::gameplay::combat::components::{Projectile, Team};
use crate::gameplay::enemy::components::Enemy;
use crate::gameplay::enemy::systems::{ClearGrace, EnemySpawnCount, SpawnedForRoom};
use crate::gameplay::map::doors::Door;
use crate::gameplay::map::generator::build_rooms;
use crate::gameplay::map::room::{CurrentRoom, Direction, FloorLayout, RoomId, RoomType};
use crate::gameplay::map::{InGameEntity, RewardRoomGoldBonusSeen, VisitedRooms};
use crate::gameplay::player::animation::PlayerAnim;
use crate::gameplay::player::components::{
    AnimationState, AttackCooldown, AttackPower, Combo, CritChance, DashCooldown, DashState,
    Energy, FacingDirection, Gold, Health, InvincibilityTimer, MoveSpeed, Player, PlayerDriveInput,
    RangedCooldown, RangedRapidFire, RewardModifiers, TeamMarker, Velocity,
};
use crate::gameplay::progression::floor::FloorNumber;
use crate::gameplay::puzzle::{ActivePuzzle, reset_active_puzzle};
use crate::gameplay::rewards::apply::{
    apply_reward_to_player_components, attack_power_gain, attack_speed_gain_s, crit_gain,
    dash_cooldown_gain_s, heal_amount, max_health_gain, move_speed_gain,
};

fn scaling_from_registry(data: &Option<Res<GameDataRegistry>>) -> RewardScalingConfig {
    data.as_ref()
        .map(|d| d.rewards.scaling.clone())
        .unwrap_or_else(RewardScalingConfig::default_config)
}
use crate::gameplay::rewards::data::RewardType;
use crate::gameplay::shop::next_refresh_cost;
use crate::states::{AppState, RoomState};
use crate::utils::entity::{safe_despawn_recursive, safe_insert_bundle};
use crate::utils::rng::GameRng;

use super::components::{
    BufferedCoopInput, CoopDamageEvent, CoopDashVisualState, CoopDoorOption, CoopMeleeFlashState,
    CoopNetPosition, CoopNetRotation, CoopNetVelocity, CoopParticipant, CoopPhase, CoopRewardMode,
    CoopRewardOption, CoopRewardSelectionGroup, CoopRpsChoice, CoopSessionEntity, CoopSessionState,
    CoopShopItem, CoopShopOffer, GhostState, LocalControlled, PlayerRewardState, PlayerShopState,
    PlayerSlot, RemoteControlled,
};
use super::net::{
    CoopCommandMessage, CoopExitDestination, CoopExitRequest, CoopNetConfig, CoopNetState,
    CoopSessionFlow, NetMode, build_input_state, build_player_replication, build_replicate_all,
    clear_coop_network_runtime, client_id_for_slot, host_client_id, is_coop_authority,
    latest_input_for, queue_exit_request, remote_client_id, take_exit_request,
    take_received_commands,
};

const DOOR_INTERACT_RANGE: f32 = 76.0;
const REVIVE_HEALTH_FRACTION: f32 = 0.5;
const COOP_DOOR_CONFIRM_TIMEOUT_S: f32 = 2.0;
const COOP_RPS_INPUT_TIMEOUT_S: f32 = 12.0;
const COOP_INPUT_DISCONNECT_TIMEOUT_FRAMES: u32 = 300;

fn normalize_coop_room_type(room_type: RoomType) -> RoomType {
    if room_type == RoomType::Event {
        RoomType::Normal
    } else {
        room_type
    }
}

fn normalize_coop_layout(layout: &mut FloorLayout) {
    for room in &mut layout.rooms {
        room.room_type = normalize_coop_room_type(room.room_type);
    }
}

pub fn is_coop_simulation_active(
    session_q: Query<&CoopSessionState, With<CoopSessionEntity>>,
) -> bool {
    session_q
        .get_single()
        .map(|session| session.phase != CoopPhase::Paused)
        .unwrap_or(true)
}

pub struct CoopRuntimePlugin;

impl Plugin for CoopRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CoopRuntimeState>()
            .add_systems(OnEnter(AppState::CoopGame), host_bootstrap_match)
            .add_systems(Update, process_pending_exit_request)
            .add_systems(
                Update,
                (
                    host_bootstrap_match,
                    host_handle_pause_requests,
                    host_refresh_session_state,
                    host_cleanup_disconnected_session,
                )
                    .run_if(in_state(AppState::CoopGame).and_then(is_coop_authority)),
            )
            .add_systems(
                Update,
                (
                    host_buffer_player_inputs,
                    host_handle_coop_player_deaths,
                    host_enter_room_phase,
                    host_enter_reward_phase_on_room_clear,
                    host_process_phase_commands,
                    host_handle_shop_exit_inputs,
                    host_handle_door_interactions,
                    host_tick_rps_resolution,
                    host_tag_replicated_entities,
                    host_sync_network_views,
                    host_sync_dash_cooldowns,
                    host_broadcast_damage_events,
                )
                    .chain()
                    .run_if(
                        in_state(AppState::CoopGame)
                            .and_then(is_coop_authority)
                            .and_then(is_coop_simulation_active),
                    ),
            );
    }
}

#[derive(Resource, Debug, Default)]
pub struct CoopRuntimeState {
    pub bootstrapped: bool,
    pub reward_rooms_seen: HashSet<u32>,
    pub shop_rooms_seen: HashSet<u32>,
    pub last_room_seen: Option<u32>,
    pub pending_next_floor: bool,
    pub pending_victory: bool,
}

pub fn reset_coop_runtime(
    mut commands: Commands,
    mut runtime: ResMut<CoopRuntimeState>,
    mut config: ResMut<CoopNetConfig>,
    mut net: ResMut<CoopNetState>,
    mut flow: ResMut<CoopSessionFlow>,
    visited: Option<ResMut<VisitedRooms>>,
    reward_bonus_seen: Option<ResMut<RewardRoomGoldBonusSeen>>,
    world_q: Query<Entity, With<InGameEntity>>,
    player_q: Query<Entity, With<CoopParticipant>>,
) {
    runtime.bootstrapped = false;
    runtime.reward_rooms_seen.clear();
    runtime.shop_rooms_seen.clear();
    runtime.last_room_seen = None;
    runtime.pending_next_floor = false;
    runtime.pending_victory = false;

    let mut to_despawn: HashSet<Entity> = HashSet::default();
    for entity in &world_q {
        to_despawn.insert(entity);
    }
    for entity in &player_q {
        to_despawn.insert(entity);
    }
    for entity in to_despawn {
        safe_despawn_recursive(&mut commands, entity);
    }

    commands.remove_resource::<FloorLayout>();
    commands.remove_resource::<CurrentRoom>();
    commands.remove_resource::<RoomState>();
    commands.remove_resource::<FloorNumber>();
    if let Some(mut visited) = visited {
        visited.0.clear();
    } else {
        commands.insert_resource(VisitedRooms::default());
    }
    if let Some(mut reward_bonus_seen) = reward_bonus_seen {
        reward_bonus_seen.0.clear();
    } else {
        commands.insert_resource(RewardRoomGoldBonusSeen::default());
    }

    super::net::reset_coop_network(&mut config, &mut net);
    super::net::reset_coop_flow(&mut flow);
}

fn process_pending_exit_request(
    mut commands: Commands,
    mut runtime: ResMut<CoopRuntimeState>,
    mut config: ResMut<CoopNetConfig>,
    mut net: ResMut<CoopNetState>,
    mut flow: ResMut<CoopSessionFlow>,
    visited: Option<ResMut<VisitedRooms>>,
    reward_bonus_seen: Option<ResMut<RewardRoomGoldBonusSeen>>,
    mut next_state: ResMut<NextState<AppState>>,
    world_q: Query<Entity, With<InGameEntity>>,
    player_q: Query<Entity, With<CoopParticipant>>,
) {
    let Some(request) = take_exit_request(&mut flow) else {
        return;
    };

    runtime.bootstrapped = false;
    runtime.reward_rooms_seen.clear();
    runtime.shop_rooms_seen.clear();
    runtime.last_room_seen = None;
    runtime.pending_next_floor = false;
    runtime.pending_victory = false;

    let mut to_despawn: HashSet<Entity> = HashSet::default();
    for entity in &world_q {
        to_despawn.insert(entity);
    }
    for entity in &player_q {
        to_despawn.insert(entity);
    }
    for entity in to_despawn {
        safe_despawn_recursive(&mut commands, entity);
    }

    commands.remove_resource::<FloorLayout>();
    commands.remove_resource::<CurrentRoom>();
    commands.remove_resource::<RoomState>();
    commands.remove_resource::<FloorNumber>();
    if let Some(mut visited) = visited {
        visited.0.clear();
    } else {
        commands.insert_resource(VisitedRooms::default());
    }
    if let Some(mut reward_bonus_seen) = reward_bonus_seen {
        reward_bonus_seen.0.clear();
    } else {
        commands.insert_resource(RewardRoomGoldBonusSeen::default());
    }

    if net.client_started {
        commands.disconnect_client();
    }
    if net.server_started {
        commands.stop_server();
    }
    let should_rearm_dedicated_server = config.mode == NetMode::Server
        && request.preserve_mode
        && request.destination == CoopExitDestination::Lobby;
    clear_coop_network_runtime(&mut net);
    flow.pending_game_entry = should_rearm_dedicated_server;
    flow.pending_exit = None;

    if request.preserve_mode {
        flow.lobby_notice = request.notice.unwrap_or_default();
    } else {
        flow.lobby_notice.clear();
        config.mode = NetMode::None;
        config.host_ip.clear();
        config.client_id = 0;
    }

    next_state.set(match request.destination {
        CoopExitDestination::Lobby => AppState::CoopLobby,
        CoopExitDestination::MainMenu => AppState::MainMenu,
        CoopExitDestination::MultiplayerMenu => AppState::MultiplayerMenu,
    });
}

fn host_bootstrap_match(
    mut commands: Commands,
    config: Res<CoopNetConfig>,
    net: Res<CoopNetState>,
    assets: Option<Res<GameAssets>>,
    data: Option<Res<GameDataRegistry>>,
    mut rng: ResMut<GameRng>,
    mut runtime: ResMut<CoopRuntimeState>,
    existing_players: Query<(), With<CoopParticipant>>,
) {
    let players_connected = match config.mode {
        NetMode::Host => net.local_connected && net.remote_connected,
        NetMode::Server => {
            net.connected_clients.contains(&host_client_id())
                && net.connected_clients.contains(&remote_client_id())
        }
        NetMode::Client | NetMode::None => false,
    };
    if !players_connected || runtime.bootstrapped || existing_players.iter().next().is_some() {
        return;
    }
    let (Some(assets), Some(data)) = (assets, data) else {
        return;
    };

    commands.insert_resource(FloorNumber(1));
    commands.insert_resource(VisitedRooms::default());
    commands.insert_resource(RewardRoomGoldBonusSeen::default());
    commands.insert_resource(RoomState::Idle);

    let mut layout = FloorLayout {
        rooms: build_rooms(Some(data.as_ref()), 1, &mut rng),
        current: RoomId(0),
    };
    normalize_coop_layout(&mut layout);
    commands.insert_resource(CurrentRoom(layout.current));
    commands.insert_resource(layout);

    let p1 = spawn_coop_player(
        &mut commands,
        &assets,
        &data,
        PlayerSlot::P1,
        Vec3::new(-220.0, 0.0, 50.0),
    );
    let mut p1_entity = commands.entity(p1);
    p1_entity.insert((
        build_player_replication(host_client_id()),
        BufferedCoopInput::default(),
    ));
    if config.mode == NetMode::Host {
        p1_entity.insert(LocalControlled);
    } else {
        p1_entity.insert(RemoteControlled {
            client_id: host_client_id(),
        });
    }

    let p2 = spawn_coop_player(
        &mut commands,
        &assets,
        &data,
        PlayerSlot::P2,
        Vec3::new(-160.0, -42.0, 50.0),
    );
    commands.entity(p2).insert((
        RemoteControlled {
            client_id: remote_client_id(),
        },
        build_player_replication(remote_client_id()),
        BufferedCoopInput::default(),
    ));

    commands.spawn((
        CoopSessionEntity,
        CoopSessionState {
            phase: CoopPhase::None,
            room_state: RoomState::Idle,
            room_type: RoomType::Start,
            current_room: 0,
            floor_number: 1,
            ..default()
        },
        build_replicate_all(),
        InGameEntity,
        Name::new("CoopSession"),
    ));

    runtime.bootstrapped = true;
    runtime.last_room_seen = Some(0);
}

fn host_handle_pause_requests(
    config: Res<CoopNetConfig>,
    local_input: Res<PlayerInputState>,
    net: Res<CoopNetState>,
    mut session_q: Query<&mut CoopSessionState, With<CoopSessionEntity>>,
) {
    let Ok(mut session) = session_q.get_single_mut() else {
        return;
    };
    if session.match_over {
        return;
    }

    let p1_pressed = latest_input_for(&net, host_client_id()).pause_pressed;
    let p2_pressed = latest_input_for(&net, remote_client_id()).pause_pressed;
    let host_pressed = (config.mode == NetMode::Host && local_input.pause_pressed) || p1_pressed;
    if !(host_pressed || p2_pressed) {
        return;
    }

    match session.phase {
        CoopPhase::None => session.phase = CoopPhase::Paused,
        CoopPhase::Paused => session.phase = CoopPhase::None,
        _ => {}
    }
}

fn host_buffer_player_inputs(
    local_input: Res<PlayerInputState>,
    mut net: ResMut<CoopNetState>,
    mut players: Query<
        (&PlayerSlot, &mut BufferedCoopInput, &mut PlayerDriveInput),
        (With<CoopParticipant>, Without<Replicated>),
    >,
) {
    // 递增帧计数器，用于检测 P2 输入是否过期
    net.host_frame_counter = net.host_frame_counter.wrapping_add(1);

    for (slot, mut buffered, mut drive) in &mut players {
        let input = {
            let client_id = slot_client_id(*slot);
            if Some(client_id) == net.local_client_id {
                build_input_state(&local_input)
            } else {
                let mut inp = latest_input_for(&net, slot_client_id(*slot));
                // 检查输入是否过期：超过 3 帧没收到新包则清零持续量，
                // 防止丢包时角色用旧的 move_axis 无限滑行
                let last_tick = net
                    .latest_input_ticks
                    .get(&slot_client_id(*slot))
                    .copied()
                    .unwrap_or(0);
                let age = net.host_frame_counter.wrapping_sub(last_tick);
                if age > 3 {
                    inp.move_axis = Vec2::ZERO;
                    inp.aim_world = None;
                    inp.attack_held = false;
                    inp.ranged_held = false;
                }
                inp
            }
        };
        buffered.0 = input;
        *drive = PlayerDriveInput {
            move_axis: input.move_axis,
            aim_world: input.aim_world,
            attack_pressed: input.attack_pressed,
            attack_held: input.attack_held,
            ranged_pressed: input.ranged_pressed,
            ranged_held: input.ranged_held,
            dash_pressed: input.dash_pressed,
            skill_1_pressed: false,
            skill_2_pressed: false,
            skill_3_pressed: false,
            skill_4_pressed: false,
            interact_pressed: input.interact_pressed,
            pause_pressed: input.pause_pressed,
            shop_pressed: input.shop_pressed,
            menu_confirm_pressed: input.menu_confirm_pressed,
            menu_cancel_pressed: input.menu_cancel_pressed,
        };
        // 消费边缘事件后清除，防止跨帧重复触发
        let client_id = slot_client_id(*slot);
        if Some(client_id) != net.local_client_id
            && let Some(stored) = net.latest_inputs.get_mut(&client_id)
        {
            stored.clear_edge_events();
        }
    }
}

fn host_handle_coop_player_deaths(
    mut death_events: EventReader<DeathEvent>,
    mut session_q: Query<&mut CoopSessionState, With<CoopSessionEntity>>,
    mut players: Query<
        (
            Entity,
            &mut GhostState,
            &mut Sprite,
            &mut DashState,
            &mut Velocity,
            &mut Health,
        ),
        (With<CoopParticipant>, Without<Replicated>),
    >,
) {
    let Ok(mut session) = session_q.get_single_mut() else {
        return;
    };

    for ev in death_events.read() {
        if ev.team != Team::Player {
            continue;
        }
        for (entity, mut ghost, mut sprite, mut dash, mut velocity, mut health) in &mut players {
            if entity != ev.entity || *ghost == GhostState::Ghost {
                continue;
            }
            *ghost = GhostState::Ghost;
            dash.active = false;
            velocity.0 = Vec2::ZERO;
            health.current = 0.0;
            sprite.color.set_alpha(0.42);
        }
    }

    let living_count = players
        .iter()
        .filter(|(_, ghost, _, _, _, _)| **ghost == GhostState::Alive)
        .count();
    if living_count == 0 {
        session.phase = CoopPhase::MatchOver;
        session.match_over = true;
        session.match_victory = false;
    }
}

fn host_enter_room_phase(
    mut runtime: ResMut<CoopRuntimeState>,
    layout: Option<Res<FloorLayout>>,
    current_room: Option<Res<CurrentRoom>>,
    room_state: Option<Res<RoomState>>,
    floor: Option<Res<FloorNumber>>,
    mut rng: ResMut<GameRng>,
    mut session_q: Query<&mut CoopSessionState, With<CoopSessionEntity>>,
    players: Query<
        (&PlayerSlot, &RewardModifiers, &GhostState),
        (With<CoopParticipant>, Without<Replicated>),
    >,
) {
    let (Some(layout), Some(current_room), Some(room_state)) = (layout, current_room, room_state)
    else {
        return;
    };
    let Ok(mut session) = session_q.get_single_mut() else {
        return;
    };
    let room_id = current_room.0.0;
    let room_type = layout
        .room(current_room.0)
        .map(|room| normalize_coop_room_type(room.room_type))
        .unwrap_or(RoomType::Normal);

    if runtime.last_room_seen == Some(room_id) {
        return;
    }

    runtime.last_room_seen = Some(room_id);
    session.phase = CoopPhase::None;
    session.reward = default_reward_state();
    session.shop = default_shop_state();
    session.door_choice = default_door_choice_state();
    session.rps = default_rps_state();
    session.revive.dead_slot = None;
    session.revive.revived = false;

    if room_type == RoomType::Shop && runtime.shop_rooms_seen.insert(room_id) {
        let floor_number = floor.as_deref().map(|value| value.0).unwrap_or(1);
        session.shop = generate_shop_state(floor_number, &mut rng, &players);
        session.phase = CoopPhase::Shop;
        return;
    }

    if room_type == RoomType::Reward
        && runtime.reward_rooms_seen.insert(room_id)
        && *room_state != RoomState::Locked
    {
        session.reward = generate_reward_state(CoopRewardMode::SingleBuff, &mut rng, &players);
        session.phase = CoopPhase::Reward;
    }
}

fn host_enter_reward_phase_on_room_clear(
    mut room_cleared: EventReader<RoomClearedEvent>,
    mut runtime: ResMut<CoopRuntimeState>,
    data: Option<Res<GameDataRegistry>>,
    layout: Option<Res<FloorLayout>>,
    current_room: Option<Res<CurrentRoom>>,
    floor: Option<Res<FloorNumber>>,
    mut rng: ResMut<GameRng>,
    mut session_q: Query<&mut CoopSessionState, With<CoopSessionEntity>>,
    mut players: Query<
        (&PlayerSlot, &RewardModifiers, &GhostState, &mut Health),
        (With<CoopParticipant>, Without<Replicated>),
    >,
) {
    let (Some(layout), Some(current_room)) = (layout, current_room) else {
        return;
    };
    let Some(event) = room_cleared.read().next() else {
        return;
    };
    if event.room != current_room.0 {
        return;
    }
    let Ok(mut session) = session_q.get_single_mut() else {
        return;
    };
    let Some(room) = layout.room(current_room.0) else {
        return;
    };
    let room_type = normalize_coop_room_type(room.room_type);
    if !matches!(room_type, RoomType::Normal | RoomType::Boss) {
        return;
    }
    if !runtime.reward_rooms_seen.insert(current_room.0.0) {
        return;
    }

    let floor_number = floor.as_deref().map(|value| value.0).unwrap_or(1);
    let reward_mode = if room_type == RoomType::Boss {
        for (_, _, ghost, mut health) in &mut players {
            if *ghost != GhostState::Alive {
                continue;
            }
            let heal = health.max * 0.80;
            health.current = (health.current + heal).min(health.max);
        }
        CoopRewardMode::DualBuff
    } else {
        CoopRewardMode::HealOrBuff
    };
    let snapshots = players
        .iter()
        .map(|(slot, mods, ghost, _)| RewardPlayerSnapshot {
            slot: *slot,
            mods: *mods,
            ghost: *ghost,
        })
        .collect::<Vec<_>>();
    session.reward = generate_reward_state_from_snapshots(reward_mode, &mut rng, &snapshots);
    session.phase = CoopPhase::Reward;

    let total_floors = data
        .as_deref()
        .map(|registry| registry.balance.total_floors.max(1))
        .unwrap_or(4);
    runtime.pending_victory = room_type == RoomType::Boss && floor_number >= total_floors;
    runtime.pending_next_floor = room_type == RoomType::Boss && !runtime.pending_victory;
}

fn host_process_phase_commands(
    mut runtime: ResMut<CoopRuntimeState>,
    mut net: ResMut<CoopNetState>,
    data: Option<Res<GameDataRegistry>>,
    mut rng: ResMut<GameRng>,
    mut floor: Option<ResMut<FloorNumber>>,
    mut layout: Option<ResMut<FloorLayout>>,
    mut current_room: Option<ResMut<CurrentRoom>>,
    mut room_state: Option<ResMut<RoomState>>,
    visited: Option<ResMut<VisitedRooms>>,
    reward_bonus_seen: Option<ResMut<RewardRoomGoldBonusSeen>>,
    spawned_for_room: Option<ResMut<SpawnedForRoom>>,
    clear_grace: Option<ResMut<ClearGrace>>,
    spawn_count: Option<ResMut<EnemySpawnCount>>,
    active_puzzle: Option<ResMut<ActivePuzzle>>,
    mut session_q: Query<&mut CoopSessionState, With<CoopSessionEntity>>,
    mut player_queries: ParamSet<(
        Query<
            (
                &PlayerSlot,
                &mut Gold,
                &mut Health,
                &mut Energy,
                &mut MoveSpeed,
                &mut AttackPower,
                &mut CritChance,
                &mut DashCooldown,
                &mut AttackCooldown,
                &mut RangedCooldown,
                &mut RewardModifiers,
                &mut GhostState,
                &mut Transform,
                &mut Sprite,
            ),
            (With<CoopParticipant>, Without<Replicated>),
        >,
        Query<
            (
                &PlayerSlot,
                &mut GhostState,
                &mut Health,
                &mut Transform,
                &mut Sprite,
                &mut PlayerDriveInput,
                &mut Velocity,
                &mut FacingDirection,
                &mut AnimationState,
                &mut PlayerAnim,
                &mut DashState,
                &mut CoopMeleeFlashState,
                &mut CoopDashVisualState,
            ),
            (With<CoopParticipant>, Without<Replicated>),
        >,
        Query<
            (&PlayerSlot, &RewardModifiers, &GhostState),
            (With<CoopParticipant>, Without<Replicated>),
        >,
    )>,
) {
    let Ok(mut session) = session_q.get_single_mut() else {
        return;
    };

    for (client_id, command) in take_received_commands(&mut net) {
        match command {
            CoopCommandMessage::SelectReward { slot, group, index }
                if session.phase == CoopPhase::Reward && slot_client_id(slot) == client_id =>
            {
                if let Some(player_state) = session.reward.players.get_mut(slot_index(slot))
                    && player_state.can_interact
                {
                    let target = match group {
                        CoopRewardSelectionGroup::Heal | CoopRewardSelectionGroup::Primary => {
                            &player_state.primary_options
                        }
                        CoopRewardSelectionGroup::Secondary => &player_state.secondary_options,
                    };
                    if let Some(choice) = target.get(index as usize).copied() {
                        match group {
                            CoopRewardSelectionGroup::Heal | CoopRewardSelectionGroup::Primary => {
                                player_state.selected_primary = Some(choice);
                            }
                            CoopRewardSelectionGroup::Secondary => {
                                player_state.selected_secondary = Some(choice);
                            }
                        }
                    }
                }
            }
            CoopCommandMessage::SelectRps { slot, choice }
                if session.phase == CoopPhase::Rps && slot_client_id(slot) == client_id =>
            {
                match slot {
                    PlayerSlot::P1 => session.rps.p1_choice = Some(choice),
                    PlayerSlot::P2 => session.rps.p2_choice = Some(choice),
                }
            }
            CoopCommandMessage::BuyShopItem { slot, index }
                if session.phase == CoopPhase::Shop && slot_client_id(slot) == client_id =>
            {
                let scaling = scaling_from_registry(&data);
                try_purchase_shop_item(
                    slot,
                    index as usize,
                    &mut session,
                    &scaling,
                    &mut player_queries.p0(),
                );
            }
            CoopCommandMessage::RefreshShop { slot }
                if session.phase == CoopPhase::Shop && slot_client_id(slot) == client_id =>
            {
                let shop_cfg = data.as_deref().map(|d| d.shop.clone()).unwrap_or_default();
                try_refresh_shop(
                    slot,
                    session.floor_number.max(1),
                    &mut session,
                    &mut rng,
                    &shop_cfg,
                    &mut player_queries.p0(),
                );
            }
            _ => {}
        }
    }

    if session.phase == CoopPhase::Reward {
        sync_reward_phase_state(&mut session, &mut rng, &player_queries.p2());
    }

    if session.phase == CoopPhase::Reward && reward_phase_complete(&session) {
        let revive_data = {
            let scaling = scaling_from_registry(&data);
            let mut players = player_queries.p0();
            apply_reward_phase(
                &mut session,
                floor.as_deref().map(|value| value.0).unwrap_or(1),
                &scaling,
                &mut players,
            )
        };
        if let Some((target, anchor)) = revive_data {
            finish_revive_phase(&mut session, target, anchor, &mut player_queries.p1());
        }

        if runtime.pending_victory {
            session.phase = CoopPhase::MatchOver;
            session.match_over = true;
            session.match_victory = true;
            runtime.pending_victory = false;
            runtime.pending_next_floor = false;
            return;
        }

        if runtime.pending_next_floor {
            if let (Some(data), Some(floor), Some(layout), Some(current_room), Some(room_state)) = (
                data.as_deref(),
                floor.as_mut(),
                layout.as_mut(),
                current_room.as_mut(),
                room_state.as_mut(),
            ) {
                let next_floor = floor.0 + 1;
                floor.0 = next_floor;
                **layout = FloorLayout {
                    rooms: build_rooms(Some(data), next_floor, &mut rng),
                    current: RoomId(0),
                };
                normalize_coop_layout(layout.as_mut());
                current_room.0 = RoomId(0);
                **room_state = RoomState::Idle;

                if let Some(mut visited) = visited {
                    visited.0.clear();
                }
                if let Some(mut seen) = reward_bonus_seen {
                    seen.0.clear();
                }
                if let Some(mut spawned) = spawned_for_room {
                    spawned.0 = None;
                }
                if let Some(mut grace) = clear_grace {
                    grace.last_room = None;
                    grace.timer = Timer::from_seconds(0.0, TimerMode::Once);
                }
                if let Some(mut spawn_count) = spawn_count {
                    spawn_count.current = 0;
                }
                if let Some(mut puzzle) = active_puzzle {
                    reset_active_puzzle(&mut puzzle);
                }

                for (slot, _, _, _, _, _, _, _, _, _, _, _, mut transform, mut sprite) in
                    &mut player_queries.p0()
                {
                    transform.translation = coop_spawn_position(*slot);
                    sprite.color.set_alpha(1.0);
                }
                runtime.last_room_seen = None;
            }
            runtime.pending_next_floor = false;
        }
    }
}

fn host_handle_shop_exit_inputs(
    mut session_q: Query<&mut CoopSessionState, With<CoopSessionEntity>>,
    players: Query<
        (&PlayerSlot, &BufferedCoopInput, &GhostState),
        (With<CoopParticipant>, Without<Replicated>),
    >,
) {
    let Ok(mut session) = session_q.get_single_mut() else {
        return;
    };
    if session.phase != CoopPhase::Shop {
        return;
    }

    for (slot, buffered, ghost) in &players {
        if *ghost != GhostState::Alive {
            session.shop.players[slot_index(*slot)].can_interact = false;
            continue;
        }
        if buffered.0.interact_pressed || buffered.0.menu_cancel_pressed {
            session.shop.players[slot_index(*slot)].can_interact = false;
        }
    }

    let active_alive_count = players
        .iter()
        .filter(|(_, _, ghost)| **ghost == GhostState::Alive)
        .count();
    let finished_count = session
        .shop
        .players
        .iter()
        .filter(|state| !state.can_interact)
        .count();
    if active_alive_count > 0 && finished_count >= active_alive_count {
        session.phase = CoopPhase::None;
        session.shop = default_shop_state();
    }
}

fn host_handle_door_interactions(
    time: Res<Time>,
    mut runtime: ResMut<CoopRuntimeState>,
    layout: Option<Res<FloorLayout>>,
    mut current_room: Option<ResMut<CurrentRoom>>,
    mut room_state: Option<ResMut<RoomState>>,
    mut visited: Option<ResMut<VisitedRooms>>,
    mut session_q: Query<&mut CoopSessionState, With<CoopSessionEntity>>,
    mut player_queries: ParamSet<(
        Query<
            (&PlayerSlot, &BufferedCoopInput, &Transform, &GhostState),
            (With<CoopParticipant>, Without<Replicated>),
        >,
        Query<(&PlayerSlot, &mut Transform), (With<CoopParticipant>, Without<Replicated>)>,
    )>,
) {
    let (Some(layout), Some(current_room), Some(room_state)) = (
        layout,
        current_room.as_deref_mut(),
        room_state.as_deref_mut(),
    ) else {
        return;
    };
    let Ok(mut session) = session_q.get_single_mut() else {
        return;
    };
    if !matches!(session.phase, CoopPhase::None | CoopPhase::DoorChoice)
        || matches!(*room_state, RoomState::Locked | RoomState::BossFight)
    {
        return;
    }

    let Some(room) = layout.room(current_room.0) else {
        return;
    };
    if room.connections.exits.is_empty() {
        return;
    }
    let room_type = normalize_coop_room_type(room.room_type);
    if matches!(room_type, RoomType::Normal | RoomType::Boss)
        && *room_state == RoomState::Cleared
        && !runtime.reward_rooms_seen.contains(&current_room.0.0)
    {
        return;
    }

    let mut p1_choice = session.door_choice.p1_choice;
    let mut p2_choice = session.door_choice.p2_choice;
    let had_pending_choice = p1_choice.is_some() ^ p2_choice.is_some();

    let living_slots = {
        let players = player_queries.p0();
        for (slot, buffered, transform, ghost) in &players {
            if *ghost != GhostState::Alive {
                match slot {
                    PlayerSlot::P1 => p1_choice = None,
                    PlayerSlot::P2 => p2_choice = None,
                }
                continue;
            }
            if !buffered.0.interact_pressed {
                continue;
            }
            let player_pos = transform.translation.truncate();
            for (index, (dir, _)) in room.connections.exits.iter().enumerate() {
                if player_pos.distance(door_world_position(*dir)) <= DOOR_INTERACT_RANGE {
                    match slot {
                        PlayerSlot::P1 if p1_choice != Some(index as u8) => {
                            p1_choice = Some(index as u8);
                            session.door_choice.input_timeout_s = COOP_DOOR_CONFIRM_TIMEOUT_S;
                        }
                        PlayerSlot::P2 if p2_choice != Some(index as u8) => {
                            p2_choice = Some(index as u8);
                            session.door_choice.input_timeout_s = COOP_DOOR_CONFIRM_TIMEOUT_S;
                        }
                        _ => {}
                    }
                    break;
                }
            }
        }

        players
            .iter()
            .filter(|(_, _, _, ghost)| **ghost == GhostState::Alive)
            .map(|(slot, _, _, _)| *slot)
            .collect::<Vec<_>>()
    };

    session.door_choice.p1_choice = p1_choice;
    session.door_choice.p2_choice = p2_choice;
    session.door_choice.chooser = match (p1_choice, p2_choice) {
        (Some(_), None) => Some(PlayerSlot::P1),
        (None, Some(_)) => Some(PlayerSlot::P2),
        _ => None,
    };

    match living_slots.as_slice() {
        [slot] => {
            session.phase = CoopPhase::None;
            let choice = match slot {
                PlayerSlot::P1 => p1_choice,
                PlayerSlot::P2 => p2_choice,
            };
            if let Some(choice) = choice {
                advance_to_room(
                    choice,
                    &mut runtime,
                    &mut session,
                    Some(&layout),
                    Some(current_room),
                    Some(room_state),
                    visited.as_deref_mut(),
                    &mut player_queries.p1(),
                );
            }
        }
        [PlayerSlot::P1, PlayerSlot::P2] | [PlayerSlot::P2, PlayerSlot::P1] => {
            if p1_choice.is_some() || p2_choice.is_some() {
                session.phase = CoopPhase::DoorChoice;
            } else {
                session.phase = CoopPhase::None;
            }
            if let (Some(p1), Some(p2)) = (p1_choice, p2_choice) {
                if p1 == p2 {
                    advance_to_room(
                        p1,
                        &mut runtime,
                        &mut session,
                        Some(&layout),
                        Some(current_room),
                        Some(room_state),
                        visited.as_deref_mut(),
                        &mut player_queries.p1(),
                    );
                } else {
                    session.phase = CoopPhase::Rps;
                    session.rps = default_rps_state();
                }
            } else if p1_choice.is_some() || p2_choice.is_some() {
                if !had_pending_choice {
                    session.door_choice.input_timeout_s = COOP_DOOR_CONFIRM_TIMEOUT_S;
                } else {
                    session.door_choice.input_timeout_s =
                        (session.door_choice.input_timeout_s - time.delta_seconds()).max(0.0);
                }
                if session.door_choice.input_timeout_s <= 0.0 {
                    session.door_choice.p1_choice = None;
                    session.door_choice.p2_choice = None;
                    session.door_choice.chooser = None;
                    session.door_choice.input_timeout_s = COOP_DOOR_CONFIRM_TIMEOUT_S;
                    session.phase = CoopPhase::None;
                }
            }
        }
        _ => {
            session.phase = CoopPhase::None;
        }
    }
}

fn host_tick_rps_resolution(
    time: Res<Time>,
    mut rng: ResMut<GameRng>,
    mut runtime: ResMut<CoopRuntimeState>,
    layout: Option<Res<FloorLayout>>,
    mut current_room: Option<ResMut<CurrentRoom>>,
    mut room_state: Option<ResMut<RoomState>>,
    mut visited: Option<ResMut<VisitedRooms>>,
    mut session_q: Query<&mut CoopSessionState, With<CoopSessionEntity>>,
    mut player_transforms: Query<
        (&PlayerSlot, &mut Transform),
        (With<CoopParticipant>, Without<Replicated>),
    >,
) {
    let Ok(mut session) = session_q.get_single_mut() else {
        return;
    };
    if session.phase != CoopPhase::Rps {
        return;
    }

    if session.rps.winner.is_none() {
        if let (Some(p1), Some(p2)) = (session.rps.p1_choice, session.rps.p2_choice) {
            match resolve_rps(p1, p2) {
                Some(winner) => {
                    session.rps.winner = Some(winner);
                    session.rps.winning_door = match winner {
                        PlayerSlot::P1 => session.door_choice.p1_choice,
                        PlayerSlot::P2 => session.door_choice.p2_choice,
                    };
                    session.rps.reveal_timer_s = 0.8;
                }
                None => {
                    reset_rps_input_round(&mut session.rps);
                }
            }
        } else {
            session.rps.input_timeout_s =
                (session.rps.input_timeout_s - time.delta_seconds()).max(0.0);
            if session.rps.input_timeout_s <= 0.0 {
                fill_missing_rps_choices(&mut session.rps, &mut rng);
            }
        }
        return;
    }

    session.rps.reveal_timer_s = (session.rps.reveal_timer_s - time.delta_seconds()).max(0.0);
    if session.rps.reveal_timer_s > 0.0 {
        return;
    }

    if let Some(door_index) = session.rps.winning_door {
        advance_to_room(
            door_index,
            &mut runtime,
            &mut session,
            layout.as_deref(),
            current_room.as_deref_mut(),
            room_state.as_deref_mut(),
            visited.as_deref_mut(),
            &mut player_transforms,
        );
    }
}

fn spawn_coop_player(
    commands: &mut Commands,
    assets: &GameAssets,
    data: &GameDataRegistry,
    slot: PlayerSlot,
    translation: Vec3,
) -> Entity {
    let cfg = &data.player;
    let entity = commands
        .spawn((
            SpriteBundle {
                texture: assets.textures.player.clone(),
                transform: Transform::from_translation(translation),
                sprite: Sprite {
                    color: Color::WHITE,
                    custom_size: Some(Vec2::new(74.0, 60.0)),
                    ..default()
                },
                ..default()
            },
            Player,
            CoopParticipant,
            slot,
            TeamMarker(Team::Player),
            GhostState::Alive,
            InGameEntity,
            Name::new(format!("Coop{}", slot.label())),
        ))
        .id();

    commands.entity(entity).insert((
        Health {
            current: cfg.max_hp,
            max: cfg.max_hp,
        },
        Energy {
            current: cfg.energy_max,
            max: cfg.energy_max,
        },
        Gold(0),
        Combo::new(1.8),
        Velocity::default(),
        MoveSpeed(cfg.move_speed),
        AttackPower(cfg.attack_power),
        FacingDirection(Vec2::X),
        AnimationState::Idle,
        CritChance(cfg.crit_chance),
        RewardModifiers::default(),
        PlayerAnim {
            state: AnimationState::Idle,
            timer: Timer::from_seconds(0.12, TimerMode::Once),
        },
        AttackCooldown::new(cfg.attack_cooldown_s),
        RangedCooldown::new(cfg.ranged_cooldown_s),
        RangedRapidFire {
            ramp: 0,
            decay: Timer::from_seconds(0.65, TimerMode::Once),
        },
    ));
    commands.entity(entity).insert((
        DashCooldown::new(cfg.dash_cooldown_s),
        InvincibilityTimer {
            timer: Timer::from_seconds(cfg.invincibility_s, TimerMode::Once),
        },
        DashState::inactive(cfg.dash_speed, cfg.dash_duration_s),
        crate::gameplay::combat::components::Hurtbox {
            team: Team::Player,
            size: Vec2::splat(30.0),
        },
        crate::gameplay::effects::flash::Flash::new(0.0),
        crate::gameplay::combat::components::Knockback(Vec2::ZERO),
        PlayerDriveInput::default(),
        BufferedCoopInput::default(),
        CoopNetPosition(translation.truncate()),
        CoopNetVelocity(Vec2::ZERO),
        CoopNetRotation(0.0),
        CoopMeleeFlashState::default(),
        CoopDashVisualState::default(),
    ));
    entity
}

fn host_tag_replicated_entities(
    mut commands: Commands,
    q_players: Query<
        (Entity, &PlayerSlot),
        (
            With<CoopParticipant>,
            Without<Replicating>,
            Without<Replicated>,
        ),
    >,
    q_enemies: Query<Entity, (With<Enemy>, Without<Replicating>, Without<Replicated>)>,
    q_projectiles: Query<Entity, (With<Projectile>, Without<Replicating>, Without<Replicated>)>,
    q_doors: Query<Entity, (With<Door>, Without<Replicating>, Without<Replicated>)>,
) {
    for (entity, slot) in &q_players {
        let client_id = match slot {
            PlayerSlot::P1 => host_client_id(),
            PlayerSlot::P2 => remote_client_id(),
        };
        safe_insert_bundle(&mut commands, entity, build_player_replication(client_id));
    }
    for entity in &q_enemies {
        safe_insert_bundle(&mut commands, entity, build_replicate_all());
    }
    for entity in &q_projectiles {
        safe_insert_bundle(&mut commands, entity, build_replicate_all());
    }
    for entity in &q_doors {
        safe_insert_bundle(&mut commands, entity, build_replicate_all());
    }
}

fn host_sync_network_views(
    mut views: ParamSet<(
        Query<
            (
                &Transform,
                &Velocity,
                &FacingDirection,
                &mut CoopNetPosition,
                &mut CoopNetVelocity,
                &mut CoopNetRotation,
            ),
            (With<CoopParticipant>, Without<Replicated>),
        >,
        Query<
            (&Transform, &mut CoopNetPosition),
            (With<Enemy>, Without<CoopParticipant>, Without<Replicated>),
        >,
        Query<
            (
                &Transform,
                &Projectile,
                &mut CoopNetPosition,
                &mut CoopNetVelocity,
                &mut CoopNetRotation,
            ),
            Without<Replicated>,
        >,
    )>,
) {
    for (transform, velocity, facing, mut net_pos, mut net_vel, mut net_rot) in &mut views.p0() {
        net_pos.0 = transform.translation.truncate();
        net_vel.0 = velocity.0;
        net_rot.0 = facing.0.y.atan2(facing.0.x);
    }
    for (transform, mut net_pos) in &mut views.p1() {
        net_pos.0 = transform.translation.truncate();
    }
    for (transform, projectile, mut net_pos, mut net_vel, mut net_rot) in &mut views.p2() {
        net_pos.0 = transform.translation.truncate();
        net_vel.0 = projectile.velocity;
        net_rot.0 = projectile.velocity.y.atan2(projectile.velocity.x);
    }
}

fn host_refresh_session_state(
    layout: Option<Res<FloorLayout>>,
    current_room: Option<Res<CurrentRoom>>,
    room_state: Option<Res<RoomState>>,
    floor: Option<Res<FloorNumber>>,
    mut session_q: Query<&mut CoopSessionState, With<CoopSessionEntity>>,
) {
    let (Some(layout), Some(current_room), Some(room_state)) = (layout, current_room, room_state)
    else {
        return;
    };
    let Ok(mut session) = session_q.get_single_mut() else {
        return;
    };
    session.room_type = layout
        .room(current_room.0)
        .map(|room| normalize_coop_room_type(room.room_type))
        .unwrap_or(RoomType::Normal);
    session.current_room = current_room.0.0;
    session.room_state = *room_state;
    session.floor_number = floor.as_deref().map(|value| value.0).unwrap_or(1);
    session.door_choice.options.clear();
    if let Some(room) = layout.room(current_room.0) {
        for (index, (dir, to)) in room.connections.exits.iter().enumerate() {
            session.door_choice.options.push(CoopDoorOption {
                index: index as u8,
                dir: *dir,
                room_type: layout
                    .room(*to)
                    .map(|destination| normalize_coop_room_type(destination.room_type))
                    .unwrap_or(RoomType::Normal),
            });
        }
    }
}

fn host_sync_dash_cooldowns(
    player_q: Query<(&PlayerSlot, &DashCooldown), (With<Player>, Without<Replicated>)>,
    mut session_q: Query<&mut CoopSessionState, With<CoopSessionEntity>>,
) {
    let Ok(mut session) = session_q.get_single_mut() else {
        return;
    };
    for (slot, cd) in &player_q {
        let frac = if cd.timer.finished() {
            0.0
        } else {
            1.0 - cd.timer.fraction()
        };
        match slot {
            PlayerSlot::P1 => session.p1_dash_cooldown_frac = frac,
            PlayerSlot::P2 => session.p2_dash_cooldown_frac = frac,
        }
    }
}

fn host_broadcast_damage_events(
    config: Res<CoopNetConfig>,
    net: Res<CoopNetState>,
    mut ev: EventReader<DamageAppliedEvent>,
    mut connection: ResMut<LyServerConnectionManager>,
) {
    if !matches!(config.mode, NetMode::Host | NetMode::Server) || net.connected_clients.is_empty() {
        ev.clear();
        return;
    }
    for e in ev.read() {
        let mut msg = CoopDamageEvent {
            amount: e.amount,
            is_crit: e.is_crit,
            pos: e.pos,
            attacker_is_player: matches!(e.attacker_team, Team::Player),
        };
        let _ = connection.send_message_to_target::<super::net::CoopCommandChannel, _>(
            &mut msg,
            lightyear::prelude::NetworkTarget::All,
        );
    }
}

fn host_cleanup_disconnected_session(
    config: Res<CoopNetConfig>,
    net: Res<CoopNetState>,
    runtime: Res<CoopRuntimeState>,
    session_q: Query<&CoopSessionState, With<CoopSessionEntity>>,
    mut flow: ResMut<CoopSessionFlow>,
) {
    let disconnected = match config.mode {
        NetMode::Host => {
            !net.remote_connected
                || input_timed_out(
                    &net,
                    remote_client_id(),
                    COOP_INPUT_DISCONNECT_TIMEOUT_FRAMES,
                )
        }
        NetMode::Server => {
            !(net.connected_clients.contains(&host_client_id())
                && net.connected_clients.contains(&remote_client_id()))
                || input_timed_out(&net, host_client_id(), COOP_INPUT_DISCONNECT_TIMEOUT_FRAMES)
                || input_timed_out(
                    &net,
                    remote_client_id(),
                    COOP_INPUT_DISCONNECT_TIMEOUT_FRAMES,
                )
        }
        NetMode::Client | NetMode::None => false,
    };
    if runtime.bootstrapped && disconnected {
        let ended_match = session_q
            .get_single()
            .map(|session| session.match_over)
            .unwrap_or(false);
        if config.mode == NetMode::Server {
            queue_exit_request(
                &mut flow,
                CoopExitRequest {
                    destination: CoopExitDestination::Lobby,
                    notice: Some("Session ended; server is waiting for new clients.".to_string()),
                    preserve_mode: true,
                },
            );
            return;
        }
        queue_exit_request(
            &mut flow,
            CoopExitRequest {
                destination: if ended_match {
                    CoopExitDestination::MainMenu
                } else {
                    CoopExitDestination::Lobby
                },
                notice: (!ended_match).then_some("队友已断开连接，合作会话已结束。".to_string()),
                preserve_mode: !ended_match,
            },
        );
    }
}

fn input_timed_out(net: &CoopNetState, client_id: ClientId, timeout_frames: u32) -> bool {
    let Some(last_frame) = net.latest_input_ticks.get(&client_id).copied() else {
        return net.host_frame_counter > timeout_frames;
    };
    net.host_frame_counter.wrapping_sub(last_frame) > timeout_frames
}

fn slot_client_id(slot: PlayerSlot) -> ClientId {
    client_id_for_slot(slot)
}

fn slot_index(slot: PlayerSlot) -> usize {
    slot.index()
}

#[allow(dead_code)]
fn spawn_dash_trail_hitbox(
    commands: &mut Commands,
    assets: &GameAssets,
    owner: Entity,
    pos: Vec2,
    damage: f32,
) {
    commands.spawn((
        SpriteBundle {
            texture: assets.textures.white.clone(),
            transform: Transform::from_translation(pos.extend(54.0)),
            sprite: Sprite {
                color: Color::srgba(0.40, 0.95, 1.0, 0.25),
                custom_size: Some(Vec2::splat(24.0)),
                ..default()
            },
            ..default()
        },
        crate::gameplay::combat::components::Hitbox {
            owner: Some(owner),
            team: Team::Player,
            damage_kind: crate::gameplay::combat::components::DamageKind::PlayerSkill,
            size: Vec2::splat(24.0),
            damage,
            knockback: 220.0,
            can_crit: false,
            crit_chance: 0.0,
            crit_multiplier: 1.0,
        },
        crate::gameplay::combat::components::Lifetime(Timer::from_seconds(0.08, TimerMode::Once)),
        InGameEntity,
        Name::new("CoopDashTrailHitbox"),
    ));
}

fn default_reward_state() -> super::components::RewardChoiceState {
    super::components::RewardChoiceState {
        lone_survivor: None,
        players: [
            PlayerRewardState {
                slot: PlayerSlot::P1,
                can_interact: false,
                mode: CoopRewardMode::None,
                primary_options: Vec::new(),
                secondary_options: Vec::new(),
                selected_primary: None,
                selected_secondary: None,
            },
            PlayerRewardState {
                slot: PlayerSlot::P2,
                can_interact: false,
                mode: CoopRewardMode::None,
                primary_options: Vec::new(),
                secondary_options: Vec::new(),
                selected_primary: None,
                selected_secondary: None,
            },
        ],
    }
}

#[derive(Debug, Clone, Copy)]
struct RewardPlayerSnapshot {
    slot: PlayerSlot,
    mods: RewardModifiers,
    ghost: GhostState,
}

fn default_shop_state() -> super::components::CoopShopState {
    super::components::CoopShopState {
        players: [
            PlayerShopState {
                slot: PlayerSlot::P1,
                can_interact: false,
                refresh_count: 0,
                offers: Vec::new(),
            },
            PlayerShopState {
                slot: PlayerSlot::P2,
                can_interact: false,
                refresh_count: 0,
                offers: Vec::new(),
            },
        ],
    }
}

fn default_door_choice_state() -> super::components::DoorChoiceState {
    super::components::DoorChoiceState {
        chooser: None,
        options: Vec::new(),
        p1_choice: None,
        p2_choice: None,
        input_timeout_s: COOP_DOOR_CONFIRM_TIMEOUT_S,
    }
}

fn default_rps_state() -> super::components::CoopRpsState {
    super::components::CoopRpsState {
        p1_choice: None,
        p2_choice: None,
        winner: None,
        winning_door: None,
        reveal_timer_s: 0.0,
        input_timeout_s: COOP_RPS_INPUT_TIMEOUT_S,
    }
}

fn reset_rps_input_round(rps: &mut super::components::CoopRpsState) {
    rps.p1_choice = None;
    rps.p2_choice = None;
    rps.input_timeout_s = COOP_RPS_INPUT_TIMEOUT_S;
}

fn fill_missing_rps_choices(rps: &mut super::components::CoopRpsState, rng: &mut GameRng) {
    let mut choices = [
        CoopRpsChoice::Rock,
        CoopRpsChoice::Paper,
        CoopRpsChoice::Scissors,
    ];

    if rps.p1_choice.is_none() {
        rng.shuffle(&mut choices);
        rps.p1_choice = Some(choices[0]);
    }
    if rps.p2_choice.is_none() {
        rng.shuffle(&mut choices);
        rps.p2_choice = Some(choices[0]);
    }
}

fn generate_reward_state(
    mode: CoopRewardMode,
    rng: &mut GameRng,
    players: &Query<
        (&PlayerSlot, &RewardModifiers, &GhostState),
        (With<CoopParticipant>, Without<Replicated>),
    >,
) -> super::components::RewardChoiceState {
    let snapshots = players
        .iter()
        .map(|(slot, mods, ghost)| RewardPlayerSnapshot {
            slot: *slot,
            mods: *mods,
            ghost: *ghost,
        })
        .collect::<Vec<_>>();
    generate_reward_state_from_snapshots(mode, rng, &snapshots)
}

fn generate_reward_state_from_snapshots(
    mode: CoopRewardMode,
    rng: &mut GameRng,
    snapshots: &[RewardPlayerSnapshot],
) -> super::components::RewardChoiceState {
    let mut state = default_reward_state();
    let living = snapshots
        .iter()
        .filter(|snapshot| snapshot.ghost == GhostState::Alive)
        .copied()
        .collect::<Vec<_>>();

    if living.len() == 1 {
        let snapshot = living[0];
        let buff = generate_reward_choices(rng, snapshot.mods, &[])
            .into_iter()
            .next()
            .unwrap_or(RewardType::IncreaseAttackPower);
        state.lone_survivor = Some(snapshot.slot);
        state.players[slot_index(snapshot.slot)] = PlayerRewardState {
            slot: snapshot.slot,
            can_interact: true,
            mode: CoopRewardMode::LoneSurvivor,
            primary_options: vec![
                CoopRewardOption::Rest,
                CoopRewardOption::Revive,
                CoopRewardOption::Buff(buff),
            ],
            secondary_options: Vec::new(),
            selected_primary: None,
            selected_secondary: None,
        };
        let other_slot = if snapshot.slot == PlayerSlot::P1 {
            PlayerSlot::P2
        } else {
            PlayerSlot::P1
        };
        state.players[slot_index(other_slot)].slot = other_slot;
        return state;
    }

    for snapshot in snapshots {
        let (primary_options, secondary_options) = if snapshot.ghost == GhostState::Alive {
            reward_options_for_mode(mode, rng, snapshot.mods)
        } else {
            (Vec::new(), Vec::new())
        };
        state.players[slot_index(snapshot.slot)] = PlayerRewardState {
            slot: snapshot.slot,
            can_interact: snapshot.ghost == GhostState::Alive,
            mode: if snapshot.ghost == GhostState::Alive {
                mode
            } else {
                CoopRewardMode::None
            },
            primary_options,
            secondary_options,
            selected_primary: None,
            selected_secondary: None,
        };
    }

    state
}

fn reward_phase_complete(session: &CoopSessionState) -> bool {
    let active = session
        .reward
        .players
        .iter()
        .filter(|player| player.can_interact)
        .collect::<Vec<_>>();
    !active.is_empty() && active.into_iter().all(reward_selection_complete)
}

fn sync_reward_phase_state(
    session: &mut CoopSessionState,
    rng: &mut GameRng,
    players: &Query<
        (&PlayerSlot, &RewardModifiers, &GhostState),
        (With<CoopParticipant>, Without<Replicated>),
    >,
) {
    if session.phase != CoopPhase::Reward {
        return;
    }

    let snapshots = players
        .iter()
        .map(|(slot, mods, ghost)| RewardPlayerSnapshot {
            slot: *slot,
            mods: *mods,
            ghost: *ghost,
        })
        .collect::<Vec<_>>();
    let living = snapshots
        .iter()
        .filter(|snapshot| snapshot.ghost == GhostState::Alive)
        .copied()
        .collect::<Vec<_>>();

    match living.as_slice() {
        [] => {
            for player in &mut session.reward.players {
                player.can_interact = false;
                player.mode = CoopRewardMode::None;
                player.selected_primary = None;
                player.selected_secondary = None;
                player.primary_options.clear();
                player.secondary_options.clear();
            }
            session.reward.lone_survivor = None;
        }
        [snapshot] => {
            let current_state = &session.reward.players[slot_index(snapshot.slot)];
            let selected = current_state.selected_primary;
            let buff = selected
                .and_then(|choice| match choice {
                    CoopRewardOption::Buff(buff) => Some(buff),
                    _ => None,
                })
                .or_else(|| {
                    current_state
                        .primary_options
                        .iter()
                        .find_map(|choice| match choice {
                            CoopRewardOption::Buff(buff) => Some(*buff),
                            _ => None,
                        })
                })
                .unwrap_or_else(|| {
                    generate_reward_choices(rng, snapshot.mods, &[])
                        .into_iter()
                        .next()
                        .unwrap_or(RewardType::IncreaseAttackPower)
                });

            let preserved_selection = match selected {
                Some(CoopRewardOption::Rest) => Some(CoopRewardOption::Rest),
                Some(CoopRewardOption::Revive) => Some(CoopRewardOption::Revive),
                Some(CoopRewardOption::Buff(_)) => Some(CoopRewardOption::Buff(buff)),
                None => None,
            };

            let other_slot = if snapshot.slot == PlayerSlot::P1 {
                PlayerSlot::P2
            } else {
                PlayerSlot::P1
            };

            session.reward = default_reward_state();
            session.reward.lone_survivor = Some(snapshot.slot);
            session.reward.players[slot_index(snapshot.slot)] = PlayerRewardState {
                slot: snapshot.slot,
                can_interact: true,
                mode: CoopRewardMode::LoneSurvivor,
                primary_options: vec![
                    CoopRewardOption::Rest,
                    CoopRewardOption::Revive,
                    CoopRewardOption::Buff(buff),
                ],
                secondary_options: Vec::new(),
                selected_primary: preserved_selection,
                selected_secondary: None,
            };
            session.reward.players[slot_index(other_slot)].slot = other_slot;
        }
        _ => {
            session.reward.lone_survivor = None;
            let shared_mode = session
                .reward
                .players
                .iter()
                .find_map(|player| {
                    (!matches!(
                        player.mode,
                        CoopRewardMode::None | CoopRewardMode::LoneSurvivor
                    ))
                    .then_some(player.mode)
                })
                .unwrap_or(CoopRewardMode::SingleBuff);

            for snapshot in snapshots {
                let player_state = &mut session.reward.players[slot_index(snapshot.slot)];
                player_state.slot = snapshot.slot;
                if snapshot.ghost == GhostState::Alive {
                    let must_regenerate = player_state.mode != shared_mode
                        || player_state.primary_options.is_empty()
                        || (shared_mode == CoopRewardMode::DualBuff
                            && player_state.secondary_options.is_empty());
                    if must_regenerate {
                        let (primary_options, secondary_options) =
                            reward_options_for_mode(shared_mode, rng, snapshot.mods);
                        player_state.primary_options = primary_options;
                        player_state.secondary_options = secondary_options;
                        player_state.selected_primary = None;
                        player_state.selected_secondary = None;
                    }
                    player_state.mode = shared_mode;
                    player_state.can_interact = true;
                } else {
                    player_state.can_interact = false;
                    player_state.mode = CoopRewardMode::None;
                    player_state.selected_primary = None;
                    player_state.selected_secondary = None;
                    player_state.primary_options.clear();
                    player_state.secondary_options.clear();
                }
            }
        }
    }
}

fn apply_reward_phase(
    session: &mut CoopSessionState,
    floor_number: u32,
    scaling: &RewardScalingConfig,
    players: &mut Query<
        (
            &PlayerSlot,
            &mut Gold,
            &mut Health,
            &mut Energy,
            &mut MoveSpeed,
            &mut AttackPower,
            &mut CritChance,
            &mut DashCooldown,
            &mut AttackCooldown,
            &mut RangedCooldown,
            &mut RewardModifiers,
            &mut GhostState,
            &mut Transform,
            &mut Sprite,
        ),
        (With<CoopParticipant>, Without<Replicated>),
    >,
) -> Option<(PlayerSlot, Vec3)> {
    let reward_state = session.reward.clone();
    let mut revive_target = None;
    let mut revive_anchor = None;

    for (
        slot,
        _gold,
        mut health,
        _energy,
        mut move_speed,
        mut attack_power,
        mut crit,
        mut dash_cd,
        mut attack_cd,
        mut ranged_cd,
        mut mods,
        ghost,
        transform,
        _sprite,
    ) in players.iter_mut()
    {
        if *ghost == GhostState::Alive && revive_anchor.is_none() {
            revive_anchor = Some(transform.translation);
        }

        let player_state = &reward_state.players[slot_index(*slot)];
        for selected in [
            player_state.selected_primary,
            player_state.selected_secondary,
        ] {
            let Some(selected) = selected else {
                continue;
            };
            if apply_reward_option_effect(
                selected,
                player_state.mode,
                floor_number,
                scaling,
                &mut health,
                &mut move_speed,
                &mut attack_power,
                &mut crit,
                &mut dash_cd,
                &mut attack_cd,
                &mut ranged_cd,
                &mut mods,
            ) {
                revive_target = Some(if *slot == PlayerSlot::P1 {
                    PlayerSlot::P2
                } else {
                    PlayerSlot::P1
                });
            }
        }
    }

    session.revive.dead_slot = revive_target;
    session.revive.revived = false;

    session.phase = CoopPhase::None;
    session.reward = default_reward_state();
    revive_target.map(|target| (target, revive_anchor.unwrap_or(coop_spawn_position(target))))
}

fn apply_reward_option_effect(
    selected: CoopRewardOption,
    mode: CoopRewardMode,
    floor_number: u32,
    scaling: &RewardScalingConfig,
    health: &mut Health,
    move_speed: &mut MoveSpeed,
    attack_power: &mut AttackPower,
    crit: &mut CritChance,
    dash_cd: &mut DashCooldown,
    attack_cd: &mut AttackCooldown,
    ranged_cd: &mut RangedCooldown,
    mods: &mut RewardModifiers,
) -> bool {
    match selected {
        CoopRewardOption::Buff(reward) => {
            apply_reward_to_player_components(
                reward,
                floor_number,
                reward_scale_for_mode(mode),
                scaling,
                mods,
                health,
                move_speed,
                dash_cd,
                ranged_cd,
                crit,
                attack_cd,
                attack_power,
            );
            false
        }
        CoopRewardOption::Rest => {
            let heal = heal_amount(scaling, health.max, floor_number);
            health.current = (health.current + heal).min(health.max);
            false
        }
        CoopRewardOption::Revive => true,
    }
}

fn reward_options_for_mode(
    mode: CoopRewardMode,
    rng: &mut GameRng,
    mods: RewardModifiers,
) -> (Vec<CoopRewardOption>, Vec<CoopRewardOption>) {
    match mode {
        CoopRewardMode::None => (Vec::new(), Vec::new()),
        CoopRewardMode::SingleBuff => (
            generate_reward_choices(rng, mods, &[])
                .into_iter()
                .map(CoopRewardOption::Buff)
                .collect(),
            Vec::new(),
        ),
        CoopRewardMode::HealOrBuff => {
            let mut primary = vec![CoopRewardOption::Rest];
            primary.extend(
                generate_reward_choices(rng, mods, &[])
                    .into_iter()
                    .map(CoopRewardOption::Buff),
            );
            (primary, Vec::new())
        }
        CoopRewardMode::DualBuff => {
            let (primary, secondary) = generate_dual_reward_choices(rng, mods);
            (
                primary.into_iter().map(CoopRewardOption::Buff).collect(),
                secondary.into_iter().map(CoopRewardOption::Buff).collect(),
            )
        }
        CoopRewardMode::LoneSurvivor => {
            let buff = generate_reward_choices(rng, mods, &[])
                .into_iter()
                .next()
                .unwrap_or(RewardType::IncreaseAttackPower);
            (
                vec![
                    CoopRewardOption::Rest,
                    CoopRewardOption::Revive,
                    CoopRewardOption::Buff(buff),
                ],
                Vec::new(),
            )
        }
    }
}

fn reward_selection_complete(player: &PlayerRewardState) -> bool {
    match player.mode {
        CoopRewardMode::None => false,
        CoopRewardMode::SingleBuff | CoopRewardMode::HealOrBuff | CoopRewardMode::LoneSurvivor => {
            player.selected_primary.is_some()
        }
        CoopRewardMode::DualBuff => {
            player.selected_primary.is_some() && player.selected_secondary.is_some()
        }
    }
}

fn reward_scale_for_mode(mode: CoopRewardMode) -> f32 {
    match mode {
        CoopRewardMode::DualBuff => 1.50,
        _ => 1.0,
    }
}

fn finish_revive_phase(
    session: &mut CoopSessionState,
    target: PlayerSlot,
    anchor: Vec3,
    revive_players: &mut Query<
        (
            &PlayerSlot,
            &mut GhostState,
            &mut Health,
            &mut Transform,
            &mut Sprite,
            &mut PlayerDriveInput,
            &mut Velocity,
            &mut FacingDirection,
            &mut AnimationState,
            &mut PlayerAnim,
            &mut DashState,
            &mut CoopMeleeFlashState,
            &mut CoopDashVisualState,
        ),
        (With<CoopParticipant>, Without<Replicated>),
    >,
) {
    let fallback = coop_spawn_position(target);
    for (
        slot,
        mut ghost,
        mut health,
        mut transform,
        mut sprite,
        mut drive,
        mut velocity,
        mut facing,
        mut animation_state,
        mut player_anim,
        mut dash_state,
        mut melee_flash,
        mut dash_visual,
    ) in revive_players.iter_mut()
    {
        if *slot != target {
            continue;
        }
        *ghost = GhostState::Alive;
        health.current = (health.max * REVIVE_HEALTH_FRACTION).max(1.0);
        drive.move_axis = Vec2::ZERO;
        drive.aim_world = None;
        drive.attack_pressed = false;
        drive.attack_held = false;
        drive.ranged_pressed = false;
        drive.ranged_held = false;
        drive.dash_pressed = false;
        drive.interact_pressed = false;
        drive.pause_pressed = false;
        drive.shop_pressed = false;
        drive.menu_confirm_pressed = false;
        drive.menu_cancel_pressed = false;
        velocity.0 = Vec2::ZERO;
        facing.0 = if *slot == PlayerSlot::P1 {
            Vec2::X
        } else {
            -Vec2::X
        };
        *animation_state = AnimationState::Idle;
        player_anim.state = AnimationState::Idle;
        player_anim.timer.reset();
        let dash_speed = dash_state.speed;
        let dash_duration_s = dash_state.timer.duration().as_secs_f32();
        *dash_state = DashState::inactive(dash_speed, dash_duration_s);
        *melee_flash = CoopMeleeFlashState::default();
        *dash_visual = CoopDashVisualState::default();
        transform.translation = anchor
            + Vec3::new(
                if *slot == PlayerSlot::P1 { -46.0 } else { 46.0 },
                -18.0,
                0.0,
            );
        transform.translation.z = fallback.z;
        sprite.color.set_alpha(1.0);
        session.revive.revived = true;
    }
}

fn generate_shop_state(
    floor_number: u32,
    rng: &mut GameRng,
    players: &Query<
        (&PlayerSlot, &RewardModifiers, &GhostState),
        (With<CoopParticipant>, Without<Replicated>),
    >,
) -> super::components::CoopShopState {
    let mut state = default_shop_state();
    for (slot, mods, ghost) in players.iter() {
        state.players[slot_index(*slot)] = PlayerShopState {
            slot: *slot,
            can_interact: *ghost == GhostState::Alive,
            refresh_count: 0,
            offers: build_shop_offers_for_player(floor_number, *mods, rng),
        };
    }
    state
}

fn try_purchase_shop_item(
    slot: PlayerSlot,
    index: usize,
    session: &mut CoopSessionState,
    scaling: &RewardScalingConfig,
    players: &mut Query<
        (
            &PlayerSlot,
            &mut Gold,
            &mut Health,
            &mut Energy,
            &mut MoveSpeed,
            &mut AttackPower,
            &mut CritChance,
            &mut DashCooldown,
            &mut AttackCooldown,
            &mut RangedCooldown,
            &mut RewardModifiers,
            &mut GhostState,
            &mut Transform,
            &mut Sprite,
        ),
        (With<CoopParticipant>, Without<Replicated>),
    >,
) {
    let state_index = slot_index(slot);
    let Some(offer) = session.shop.players[state_index].offers.get(index).cloned() else {
        return;
    };
    if offer.purchased || !session.shop.players[state_index].can_interact {
        return;
    }

    for (
        player_slot,
        mut gold,
        mut health,
        mut energy,
        mut move_speed,
        mut attack_power,
        mut crit,
        mut dash_cd,
        mut attack_cd,
        mut ranged_cd,
        mut mods,
        ghost,
        _transform,
        _sprite,
    ) in players.iter_mut()
    {
        if *player_slot != slot || *ghost != GhostState::Alive {
            continue;
        }
        if gold.0 < offer.cost {
            return;
        }
        if !apply_shop_item(
            offer.item,
            session.floor_number.max(1),
            scaling,
            &mut health,
            &mut energy,
            &mut move_speed,
            &mut attack_power,
            &mut crit,
            &mut dash_cd,
            &mut attack_cd,
            &mut ranged_cd,
            &mut mods,
        ) {
            return;
        }
        gold.0 -= offer.cost;
        if let Some(session_offer) = session.shop.players[state_index].offers.get_mut(index) {
            session_offer.purchased = true;
        }
        return;
    }
}

fn build_shop_offers_for_player(
    floor_number: u32,
    mods: RewardModifiers,
    rng: &mut GameRng,
) -> Vec<CoopShopOffer> {
    let mut pool = vec![
        CoopShopItem::Heal,
        CoopShopItem::IncreaseMaxHealth,
        CoopShopItem::IncreaseAttackPower,
        CoopShopItem::ReduceDashCooldown,
        CoopShopItem::IncreaseMoveSpeed,
        CoopShopItem::IncreaseEnergyMax,
        CoopShopItem::IncreaseCritChance,
        CoopShopItem::IncreaseAttackSpeed,
    ];
    rng.shuffle(&mut pool);
    pool.truncate(3);

    let base_cost = shop_base_cost(floor_number);
    pool.into_iter()
        .map(|item| {
            let (title, description) = describe_shop_item(item);
            CoopShopOffer {
                item,
                title: title.to_string(),
                description: description.to_string(),
                cost: shop_item_cost(item, floor_number, base_cost, mods),
                purchased: false,
            }
        })
        .collect()
}

fn apply_shop_item(
    item: CoopShopItem,
    floor_number: u32,
    scaling: &RewardScalingConfig,
    health: &mut Health,
    energy: &mut Energy,
    move_speed: &mut MoveSpeed,
    attack_power: &mut AttackPower,
    crit: &mut CritChance,
    dash_cd: &mut DashCooldown,
    attack_cd: &mut AttackCooldown,
    ranged_cd: &mut RangedCooldown,
    mods: &mut RewardModifiers,
) -> bool {
    match item {
        CoopShopItem::Heal => {
            health.current = (health.current + 35.0).min(health.max);
            true
        }
        CoopShopItem::IncreaseMaxHealth => {
            let gain = max_health_gain(scaling, floor_number);
            health.max += gain;
            health.current = (health.current + gain).min(health.max);
            mods.shop_max_health_purchases = mods.shop_max_health_purchases.saturating_add(1);
            true
        }
        CoopShopItem::IncreaseAttackPower => {
            attack_power.0 += attack_power_gain(scaling, floor_number);
            mods.shop_attack_power_purchases = mods.shop_attack_power_purchases.saturating_add(1);
            true
        }
        CoopShopItem::ReduceDashCooldown => {
            let remain = (0.20 - mods.shop_dash_cooldown_reduction_s).max(0.0);
            if remain <= 0.0 {
                return false;
            }
            mods.shop_dash_cooldown_reduction_s +=
                dash_cooldown_gain_s(scaling, floor_number).min(remain);
            dash_cd.apply_reduction(mods.total_dash_cooldown_reduction());
            mods.shop_dash_purchases = mods.shop_dash_purchases.saturating_add(1);
            true
        }
        CoopShopItem::IncreaseMoveSpeed => {
            let gain = move_speed_gain(scaling, floor_number) * 0.75;
            move_speed.0 += gain;
            mods.shop_move_speed_purchases = mods.shop_move_speed_purchases.saturating_add(1);
            true
        }
        CoopShopItem::IncreaseEnergyMax => {
            energy.max += 25.0;
            energy.current = (energy.current + 25.0).min(energy.max);
            true
        }
        CoopShopItem::IncreaseCritChance => {
            let gain = crit_gain(scaling, floor_number) * 0.75;
            let next = (crit.0 + gain).clamp(0.0, 1.0);
            if (next - crit.0).abs() < f32::EPSILON {
                return false;
            }
            crit.0 = next;
            mods.shop_crit_purchases = mods.shop_crit_purchases.saturating_add(1);
            true
        }
        CoopShopItem::IncreaseAttackSpeed => {
            let remain = (0.18 - mods.shop_attack_speed_reduction_s).max(0.0);
            if remain <= 0.0 {
                return false;
            }
            mods.shop_attack_speed_reduction_s +=
                attack_speed_gain_s(scaling, floor_number).min(remain);
            attack_cd.apply_speed_bonus(mods.total_melee_speed_bonus());
            ranged_cd.apply_speed_bonus(mods.total_ranged_speed_bonus());
            mods.shop_attack_speed_purchases = mods.shop_attack_speed_purchases.saturating_add(1);
            true
        }
    }
}

fn try_refresh_shop(
    slot: PlayerSlot,
    floor_number: u32,
    session: &mut CoopSessionState,
    rng: &mut GameRng,
    shop: &crate::data::definitions::ShopConfig,
    players: &mut Query<
        (
            &PlayerSlot,
            &mut Gold,
            &mut Health,
            &mut Energy,
            &mut MoveSpeed,
            &mut AttackPower,
            &mut CritChance,
            &mut DashCooldown,
            &mut AttackCooldown,
            &mut RangedCooldown,
            &mut RewardModifiers,
            &mut GhostState,
            &mut Transform,
            &mut Sprite,
        ),
        (With<CoopParticipant>, Without<Replicated>),
    >,
) {
    let state_index = slot_index(slot);
    let can_interact = session.shop.players[state_index].can_interact;
    let refresh_count = session.shop.players[state_index].refresh_count;
    if !can_interact {
        return;
    }

    for (
        player_slot,
        mut gold,
        _health,
        _energy,
        _move_speed,
        _attack_power,
        _crit,
        _dash_cd,
        _attack_cd,
        _ranged_cd,
        mods,
        ghost,
        _transform,
        _sprite,
    ) in players.iter_mut()
    {
        if *player_slot != slot || *ghost != GhostState::Alive {
            continue;
        }

        let refresh_cost = next_refresh_cost(refresh_count, shop);
        if gold.0 < refresh_cost {
            return;
        }

        gold.0 -= refresh_cost;
        let next_refresh_count = refresh_count.saturating_add(1);
        session.shop.players[state_index].refresh_count = next_refresh_count;
        session.shop.players[state_index].offers =
            build_shop_offers_for_player(floor_number, *mods, rng);
        return;
    }
}

fn advance_to_room(
    door_index: u8,
    runtime: &mut CoopRuntimeState,
    session: &mut CoopSessionState,
    layout: Option<&FloorLayout>,
    current_room: Option<&mut CurrentRoom>,
    room_state: Option<&mut RoomState>,
    visited: Option<&mut VisitedRooms>,
    player_transforms: &mut Query<
        (&PlayerSlot, &mut Transform),
        (With<CoopParticipant>, Without<Replicated>),
    >,
) {
    let (Some(layout), Some(current_room), Some(room_state)) = (layout, current_room, room_state)
    else {
        return;
    };
    let Some(room) = layout.room(current_room.0) else {
        return;
    };
    let Some((dir, next_room)) = room.connections.exits.get(door_index as usize).copied() else {
        return;
    };

    current_room.0 = next_room;
    if let Some(visited) = visited {
        visited.0.insert(next_room);
    }
    *room_state = RoomState::Idle;

    let entry_from = opposite_direction(dir);
    for (slot, mut transform) in player_transforms.iter_mut() {
        transform.translation = player_spawn_position(
            entry_from,
            transform.translation.z,
            if *slot == PlayerSlot::P1 { 22.0 } else { -22.0 },
        );
    }

    session.phase = CoopPhase::None;
    session.reward = default_reward_state();
    session.shop = default_shop_state();
    session.door_choice = default_door_choice_state();
    session.rps = default_rps_state();
    session.revive.dead_slot = None;
    session.revive.revived = false;
    session.current_room = next_room.0;
    session.room_type = layout
        .room(next_room)
        .map(|next| next.room_type)
        .unwrap_or(RoomType::Normal);
    session.room_state = *room_state;
    runtime.last_room_seen = None;
}

fn generate_reward_choices(
    rng: &mut GameRng,
    mods: RewardModifiers,
    excluded: &[RewardType],
) -> Vec<RewardType> {
    let mut pool = reward_pool()
        .into_iter()
        .filter(|reward| !mods.reward_at_max(*reward) && !excluded.contains(reward))
        .collect::<Vec<_>>();
    if pool.len() < 3 {
        pool = reward_pool()
            .into_iter()
            .filter(|reward| !excluded.contains(reward))
            .collect::<Vec<_>>();
    }
    rng.shuffle(&mut pool);
    pool.truncate(3);
    pool
}

fn generate_dual_reward_choices(
    rng: &mut GameRng,
    mods: RewardModifiers,
) -> (Vec<RewardType>, Vec<RewardType>) {
    let primary = generate_reward_choices(rng, mods, &[]);
    let mut secondary = generate_reward_choices(rng, mods, &primary);
    if secondary.len() < 3 {
        secondary = generate_reward_choices(rng, mods, &[]);
    }
    (primary, secondary)
}

fn reward_pool() -> Vec<RewardType> {
    vec![
        RewardType::EnhanceMeleeWeapon,
        RewardType::IncreaseAttackSpeed,
        RewardType::IncreaseAttackPower,
        RewardType::IncreaseMaxHealth,
        RewardType::ReduceDashCooldown,
        RewardType::LifeStealOnKill,
        RewardType::IncreaseCritChance,
        RewardType::IncreaseMoveSpeed,
        RewardType::DashDamageTrail,
        RewardType::EnhanceRangedWeapon,
    ]
}

fn describe_shop_item(item: CoopShopItem) -> (&'static str, &'static str) {
    match item {
        CoopShopItem::Heal => ("Restock", "Restore 35 HP immediately"),
        CoopShopItem::IncreaseMaxHealth => ("Fortify", "Increase max health"),
        CoopShopItem::IncreaseAttackPower => ("Sharpen", "Increase attack power"),
        CoopShopItem::ReduceDashCooldown => ("Quickstep", "Reduce dash cooldown"),
        CoopShopItem::IncreaseMoveSpeed => ("Lightfoot", "Increase move speed"),
        CoopShopItem::IncreaseEnergyMax => ("Charge", "Increase max energy"),
        CoopShopItem::IncreaseCritChance => ("Keen Eye", "Increase crit chance"),
        CoopShopItem::IncreaseAttackSpeed => ("Flurry", "Increase attack speed"),
    }
}

fn shop_base_cost(floor_number: u32) -> u32 {
    match floor_number {
        1 => 55,
        2 => 65,
        3 => 75,
        _ => 85,
    }
}

fn shop_item_extra_cost(item: CoopShopItem) -> u32 {
    match item {
        CoopShopItem::Heal => 0,
        CoopShopItem::IncreaseMaxHealth => 15,
        CoopShopItem::IncreaseAttackPower => 18,
        CoopShopItem::ReduceDashCooldown => 18,
        CoopShopItem::IncreaseMoveSpeed => 15,
        CoopShopItem::IncreaseEnergyMax => 12,
        CoopShopItem::IncreaseCritChance => 20,
        CoopShopItem::IncreaseAttackSpeed => 20,
    }
}

fn shop_purchase_count(mods: RewardModifiers, item: CoopShopItem) -> u8 {
    match item {
        CoopShopItem::Heal | CoopShopItem::IncreaseEnergyMax => 0,
        CoopShopItem::IncreaseMaxHealth => mods.shop_max_health_purchases,
        CoopShopItem::IncreaseAttackPower => mods.shop_attack_power_purchases,
        CoopShopItem::ReduceDashCooldown => mods.shop_dash_purchases,
        CoopShopItem::IncreaseMoveSpeed => mods.shop_move_speed_purchases,
        CoopShopItem::IncreaseCritChance => mods.shop_crit_purchases,
        CoopShopItem::IncreaseAttackSpeed => mods.shop_attack_speed_purchases,
    }
}

fn shop_repeat_price_mult(purchases: u8) -> f32 {
    match purchases {
        0 => 1.00,
        1 => 1.35,
        2 => 1.75,
        _ => 2.15,
    }
}

fn shop_item_cost(
    item: CoopShopItem,
    _floor_number: u32,
    base_cost: u32,
    mods: RewardModifiers,
) -> u32 {
    let base = base_cost + shop_item_extra_cost(item);
    let purchases = shop_purchase_count(mods, item);
    ((base as f32) * shop_repeat_price_mult(purchases)).round() as u32
}

fn resolve_rps(p1: CoopRpsChoice, p2: CoopRpsChoice) -> Option<PlayerSlot> {
    match (p1, p2) {
        (CoopRpsChoice::Rock, CoopRpsChoice::Scissors)
        | (CoopRpsChoice::Paper, CoopRpsChoice::Rock)
        | (CoopRpsChoice::Scissors, CoopRpsChoice::Paper) => Some(PlayerSlot::P1),
        (CoopRpsChoice::Scissors, CoopRpsChoice::Rock)
        | (CoopRpsChoice::Rock, CoopRpsChoice::Paper)
        | (CoopRpsChoice::Paper, CoopRpsChoice::Scissors) => Some(PlayerSlot::P2),
        _ => None,
    }
}

fn door_world_position(dir: Direction) -> Vec2 {
    match dir {
        Direction::Right => Vec2::new(ROOM_HALF_WIDTH - 10.0, 0.0),
        Direction::Left => Vec2::new(-(ROOM_HALF_WIDTH - 10.0), 0.0),
        Direction::Up => Vec2::new(0.0, ROOM_HALF_HEIGHT - 10.0),
        Direction::Down => Vec2::new(0.0, -(ROOM_HALF_HEIGHT - 10.0)),
    }
}

fn opposite_direction(dir: Direction) -> Direction {
    match dir {
        Direction::Up => Direction::Down,
        Direction::Down => Direction::Up,
        Direction::Left => Direction::Right,
        Direction::Right => Direction::Left,
    }
}

fn player_spawn_position(entry_from: Direction, z: f32, y_offset: f32) -> Vec3 {
    match entry_from {
        Direction::Left => Vec3::new(-ROOM_HALF_WIDTH * 0.6, y_offset, z),
        Direction::Right => Vec3::new(ROOM_HALF_WIDTH * 0.6, y_offset, z),
        Direction::Up => Vec3::new(0.0, ROOM_HALF_HEIGHT * 0.55 + y_offset, z),
        Direction::Down => Vec3::new(0.0, -ROOM_HALF_HEIGHT * 0.55 + y_offset, z),
    }
}

fn coop_spawn_position(slot: PlayerSlot) -> Vec3 {
    match slot {
        PlayerSlot::P1 => Vec3::new(-220.0, 0.0, 50.0),
        PlayerSlot::P2 => Vec3::new(-160.0, -42.0, 50.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::map::room::{RoomBounds, RoomConnections, RoomData};

    fn seeded_rng(seed: u64) -> GameRng {
        let mut rng = GameRng::default();
        rng.reseed(seed);
        rng
    }

    fn snapshot(slot: PlayerSlot, ghost: GhostState) -> RewardPlayerSnapshot {
        RewardPlayerSnapshot {
            slot,
            mods: RewardModifiers::default(),
            ghost,
        }
    }

    fn sample_player_stats() -> (
        Health,
        MoveSpeed,
        AttackPower,
        CritChance,
        DashCooldown,
        AttackCooldown,
        RangedCooldown,
        RewardModifiers,
    ) {
        (
            Health {
                current: 50.0,
                max: 100.0,
            },
            MoveSpeed(280.0),
            AttackPower(20.0),
            CritChance(0.10),
            DashCooldown::new(0.6),
            AttackCooldown::new(0.5),
            RangedCooldown::new(0.8),
            RewardModifiers::default(),
        )
    }

    #[test]
    fn reward_room_generates_single_buff_for_each_living_player() {
        let mut rng = seeded_rng(1);
        let state = generate_reward_state_from_snapshots(
            CoopRewardMode::SingleBuff,
            &mut rng,
            &[
                snapshot(PlayerSlot::P1, GhostState::Alive),
                snapshot(PlayerSlot::P2, GhostState::Alive),
            ],
        );

        assert_eq!(state.lone_survivor, None);
        for player in state.players {
            assert!(player.can_interact);
            assert_eq!(player.mode, CoopRewardMode::SingleBuff);
            assert_eq!(player.primary_options.len(), 3);
            assert!(player.secondary_options.is_empty());
            assert!(
                player
                    .primary_options
                    .iter()
                    .all(|option| matches!(option, CoopRewardOption::Buff(_)))
            );
        }
    }

    #[test]
    fn normal_clear_generates_heal_plus_three_buffs() {
        let mut rng = seeded_rng(2);
        let state = generate_reward_state_from_snapshots(
            CoopRewardMode::HealOrBuff,
            &mut rng,
            &[
                snapshot(PlayerSlot::P1, GhostState::Alive),
                snapshot(PlayerSlot::P2, GhostState::Alive),
            ],
        );

        for player in state.players {
            assert!(player.can_interact);
            assert_eq!(player.mode, CoopRewardMode::HealOrBuff);
            assert_eq!(player.primary_options.len(), 4);
            assert_eq!(player.primary_options[0], CoopRewardOption::Rest);
            assert!(
                player.primary_options[1..]
                    .iter()
                    .all(|option| matches!(option, CoopRewardOption::Buff(_)))
            );
            assert!(player.secondary_options.is_empty());
        }
    }

    #[test]
    fn boss_clear_generates_dual_reward_columns() {
        let mut rng = seeded_rng(3);
        let state = generate_reward_state_from_snapshots(
            CoopRewardMode::DualBuff,
            &mut rng,
            &[
                snapshot(PlayerSlot::P1, GhostState::Alive),
                snapshot(PlayerSlot::P2, GhostState::Alive),
            ],
        );

        for player in state.players {
            assert!(player.can_interact);
            assert_eq!(player.mode, CoopRewardMode::DualBuff);
            assert_eq!(player.primary_options.len(), 3);
            assert_eq!(player.secondary_options.len(), 3);
            assert!(
                player
                    .primary_options
                    .iter()
                    .all(|option| matches!(option, CoopRewardOption::Buff(_)))
            );
            assert!(
                player
                    .secondary_options
                    .iter()
                    .all(|option| matches!(option, CoopRewardOption::Buff(_)))
            );

            let primary_buffs = player
                .primary_options
                .iter()
                .filter_map(|option| match option {
                    CoopRewardOption::Buff(reward) => Some(*reward),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let secondary_buffs = player
                .secondary_options
                .iter()
                .filter_map(|option| match option {
                    CoopRewardOption::Buff(reward) => Some(*reward),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert!(
                primary_buffs
                    .iter()
                    .all(|reward| !secondary_buffs.contains(reward))
            );
        }
    }

    #[test]
    fn lone_survivor_keeps_rest_revive_buff_exception() {
        let mut rng = seeded_rng(4);
        let state = generate_reward_state_from_snapshots(
            CoopRewardMode::HealOrBuff,
            &mut rng,
            &[
                snapshot(PlayerSlot::P1, GhostState::Alive),
                snapshot(PlayerSlot::P2, GhostState::Ghost),
            ],
        );

        assert_eq!(state.lone_survivor, Some(PlayerSlot::P1));
        let survivor = &state.players[PlayerSlot::P1.index()];
        let ghost = &state.players[PlayerSlot::P2.index()];
        assert!(survivor.can_interact);
        assert_eq!(survivor.mode, CoopRewardMode::LoneSurvivor);
        assert_eq!(survivor.primary_options.len(), 3);
        assert_eq!(survivor.primary_options[0], CoopRewardOption::Rest);
        assert_eq!(survivor.primary_options[1], CoopRewardOption::Revive);
        assert!(matches!(
            survivor.primary_options[2],
            CoopRewardOption::Buff(_)
        ));
        assert!(!ghost.can_interact);
        assert_eq!(ghost.mode, CoopRewardMode::None);
    }

    #[test]
    fn reward_application_heal_and_revive_behave_as_expected() {
        let (
            mut health,
            mut move_speed,
            mut attack_power,
            mut crit,
            mut dash_cd,
            mut attack_cd,
            mut ranged_cd,
            mut mods,
        ) = sample_player_stats();
        let scaling = RewardScalingConfig::default_config();

        let revive = apply_reward_option_effect(
            CoopRewardOption::Rest,
            CoopRewardMode::HealOrBuff,
            1,
            &scaling,
            &mut health,
            &mut move_speed,
            &mut attack_power,
            &mut crit,
            &mut dash_cd,
            &mut attack_cd,
            &mut ranged_cd,
            &mut mods,
        );
        assert!(!revive);
        assert!(health.current > 50.0);

        let revive = apply_reward_option_effect(
            CoopRewardOption::Revive,
            CoopRewardMode::LoneSurvivor,
            1,
            &scaling,
            &mut health,
            &mut move_speed,
            &mut attack_power,
            &mut crit,
            &mut dash_cd,
            &mut attack_cd,
            &mut ranged_cd,
            &mut mods,
        );
        assert!(revive);
    }

    #[test]
    fn boss_reward_application_uses_boss_scale() {
        let (
            mut single_health,
            mut single_move_speed,
            mut single_attack_power,
            mut single_crit,
            mut single_dash_cd,
            mut single_attack_cd,
            mut single_ranged_cd,
            mut single_mods,
        ) = sample_player_stats();
        let (
            mut boss_health,
            mut boss_move_speed,
            mut boss_attack_power,
            mut boss_crit,
            mut boss_dash_cd,
            mut boss_attack_cd,
            mut boss_ranged_cd,
            mut boss_mods,
        ) = sample_player_stats();
        let scaling = RewardScalingConfig::default_config();

        let reward = CoopRewardOption::Buff(RewardType::IncreaseAttackPower);
        assert!(!apply_reward_option_effect(
            reward,
            CoopRewardMode::SingleBuff,
            1,
            &scaling,
            &mut single_health,
            &mut single_move_speed,
            &mut single_attack_power,
            &mut single_crit,
            &mut single_dash_cd,
            &mut single_attack_cd,
            &mut single_ranged_cd,
            &mut single_mods,
        ));
        assert!(!apply_reward_option_effect(
            reward,
            CoopRewardMode::DualBuff,
            1,
            &scaling,
            &mut boss_health,
            &mut boss_move_speed,
            &mut boss_attack_power,
            &mut boss_crit,
            &mut boss_dash_cd,
            &mut boss_attack_cd,
            &mut boss_ranged_cd,
            &mut boss_mods,
        ));

        assert!(boss_attack_power.0 > single_attack_power.0);
    }

    #[test]
    fn shop_repeat_price_scaling_matches_single_player_curve() {
        let floor_number = 2;
        let base_cost = shop_base_cost(floor_number);
        let base = shop_item_cost(
            CoopShopItem::IncreaseAttackPower,
            floor_number,
            base_cost,
            RewardModifiers::default(),
        );

        let mut mods = RewardModifiers {
            shop_attack_power_purchases: 1,
            ..Default::default()
        };
        let once_bought = shop_item_cost(
            CoopShopItem::IncreaseAttackPower,
            floor_number,
            base_cost,
            mods,
        );

        mods.shop_attack_power_purchases = 2;
        let twice_bought = shop_item_cost(
            CoopShopItem::IncreaseAttackPower,
            floor_number,
            base_cost,
            mods,
        );

        assert_eq!(base, 83);
        assert_eq!(once_bought, 112);
        assert_eq!(twice_bought, 145);
        let shop = crate::data::definitions::ShopConfig::default();
        assert_eq!(next_refresh_cost(0, &shop), 0);
        assert_eq!(next_refresh_cost(1, &shop), 30);
        assert_eq!(next_refresh_cost(2, &shop), 45);
        assert_eq!(next_refresh_cost(3, &shop), 60);
    }

    #[test]
    fn energy_shop_item_applies_same_effect_math() {
        let (
            mut health,
            mut move_speed,
            mut attack_power,
            mut crit,
            mut dash_cd,
            mut attack_cd,
            mut ranged_cd,
            mut mods,
        ) = sample_player_stats();
        let mut energy = Energy {
            current: 30.0,
            max: 75.0,
        };
        let scaling = RewardScalingConfig::default_config();

        assert!(apply_shop_item(
            CoopShopItem::IncreaseEnergyMax,
            1,
            &scaling,
            &mut health,
            &mut energy,
            &mut move_speed,
            &mut attack_power,
            &mut crit,
            &mut dash_cd,
            &mut attack_cd,
            &mut ranged_cd,
            &mut mods,
        ));
        assert_eq!(energy.max, 100.0);
        assert_eq!(energy.current, 55.0);
    }

    #[test]
    fn shop_offer_generation_is_player_specific_and_unique() {
        let mut rng = seeded_rng(8);
        let offers = build_shop_offers_for_player(1, RewardModifiers::default(), &mut rng);

        assert_eq!(offers.len(), 3);
        for offer in &offers {
            assert!(!offer.title.is_empty());
            assert!(!offer.description.is_empty());
            assert!(offer.cost > 0);
        }

        for (index, offer) in offers.iter().enumerate() {
            assert!(
                offers
                    .iter()
                    .skip(index + 1)
                    .all(|other| other.item != offer.item)
            );
        }
    }

    #[test]
    fn coop_layout_normalization_rewrites_puzzle_rooms_to_normal() {
        let mut layout = FloorLayout {
            rooms: vec![
                RoomData {
                    id: RoomId(0),
                    room_type: RoomType::Event,
                    mystery: true,
                    connections: RoomConnections { exits: Vec::new() },
                    bounds: RoomBounds {
                        half_size: Vec2::new(320.0, 180.0),
                    },
                },
                RoomData {
                    id: RoomId(1),
                    room_type: RoomType::Reward,
                    mystery: false,
                    connections: RoomConnections { exits: Vec::new() },
                    bounds: RoomBounds {
                        half_size: Vec2::new(320.0, 180.0),
                    },
                },
            ],
            current: RoomId(0),
        };

        normalize_coop_layout(&mut layout);

        assert_eq!(layout.rooms[0].room_type, RoomType::Normal);
        assert_eq!(layout.rooms[1].room_type, RoomType::Reward);
    }

    #[test]
    fn default_rps_state_starts_with_timeout_budget() {
        let state = default_rps_state();

        assert_eq!(state.input_timeout_s, COOP_RPS_INPUT_TIMEOUT_S);
        assert!(state.p1_choice.is_none());
        assert!(state.p2_choice.is_none());
        assert!(state.winner.is_none());
    }

    #[test]
    fn reset_rps_input_round_clears_choices_and_restores_timeout() {
        let mut state = default_rps_state();
        state.p1_choice = Some(CoopRpsChoice::Rock);
        state.p2_choice = Some(CoopRpsChoice::Scissors);
        state.input_timeout_s = 0.0;

        reset_rps_input_round(&mut state);

        assert!(state.p1_choice.is_none());
        assert!(state.p2_choice.is_none());
        assert_eq!(state.input_timeout_s, COOP_RPS_INPUT_TIMEOUT_S);
    }

    #[test]
    fn fill_missing_rps_choices_assigns_missing_inputs() {
        let mut rng = seeded_rng(9);
        let mut state = default_rps_state();
        state.input_timeout_s = 0.0;

        fill_missing_rps_choices(&mut state, &mut rng);

        assert!(state.p1_choice.is_some());
        assert!(state.p2_choice.is_some());
    }
}
