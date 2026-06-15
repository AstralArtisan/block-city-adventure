use bevy::prelude::*;
use lightyear::prelude::Replicated;
use std::time::Duration;

use crate::constants::{ROOM_HALF_HEIGHT, ROOM_HALF_WIDTH};
use crate::coop::components::{CoopNetPosition, CoopNetRotation, CoopNetVelocity};
use crate::coop::components::{CoopParticipant, GhostState};
use crate::coop::net::{CoopNetConfig, NetMode, is_coop_authority};
use crate::coop::runtime::is_coop_simulation_active;
use crate::core::events::{DamageAppliedEvent, DamageEvent, DeathEvent, RoomClearedEvent};
use crate::data::definitions::EnemyStatsConfig;
use crate::data::registry::GameDataRegistry;
use crate::gameplay::augment::data::{AugmentId, AugmentInventory};
use crate::gameplay::augment::effects::DashResetSpeedBuff;
use crate::gameplay::augment::tuning;
use crate::gameplay::combat::components::{
    DamageKind, Hitbox, Hurtbox, Knockback, Lifetime, Projectile, Team,
};
use crate::gameplay::combat::projectiles;
use crate::gameplay::effects::flash::Flash;
use crate::gameplay::effects::particles;
use crate::gameplay::enemy::{ai, boss, spawner};
use crate::gameplay::event_room::{ActiveEvent, EventType};
use crate::gameplay::map::InGameEntity;
use crate::gameplay::map::room::{CurrentRoom, FloorLayout, RoomType};
use crate::gameplay::player::components::{
    DashCooldown, Gold, Health, InvincibilityTimer, RewardModifiers, TeamMarker,
};
use crate::gameplay::player::components::{Health as PlayerHealth, Player};
use crate::gameplay::progression::difficulty::{
    get_floor_difficulty_multiplier, get_floor_enemy_count,
};
use crate::gameplay::progression::experience::XpGainEvent;
use crate::gameplay::progression::floor::FloorNumber;
use crate::gameplay::puzzle::{ActivePuzzle, PuzzleEntity, reset_active_puzzle};
use crate::gameplay::shop::ShopKiosk;
use crate::gameplay::skills::ChargeGainEvent;
use crate::states::{AppState, GamePhase, RoomState};
use crate::utils::collision::aabb_from_transform_size;
use crate::utils::entity::safe_despawn_recursive;
use crate::utils::math::{clamp_in_room, direction_to};
use crate::utils::rng::GameRng;

use super::components::*;

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct EnemyVelocity(pub Vec2);

#[derive(Component, Debug, Clone)]
struct LobberImpact {
    owner: Entity,
    timer: Timer,
    damage: f32,
    radius: f32,
}

#[derive(Component)]
pub struct EnemyHealthBar {
    pub owner: Entity,
    pub bar_width: f32,
}

#[derive(Component)]
pub struct EnemyHealthBarFill {
    pub owner: Entity,
}

pub struct EnemySystemsPlugin;

impl Plugin for EnemySystemsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameRng>()
            .insert_resource(EnemySpawnCount { current: 0 })
            .insert_resource(SpawnedForRoom::default())
            .insert_resource(ClearGrace::default())
            .add_systems(
                Update,
                room_entry_spawner.run_if(
                    in_state(AppState::InGame)
                        .or_else(
                            in_state(AppState::CoopGame)
                                .and_then(is_coop_authority)
                                .and_then(is_coop_simulation_active),
                        )
                        .and_then(in_state(GamePhase::Playing)),
                ),
            )
            .add_systems(
                Update,
                (spawn_enemy_health_bars, update_enemy_health_bars).run_if(
                    in_state(AppState::InGame)
                        .or_else(
                            in_state(AppState::CoopGame)
                                .and_then(is_coop_authority)
                                .and_then(is_coop_simulation_active),
                        )
                        .and_then(in_state(GamePhase::Playing)),
                ),
            )
            .add_systems(
                Update,
                enemy_buff_decay_system.run_if(
                    in_state(AppState::InGame)
                        .or_else(
                            in_state(AppState::CoopGame)
                                .and_then(is_coop_authority)
                                .and_then(is_coop_simulation_active),
                        )
                        .and_then(in_state(GamePhase::Playing)),
                ),
            )
            .add_systems(
                Update,
                ai::update_enemy_ai.run_if(
                    in_state(AppState::InGame)
                        .or_else(
                            in_state(AppState::CoopGame)
                                .and_then(is_coop_authority)
                                .and_then(is_coop_simulation_active),
                        )
                        .and_then(in_state(GamePhase::Playing)),
                ),
            )
            .add_systems(
                Update,
                (
                    bomber_fuse_system.after(ai::update_enemy_ai),
                    shielder_facing_system.after(ai::update_enemy_ai),
                    shielder_block_system
                        .before(crate::gameplay::combat::hitbox::detect_hitbox_hurtbox_overlap),
                    summoner_summon_system.after(ai::update_enemy_ai),
                    elite_berserk_system
                        .after(crate::gameplay::combat::damage::apply_damage_events)
                        .before(enemy_attack_system),
                    elite_teleport_system
                        .after(ai::update_enemy_ai)
                        .before(enemy_attack_system),
                    elite_vampiric_system
                        .after(crate::gameplay::combat::damage::apply_damage_events),
                    elite_splitting_system
                        .after(crate::gameplay::combat::damage::apply_damage_events)
                        .before(enemy_death_system),
                    summoner_death_cleanup.before(enemy_death_system),
                )
                    .run_if(
                        in_state(AppState::InGame)
                            .or_else(
                                in_state(AppState::CoopGame)
                                    .and_then(is_coop_authority)
                                    .and_then(is_coop_simulation_active),
                            )
                            .and_then(in_state(GamePhase::Playing)),
                    ),
            )
            .add_systems(
                Update,
                (
                    enemy_attack_system,
                    resolve_lobber_impacts.after(enemy_attack_system),
                )
                    .run_if(
                        in_state(AppState::InGame)
                            .or_else(
                                in_state(AppState::CoopGame)
                                    .and_then(is_coop_authority)
                                    .and_then(is_coop_simulation_active),
                            )
                            .and_then(in_state(GamePhase::Playing)),
                    ),
            )
            .add_systems(
                Update,
                boss::boss_phase_controller.run_if(
                    in_state(AppState::InGame)
                        .or_else(
                            in_state(AppState::CoopGame)
                                .and_then(is_coop_authority)
                                .and_then(is_coop_simulation_active),
                        )
                        .and_then(in_state(GamePhase::Playing)),
                ),
            )
            .add_systems(
                Update,
                boss::boss_attack_patterns.run_if(
                    in_state(AppState::InGame)
                        .or_else(
                            in_state(AppState::CoopGame)
                                .and_then(is_coop_authority)
                                .and_then(is_coop_simulation_active),
                        )
                        .and_then(in_state(GamePhase::Playing)),
                ),
            )
            .add_systems(
                Update,
                (
                    boss::boss_guardian_facing_system,
                    boss::boss_decoy_system,
                    boss::tide_hunter_state_machine,
                    boss::tide_hunter_contact_damage_system.after(boss::tide_hunter_state_machine),
                    boss::tide_hunter_parry_check.after(boss::tide_hunter_state_machine),
                    boss::shadow_trail_fade_system.after(boss::tide_hunter_state_machine),
                    boss::shadow_trail_damage_system.after(boss::shadow_trail_fade_system),
                    ai::boss_movement_override.after(ai::update_enemy_ai),
                    boss::boss_subcore_orbit,
                    boss::boss_core_shield_update,
                    boss::boss_core_phase_respawn.after(boss::boss_phase_controller),
                    boss::boss_mechanic_hint_system,
                )
                    .run_if(
                        in_state(AppState::InGame)
                            .or_else(
                                in_state(AppState::CoopGame)
                                    .and_then(is_coop_authority)
                                    .and_then(is_coop_simulation_active),
                            )
                            .and_then(in_state(GamePhase::Playing)),
                    ),
            )
            .add_systems(
                Update,
                (
                    boss_contact_damage_system,
                    charger_contact_damage_system,
                    charger_stun_visual_system,
                    charger_windup_visual_system,
                )
                    .run_if(
                        in_state(AppState::InGame)
                            .or_else(
                                in_state(AppState::CoopGame)
                                    .and_then(is_coop_authority)
                                    .and_then(is_coop_simulation_active),
                            )
                            .and_then(in_state(GamePhase::Playing)),
                    ),
            )
            .add_systems(
                Update,
                enemy_death_system.run_if(
                    in_state(AppState::InGame)
                        .or_else(
                            in_state(AppState::CoopGame)
                                .and_then(is_coop_authority)
                                .and_then(is_coop_simulation_active),
                        )
                        .and_then(in_state(GamePhase::Playing)),
                ),
            )
            .add_systems(
                Update,
                clear_enemy_attacks_on_room_clear.run_if(
                    in_state(AppState::InGame)
                        .or_else(
                            in_state(AppState::CoopGame)
                                .and_then(is_coop_authority)
                                .and_then(is_coop_simulation_active),
                        )
                        .and_then(in_state(GamePhase::Playing)),
                ),
            );
    }
}

#[derive(Resource, Default, Debug, Clone, Copy)]
pub(crate) struct SpawnedForRoom(pub Option<u32>);

#[derive(Resource, Debug)]
pub(crate) struct ClearGrace {
    pub(crate) last_room: Option<u32>,
    pub(crate) timer: Timer,
}

impl Default for ClearGrace {
    fn default() -> Self {
        Self {
            last_room: None,
            timer: Timer::from_seconds(0.0, TimerMode::Once),
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct EnemySpawnCount {
    pub current: u32,
}

fn enemy_buff_decay_system(
    mut commands: Commands,
    time: Res<Time>,
    mut buffed_enemies: Query<(Entity, &mut EnemyBuffState)>,
) {
    for (entity, mut buff) in &mut buffed_enemies {
        buff.timer.tick(time.delta());
        if buff.timer.finished() {
            commands.entity(entity).remove::<EnemyBuffState>();
        }
    }
}

pub fn room_entry_spawner(
    mut commands: Commands,
    mut spawned: ResMut<SpawnedForRoom>,
    data: Res<GameDataRegistry>,
    assets: Res<crate::core::assets::GameAssets>,
    coop_config: Option<Res<CoopNetConfig>>,
    coop_players: Query<(), With<CoopParticipant>>,
    layout: Res<FloorLayout>,
    current_room: Res<CurrentRoom>,
    mut room_state: ResMut<RoomState>,
    mut cleanup_q: ParamSet<(
        Query<Entity, (With<Enemy>, Without<Replicated>)>,
        Query<Entity, (With<Projectile>, Without<Replicated>)>,
        Query<Entity, (With<Hitbox>, Without<Replicated>)>,
        Query<Entity, With<ShopKiosk>>,
        Query<Entity, With<PuzzleEntity>>,
        Query<Entity, With<BossSubCore>>,
    )>,
    mut spawn_count: ResMut<EnemySpawnCount>,
    mut active_puzzle: ResMut<ActivePuzzle>,
    floor: Option<Res<FloorNumber>>,
) {
    if spawned.0 == Some(current_room.0.0) {
        return;
    }
    spawned.0 = Some(current_room.0.0);

    for entity in &cleanup_q.p0() {
        safe_despawn_recursive(&mut commands, entity);
    }
    for entity in &cleanup_q.p1() {
        safe_despawn_recursive(&mut commands, entity);
    }
    for entity in &cleanup_q.p2() {
        safe_despawn_recursive(&mut commands, entity);
    }
    for entity in &cleanup_q.p3() {
        safe_despawn_recursive(&mut commands, entity);
    }
    for entity in &cleanup_q.p4() {
        safe_despawn_recursive(&mut commands, entity);
    }
    for entity in &cleanup_q.p5() {
        safe_despawn_recursive(&mut commands, entity);
    }
    reset_active_puzzle(&mut active_puzzle);

    let room = layout.room(current_room.0).unwrap();
    let floor_number = floor.as_deref().map(|value| value.0).unwrap_or(1);
    let mut floor_multiplier = get_floor_difficulty_multiplier(&data, floor_number);
    let base_enemy_count = get_floor_enemy_count(&data, floor_number);
    let coop_host_active = coop_config
        .as_deref()
        .map(|value| {
            matches!(value.mode, NetMode::Host | NetMode::Server) && !coop_players.is_empty()
        })
        .unwrap_or(false);
    let coop_hp_mult = if coop_host_active { 2.0 } else { 1.0 };
    let room_type = if coop_host_active && room.room_type == RoomType::Event {
        RoomType::Normal
    } else {
        room.room_type
    };

    match room_type {
        RoomType::Start | RoomType::Reward | RoomType::Shop | RoomType::Event => {
            *room_state = RoomState::Idle;
        }
        RoomType::Normal => {
            *room_state = RoomState::Locked;
            if spawn_count.current == 0 {
                spawn_count.current = base_enemy_count;
            }
            let mut enemy_count = spawn_count.current;
            if floor_number == 1 {
                let easing = data.balance.floor1_easing;
                if current_room.0.0 == 1 {
                    let adjusted = (enemy_count as i32 + easing.room1_count_offset).max(0) as u32;
                    enemy_count = adjusted.max(easing.min_enemy_count);
                    floor_multiplier *= easing.room1_mult;
                } else if current_room.0.0 == 2 {
                    floor_multiplier *= easing.room2_mult;
                }
            }
            spawn_room_enemies(
                &mut commands,
                &assets,
                &data,
                enemy_count,
                floor_multiplier,
                floor_number,
                coop_hp_mult,
            );
        }
        RoomType::Elite => {
            *room_state = RoomState::Locked;
            floor_multiplier *= 1.3;
            spawn_elite_room_enemies(
                &mut commands,
                &assets,
                &data,
                floor_multiplier,
                floor_number,
                coop_hp_mult,
            );
        }
        RoomType::Boss => {
            *room_state = RoomState::BossFight;
            spawn_boss(
                &mut commands,
                &assets,
                &data,
                floor_number,
                floor_multiplier,
                coop_hp_mult,
            );
        }
    }
}

pub fn spawn_room_enemies(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    data: &GameDataRegistry,
    enemy_count: u32,
    floor_multiplier: f32,
    floor_number: u32,
    coop_hp_mult: f32,
) {
    let count = enemy_count as usize;
    let points = player_safe_spawn_points(spawner::get_spawn_points_for_room(), count);
    let pool = spawner::choose_enemy_types(data, floor_number);
    let frontline_pool = spawner::frontline_enemy_types(&pool);
    let backline_pool = spawner::backline_enemy_types(&pool);
    let mut rng = GameRng::default();
    let spawn_n = count.min(points.len());
    let frontline_in_pool = !frontline_pool.is_empty();
    let backline_in_pool = !backline_pool.is_empty();

    let elite_chance = elite_chance_for_floor(&data.balance, floor_number);
    let elite_idx = if spawn_n > 0 && rng.gen_range_f32(0.0, 1.0) < elite_chance {
        Some((rng.gen_range_f32(0.0, spawn_n as f32) as usize).min(spawn_n - 1))
    } else {
        None
    };
    let mut planned_types = vec![None; spawn_n];
    if spawn_n > 0 && frontline_in_pool {
        planned_types[0] = Some(spawner::pick_enemy_type(&mut rng, &frontline_pool));
    }
    if spawn_n > 1 && backline_in_pool {
        planned_types[1] = Some(spawner::pick_enemy_type(&mut rng, &backline_pool));
    }
    if floor_number == 3 && spawn_n > 2 {
        let special_pool = pool
            .iter()
            .copied()
            .filter(|enemy_type| matches!(enemy_type, EnemyType::Flanker | EnemyType::Sniper))
            .collect::<Vec<_>>();
        if !special_pool.is_empty() {
            planned_types[2] = Some(spawner::pick_enemy_type(&mut rng, &special_pool));
        }
    } else if floor_number >= 4 {
        if spawn_n > 2 {
            let aggressive_pool = pool
                .iter()
                .copied()
                .filter(|enemy_type| matches!(enemy_type, EnemyType::Charger | EnemyType::Flanker))
                .collect::<Vec<_>>();
            if !aggressive_pool.is_empty() {
                planned_types[2] = Some(spawner::pick_enemy_type(&mut rng, &aggressive_pool));
            }
        }
        if spawn_n > 3 {
            let pressure_pool = pool
                .iter()
                .copied()
                .filter(|enemy_type| {
                    matches!(enemy_type, EnemyType::Sniper | EnemyType::SupportCaster)
                })
                .collect::<Vec<_>>();
            if !pressure_pool.is_empty() {
                planned_types[3] = Some(spawner::pick_enemy_type(&mut rng, &pressure_pool));
            }
        }
    }

    for i in 0..spawn_n {
        let enemy_type =
            planned_types[i].unwrap_or_else(|| spawner::pick_enemy_type(&mut rng, &pool));
        let is_elite = elite_idx == Some(i) && enemy_type != EnemyType::SupportCaster;
        spawn_enemy(
            commands,
            assets,
            data,
            enemy_type,
            points[i],
            floor_number,
            floor_multiplier,
            coop_hp_mult,
            is_elite,
        );
    }
}

fn spawn_elite_room_enemies(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    data: &GameDataRegistry,
    floor_multiplier: f32,
    floor_number: u32,
    coop_hp_mult: f32,
) {
    let mut points = player_safe_spawn_points(spawner::get_spawn_points_for_room(), 3);
    points.sort_by(|a, b| a.length_squared().total_cmp(&b.length_squared()));

    let mut pool = spawner::choose_enemy_types(data, floor_number);
    pool.retain(|enemy_type| *enemy_type != EnemyType::Boss);
    let mut rng = GameRng::default();

    for (index, point) in points.into_iter().take(3).enumerate() {
        let enemy_type = spawner::pick_enemy_type(&mut rng, &pool);
        let is_elite = index == 0;
        if is_elite {
            spawn_enemy_with_elite_scale(
                commands,
                assets,
                data,
                enemy_type,
                point,
                floor_number,
                floor_multiplier,
                coop_hp_mult,
                true,
                data.balance.use_sprite_textures,
                1.4,
                1.0,
            );
        } else {
            spawn_enemy(
                commands,
                assets,
                data,
                enemy_type,
                point,
                floor_number,
                floor_multiplier,
                coop_hp_mult,
                false,
            );
        }
    }
}

fn player_safe_spawn_points(points: Vec<Vec2>, required_count: usize) -> Vec<Vec2> {
    let player_spawn = Vec2::new(-ROOM_HALF_WIDTH * 0.6, 0.0);
    let mut safe_points = points
        .iter()
        .copied()
        .filter(|point| point.distance(player_spawn) >= 120.0)
        .collect::<Vec<_>>();
    if safe_points.len() >= required_count || safe_points.len() == points.len() {
        return safe_points;
    }

    let mut fallback_points = points
        .into_iter()
        .filter(|point| point.distance(player_spawn) < 120.0)
        .collect::<Vec<_>>();
    fallback_points.sort_by(|a, b| {
        b.distance_squared(player_spawn)
            .total_cmp(&a.distance_squared(player_spawn))
    });
    safe_points.extend(fallback_points);
    safe_points
}

fn enemy_health_bar_size(is_elite: bool) -> Vec2 {
    if is_elite {
        Vec2::new(32.0, 4.0)
    } else {
        Vec2::new(24.0, 3.0)
    }
}

fn enemy_health_bar_height(bar_width: f32) -> f32 {
    if bar_width >= 32.0 { 4.0 } else { 3.0 }
}

fn enemy_health_bar_color(ratio: f32) -> Color {
    if ratio > 0.5 {
        Color::srgb(0.25, 0.90, 0.25)
    } else if ratio > 0.25 {
        Color::srgb(0.95, 0.82, 0.18)
    } else {
        Color::srgb(0.95, 0.18, 0.16)
    }
}

fn enemy_health_bar_translation(owner_translation: Vec3, z: f32) -> Vec3 {
    Vec3::new(owner_translation.x, owner_translation.y + 20.0, z)
}

fn enemy_health_bar_fill_translation(
    owner_translation: Vec3,
    bar_width: f32,
    fill_width: f32,
) -> Vec3 {
    Vec3::new(
        owner_translation.x - bar_width * 0.5 + fill_width * 0.5,
        owner_translation.y + 20.0,
        51.0,
    )
}

fn spawn_enemy_health_bars(
    mut commands: Commands,
    assets: Res<crate::core::assets::GameAssets>,
    enemies: Query<
        (Entity, &Health, Option<&Elite>, &GlobalTransform),
        (With<Enemy>, Without<BossArchetype>),
    >,
    health_bars: Query<&EnemyHealthBar>,
) {
    for (enemy, _health, elite, transform) in &enemies {
        if health_bars.iter().any(|bar| bar.owner == enemy) {
            continue;
        }

        let size = enemy_health_bar_size(elite.is_some());
        let owner_translation = transform.translation();
        commands.spawn((
            SpriteBundle {
                texture: assets.textures.white.clone(),
                transform: Transform::from_translation(enemy_health_bar_translation(
                    owner_translation,
                    50.0,
                )),
                sprite: Sprite {
                    color: Color::srgba(0.05, 0.05, 0.05, 0.78),
                    custom_size: Some(size),
                    ..default()
                },
                ..default()
            },
            EnemyHealthBar {
                owner: enemy,
                bar_width: size.x,
            },
            InGameEntity,
            Name::new("EnemyHealthBar"),
        ));

        commands.spawn((
            SpriteBundle {
                texture: assets.textures.white.clone(),
                transform: Transform::from_translation(enemy_health_bar_translation(
                    owner_translation,
                    51.0,
                )),
                sprite: Sprite {
                    color: Color::srgb(0.25, 0.90, 0.25),
                    custom_size: Some(size),
                    ..default()
                },
                ..default()
            },
            EnemyHealthBar {
                owner: enemy,
                bar_width: size.x,
            },
            EnemyHealthBarFill { owner: enemy },
            InGameEntity,
            Name::new("EnemyHealthBarFill"),
        ));
    }
}

fn update_enemy_health_bars(
    mut commands: Commands,
    owners: Query<(&Health, &GlobalTransform), With<Enemy>>,
    mut bars: Query<(Entity, &EnemyHealthBar, &mut Transform), Without<EnemyHealthBarFill>>,
    mut fills: Query<(
        Entity,
        &EnemyHealthBar,
        &EnemyHealthBarFill,
        &mut Transform,
        &mut Sprite,
    )>,
) {
    for (bar_entity, bar, mut transform) in &mut bars {
        let Ok((_health, owner_transform)) = owners.get(bar.owner) else {
            commands.entity(bar_entity).despawn_recursive();
            continue;
        };
        transform.translation = enemy_health_bar_translation(owner_transform.translation(), 50.0);
    }

    for (fill_entity, bar, fill, mut transform, mut sprite) in &mut fills {
        let Ok((health, owner_transform)) = owners.get(fill.owner) else {
            commands.entity(fill_entity).despawn_recursive();
            continue;
        };

        let ratio = if health.max > 0.0 {
            (health.current / health.max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let fill_width = bar.bar_width * ratio;
        sprite.custom_size = Some(Vec2::new(
            fill_width.max(0.0),
            enemy_health_bar_height(bar.bar_width),
        ));
        sprite.color = enemy_health_bar_color(ratio);
        transform.translation = enemy_health_bar_fill_translation(
            owner_transform.translation(),
            bar.bar_width,
            fill_width,
        );
    }
}

pub fn spawn_enemy(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    data: &GameDataRegistry,
    enemy_type: EnemyType,
    pos: Vec2,
    floor_number: u32,
    floor_multiplier: f32,
    coop_hp_mult: f32,
    is_elite: bool,
) -> Entity {
    spawn_enemy_with_elite_scale(
        commands,
        assets,
        data,
        enemy_type,
        pos,
        floor_number,
        floor_multiplier,
        coop_hp_mult,
        is_elite,
        data.balance.use_sprite_textures,
        1.0,
        1.3,
    )
}

fn spawn_enemy_with_elite_scale(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    data: &GameDataRegistry,
    enemy_type: EnemyType,
    pos: Vec2,
    floor_number: u32,
    floor_multiplier: f32,
    coop_hp_mult: f32,
    is_elite: bool,
    use_sprite_textures: bool,
    elite_transform_scale: f32,
    elite_sprite_scale: f32,
) -> Entity {
    let stats_cfg = match enemy_type {
        EnemyType::MeleeChaser => &data.enemies.melee_chaser,
        EnemyType::Lobber => &data.enemies.lobber,
        EnemyType::RangedShooter => &data.enemies.ranged_shooter,
        EnemyType::Charger => &data.enemies.charger,
        EnemyType::Flanker => &data.enemies.flanker,
        EnemyType::Sniper => &data.enemies.sniper,
        EnemyType::SupportCaster => &data.enemies.support_caster,
        EnemyType::Bomber => &data.enemies.bomber,
        EnemyType::Shielder => &data.enemies.shielder,
        EnemyType::Summoner => &data.enemies.summoner,
        EnemyType::Boss => &data.enemies.melee_chaser,
    };
    let mut stats = scaled_enemy_stats(data, stats_cfg, enemy_type, floor_number, floor_multiplier);
    stats.max_hp *= coop_hp_mult.max(1.0);
    let elite_affixes = if is_elite && enemy_type != EnemyType::Boss {
        let mut affix_rng = GameRng::default();
        let seed = floor_number as u64
            ^ ((pos.x.to_bits() as u64) << 1)
            ^ ((pos.y.to_bits() as u64) << 17);
        affix_rng.reseed(seed);
        let affixes = pick_elite_affixes(floor_number, &mut affix_rng);
        if affixes.contains(&EliteAffix::Swift) {
            stats.move_speed *=
                data.elite_affixes
                    .param_or(EliteAffix::Swift, "move_speed_mult", 1.40);
            stats.attack_cooldown_s *=
                data.elite_affixes
                    .param_or(EliteAffix::Swift, "attack_cooldown_mult", 0.70);
        }
        affixes
    } else {
        Vec::new()
    };
    if is_elite && enemy_type != EnemyType::Boss {
        stats.max_hp *= data.balance.elite_hp_mult.max(1.0);
        stats.attack_damage *= data.balance.elite_damage_mult.max(1.0);
    }
    let color = enemy_color(enemy_type, is_elite);
    let base_sprite_size = if enemy_type == EnemyType::Boss {
        56.0
    } else {
        match enemy_type {
            EnemyType::Charger => 30.0,
            EnemyType::SupportCaster => 30.0,
            EnemyType::Shielder => 32.0,
            EnemyType::Sniper => 27.0,
            EnemyType::Bomber => 26.0,
            EnemyType::Summoner => 25.0,
            EnemyType::Flanker => 24.0,
            _ => 28.0,
        }
    };
    let sprite_size = if is_elite && enemy_type != EnemyType::Boss {
        base_sprite_size * elite_sprite_scale
    } else {
        base_sprite_size
    };
    let hurtbox_size = if enemy_type == EnemyType::Boss {
        60.0
    } else {
        match enemy_type {
            EnemyType::Charger => 28.0,
            EnemyType::SupportCaster => 28.0,
            EnemyType::Shielder => 30.0,
            EnemyType::Sniper => 24.0,
            EnemyType::Bomber => 24.0,
            EnemyType::Summoner => 23.0,
            EnemyType::Flanker => 22.0,
            _ => 26.0,
        }
    };

    let transform = if is_elite
        && enemy_type != EnemyType::Boss
        && (elite_transform_scale - 1.0).abs() > f32::EPSILON
    {
        Transform::from_xyz(pos.x, pos.y, 45.0).with_scale(Vec3::splat(elite_transform_scale))
    } else {
        Transform::from_translation(pos.extend(45.0))
    };

    let (texture, sprite_color) = if use_sprite_textures {
        if let Some(tex) = assets.textures.enemy_sprites.get(&enemy_type) {
            let tint = if is_elite && enemy_type != EnemyType::Boss {
                Color::srgb(1.0, 0.92, 0.7)
            } else {
                Color::WHITE
            };
            (tex.clone(), tint)
        } else {
            (assets.textures.white.clone(), color)
        }
    } else {
        (assets.textures.white.clone(), color)
    };

    let mut entity = commands.spawn((
        SpriteBundle {
            texture,
            transform,
            sprite: Sprite {
                color: sprite_color,
                custom_size: Some(Vec2::splat(sprite_size)),
                ..default()
            },
            ..default()
        },
        Enemy,
        EnemyKind(enemy_type),
        TeamMarker(Team::Enemy),
        Health {
            current: stats.max_hp,
            max: stats.max_hp,
        },
        stats,
        EnemyAttackCooldown {
            timer: Timer::from_seconds(stats.attack_cooldown_s, TimerMode::Once),
        },
        EnemyVelocity::default(),
        Hurtbox {
            team: Team::Enemy,
            size: Vec2::splat(hurtbox_size),
        },
        Flash::new(0.0),
        Knockback(Vec2::ZERO),
        InGameEntity,
        Name::new("Enemy"),
    ));
    entity.insert((
        CoopNetPosition(pos),
        CoopNetVelocity(Vec2::ZERO),
        CoopNetRotation(0.0),
    ));

    if let Some(&primary_affix) = elite_affixes.first() {
        entity.insert((
            Elite,
            EliteAffixMarker(primary_affix),
            EliteAffixes(elite_affixes.clone()),
        ));
        for affix in &elite_affixes {
            match affix {
                EliteAffix::Shielded => {
                    let reduction =
                        data.elite_affixes
                            .param_or(EliteAffix::Shielded, "damage_reduction", 0.25);
                    let immune_knockback =
                        data.elite_affixes
                            .param_or(EliteAffix::Shielded, "immune_knockback", 1.0)
                            > 0.0;
                    let immune_freeze =
                        data.elite_affixes
                            .param_or(EliteAffix::Shielded, "immune_freeze", 1.0)
                            > 0.0;
                    entity.insert(ShieldedAffixState {
                        damage_reduction: reduction,
                        immune_knockback,
                        immune_freeze,
                    });
                }
                EliteAffix::Berserk => {
                    entity.insert(BerserkAffixState { active: false });
                }
                EliteAffix::Teleporting => {
                    let interval =
                        data.elite_affixes
                            .param_or(EliteAffix::Teleporting, "interval_s", 4.0);
                    entity.insert(TeleportAffixTimer {
                        timer: Timer::from_seconds(interval, TimerMode::Repeating),
                    });
                }
                EliteAffix::Swift | EliteAffix::Splitting | EliteAffix::Vampiric => {}
            }
        }
        entity.with_children(|parent| {
            let label = elite_affixes
                .iter()
                .map(|affix| affix.label())
                .collect::<Vec<_>>()
                .join("+");
            let label_color = primary_affix.color();
            // Outline: 4 black shadow copies offset by ±1px
            for &(dx, dy) in &[(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                parent.spawn(Text2dBundle {
                    text: Text::from_section(
                        label.clone(),
                        TextStyle {
                            font: assets.font.clone(),
                            font_size: 18.0,
                            color: Color::srgba(0.0, 0.0, 0.0, 0.9),
                        },
                    ),
                    transform: Transform::from_translation(Vec3::new(dx, 28.0 + dy, 9.9)),
                    ..default()
                });
            }
            // Main colored label
            parent.spawn((
                Text2dBundle {
                    text: Text::from_section(
                        label,
                        TextStyle {
                            font: assets.font.clone(),
                            font_size: 18.0,
                            color: label_color,
                        },
                    ),
                    transform: Transform::from_translation(Vec3::new(0.0, 28.0, 10.0)),
                    ..default()
                },
                EliteAffixLabel,
            ));
            // Body glow aura
            parent.spawn((
                SpriteBundle {
                    texture: assets.textures.white.clone(),
                    transform: Transform::from_translation(Vec3::new(0.0, 0.0, -0.5)),
                    sprite: Sprite {
                        color: label_color.with_alpha(0.18),
                        custom_size: Some(Vec2::splat(48.0)),
                        ..default()
                    },
                    ..default()
                },
                EliteGlow,
            ));
        });
    }

    if enemy_type == EnemyType::Charger {
        entity.insert(ChargerState {
            phase: ChargerPhase::Idle,
            timer: Timer::from_seconds(0.1, TimerMode::Once),
            dir: Vec2::X,
        });
    }
    if enemy_type == EnemyType::Flanker {
        entity.insert(FlankerState {
            phase: FlankerPhase::Stalk,
            timer: Timer::from_seconds(0.1, TimerMode::Once),
            dir: Vec2::X,
            strafe_sign: 1.0,
            repath_timer: Timer::from_seconds(0.40, TimerMode::Once),
        });
    }
    if enemy_type == EnemyType::Sniper {
        entity.insert(SniperState {
            phase: SniperPhase::Idle,
            timer: Timer::from_seconds(0.1, TimerMode::Once),
            aim_dir: Vec2::X,
        });
    }
    if enemy_type == EnemyType::Bomber {
        entity.insert(BomberState {
            phase: BomberPhase::Approach,
            timer: Timer::from_seconds(1.0, TimerMode::Once),
            explosion_radius: 65.0,
            explosion_damage: stats.attack_damage,
        });
    }
    if enemy_type == EnemyType::Shielder {
        entity.insert(ShielderState {
            facing: Vec2::X,
            shield_half_angle: std::f32::consts::FRAC_PI_3,
        });
    }
    if enemy_type == EnemyType::Summoner {
        entity.insert(SummonerState {
            summon_timer: Timer::from_seconds(stats.attack_cooldown_s, TimerMode::Once),
            max_active_summons: 3,
        });
    }

    entity.id()
}

pub fn spawn_boss(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    data: &GameDataRegistry,
    floor_number: u32,
    floor_multiplier: f32,
    coop_hp_mult: f32,
) -> Entity {
    let archetype = BossArchetype::from_floor(floor_number);
    let stats = scaled_boss_stats(data, archetype, floor_multiplier, floor_number);
    let stats = EnemyStats {
        max_hp: stats.max_hp * coop_hp_mult.max(1.0),
        ..stats
    };
    let (sprite_size, hurtbox_size) = match archetype {
        BossArchetype::Floor1Guardian => (72.0_f32, 68.0_f32),
        BossArchetype::MirrorWarden => (60.0, 56.0),
        BossArchetype::TideHunter => (32.0, 30.0),
        BossArchetype::CubeCore => (84.0, 80.0),
    };
    let use_textures = data.balance.use_sprite_textures;
    let (boss_texture, boss_sprite_color) = if use_textures {
        if let Some(tex) = assets.textures.boss_sprites.get(&archetype) {
            (tex.clone(), Color::WHITE)
        } else {
            (assets.textures.white.clone(), boss::boss_color(archetype))
        }
    } else {
        (assets.textures.white.clone(), boss::boss_color(archetype))
    };
    let id = commands
        .spawn((
            SpriteBundle {
                texture: boss_texture,
                transform: Transform::from_translation(Vec3::new(220.0, 0.0, 45.0)),
                sprite: Sprite {
                    color: boss_sprite_color,
                    custom_size: Some(Vec2::splat(sprite_size)),
                    ..default()
                },
                ..default()
            },
            Enemy,
            TeamMarker(Team::Enemy),
            Health {
                current: stats.max_hp,
                max: stats.max_hp,
            },
            stats,
            EnemyVelocity::default(),
            Hurtbox {
                team: Team::Enemy,
                size: Vec2::splat(hurtbox_size),
            },
            Flash::new(0.0),
            Knockback(Vec2::ZERO),
            InGameEntity,
            Name::new(boss::boss_name(archetype)),
        ))
        .id();
    commands.entity(id).insert((
        CoopNetPosition(Vec2::new(220.0, 0.0)),
        CoopNetVelocity(Vec2::ZERO),
        CoopNetRotation(0.0),
    ));
    let (kind, phase, timer, cycle) = boss::spawn_boss_bundle(data, archetype);
    commands.entity(id).insert((kind, archetype, phase));
    if !matches!(archetype, BossArchetype::TideHunter) {
        commands.entity(id).insert((timer, cycle));
    }
    match archetype {
        BossArchetype::Floor1Guardian => {
            commands.entity(id).insert(BossDirectionalDefense {
                facing: Vec2::NEG_X,
            });
            // 盾牌方向指示器：显示在 Boss 正面的橙色矩形
            let shield_indicator = commands
                .spawn((
                    SpriteBundle {
                        texture: assets.textures.white.clone(),
                        transform: Transform::from_translation(Vec3::new(-40.0, 0.0, 1.0)),
                        sprite: Sprite {
                            color: Color::srgba(1.0, 0.55, 0.1, 0.85),
                            custom_size: Some(Vec2::new(10.0, 52.0)),
                            ..default()
                        },
                        ..default()
                    },
                    GuardianShieldIndicator,
                    InGameEntity,
                ))
                .id();
            commands.entity(id).add_child(shield_indicator);
        }
        BossArchetype::TideHunter => {
            commands.entity(id).insert(TideHunterState {
                phase: TideHunterPhase::Stalk,
                timer: Timer::from_seconds(1.8, TimerMode::Once),
                dash_target: Vec2::ZERO,
                dash_start: Vec2::ZERO,
                dashes_remaining: 0,
                dashes_per_cycle: 1,
                shadow_duration_s: 2.5,
                stalk_duration_s: 1.8,
                reposition_duration_s: 0.9,
                contact_hit_cooldown: Timer::from_seconds(0.0, TimerMode::Once),
                parry_window_active: false,
            });
        }
        BossArchetype::CubeCore => {
            let boss_pos = Vec2::new(220.0, 0.0);
            commands
                .entity(id)
                .insert(BossCoreShield { cores_alive: 4 });
            for i in 0..4u8 {
                let angle = i as f32 / 4.0 * std::f32::consts::TAU;
                let spawn_pos = boss_pos + Vec2::new(angle.cos(), angle.sin()) * 85.0;
                boss::spawn_cube_core_subcore(commands, assets, id, spawn_pos, angle, 0.55, 40.0);
            }
        }
        BossArchetype::MirrorWarden => {}
    }
    id
}

fn bomber_fuse_system(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<crate::core::assets::GameAssets>,
    player_q: Query<(&GlobalTransform, Option<&GhostState>), (With<Player>, Without<Replicated>)>,
    mut bombers: Query<
        (
            Entity,
            &EnemyStats,
            &Transform,
            &mut BomberState,
            &mut Sprite,
            Option<&Elite>,
        ),
        (With<Enemy>, Without<Replicated>),
    >,
) {
    let player_positions = player_q
        .iter()
        .filter_map(|(tf, ghost)| {
            (!matches!(ghost, Some(GhostState::Ghost))).then_some(tf.translation().truncate())
        })
        .collect::<Vec<_>>();

    for (entity, stats, tf, mut state, mut sprite, elite) in &mut bombers {
        let pos = tf.translation.truncate();
        match state.phase {
            BomberPhase::Approach => {
                sprite.color = enemy_color(EnemyType::Bomber, elite.is_some());
                let Some(dist) = player_positions
                    .iter()
                    .map(|player_pos| pos.distance(*player_pos))
                    .min_by(|a, b| a.total_cmp(b))
                else {
                    continue;
                };
                if dist <= stats.attack_range {
                    state.phase = BomberPhase::Fuse;
                    state.timer = Timer::from_seconds(1.0, TimerMode::Once);
                    state.timer.reset();
                }
            }
            BomberPhase::Fuse => {
                state.timer.tick(time.delta());
                let pulse = ((state.timer.elapsed_secs() * 20.0).sin().abs()) > 0.45;
                sprite.color = if pulse {
                    Color::WHITE
                } else {
                    Color::srgb(1.0, 0.28, 0.22)
                };
                if state.timer.finished() {
                    spawn_enemy_explosion_hitbox(
                        &mut commands,
                        &assets,
                        entity,
                        pos,
                        state.explosion_radius,
                        state.explosion_damage,
                    );
                    state.phase = BomberPhase::Exploded;
                    safe_despawn_recursive(&mut commands, entity);
                }
            }
            BomberPhase::Exploded => {
                safe_despawn_recursive(&mut commands, entity);
            }
        }
    }
}

fn shielder_facing_system(
    time: Res<Time>,
    player_q: Query<(&GlobalTransform, Option<&GhostState>), (With<Player>, Without<Replicated>)>,
    mut shielders: Query<(&mut Transform, &mut ShielderState), (With<Enemy>, Without<Replicated>)>,
) {
    let player_positions = player_q
        .iter()
        .filter_map(|(tf, ghost)| {
            (!matches!(ghost, Some(GhostState::Ghost))).then_some(tf.translation().truncate())
        })
        .collect::<Vec<_>>();
    if player_positions.is_empty() {
        return;
    }

    for (mut tf, mut state) in &mut shielders {
        let pos = tf.translation.truncate();
        let Some(player_pos) = player_positions
            .iter()
            .copied()
            .min_by(|a, b| pos.distance(*a).total_cmp(&pos.distance(*b)))
        else {
            continue;
        };
        let target_dir = direction_to(pos, player_pos);
        if target_dir.length_squared() <= f32::EPSILON {
            continue;
        }
        let current_facing = if state.facing.length_squared() <= f32::EPSILON {
            target_dir
        } else {
            state.facing
        };
        state.facing = current_facing
            .lerp(target_dir, (time.delta_seconds() * 5.0).clamp(0.0, 1.0))
            .normalize_or_zero();
        if state.facing.length_squared() > f32::EPSILON {
            tf.rotation = Quat::from_rotation_z(state.facing.y.atan2(state.facing.x));
        }
    }
}

fn shielder_block_system(
    mut commands: Commands,
    assets: Res<crate::core::assets::GameAssets>,
    shielders: Query<
        (&GlobalTransform, &Hurtbox, &ShielderState),
        (With<Enemy>, Without<Replicated>),
    >,
    hitboxes: Query<(Entity, &Hitbox, &GlobalTransform), Without<Replicated>>,
) {
    let mut blocked_hitboxes = Vec::new();

    for (shielder_tf, hurtbox, state) in &shielders {
        let shielder_aabb = aabb_from_transform_size(shielder_tf, hurtbox.size);
        let shielder_pos = shielder_tf.translation().truncate();
        let facing = state.facing.normalize_or_zero();
        let facing_cos = state.shield_half_angle.cos();

        for (hitbox_entity, hitbox, hitbox_tf) in &hitboxes {
            if blocked_hitboxes.contains(&hitbox_entity) {
                continue;
            }
            if hitbox.team != Team::Player || hitbox.damage_kind != DamageKind::PlayerRanged {
                continue;
            }

            let hitbox_aabb = aabb_from_transform_size(hitbox_tf, hitbox.size);
            if !shielder_aabb.intersects(hitbox_aabb) {
                continue;
            }

            let to_hitbox = hitbox_tf.translation().truncate() - shielder_pos;
            if to_hitbox.length_squared() > f32::EPSILON
                && facing.length_squared() > f32::EPSILON
                && facing.dot(to_hitbox.normalize()) < facing_cos
            {
                continue;
            }

            blocked_hitboxes.push(hitbox_entity);
            particles::spawn_hit_particles(
                &mut commands,
                &assets,
                hitbox_tf.translation().truncate(),
                Color::srgb(0.45, 0.62, 0.88),
            );
            safe_despawn_recursive(&mut commands, hitbox_entity);
        }
    }
}

fn summoner_summon_system(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<crate::core::assets::GameAssets>,
    data: Res<GameDataRegistry>,
    mut rng: ResMut<GameRng>,
    coop_config: Option<Res<CoopNetConfig>>,
    coop_players: Query<(), With<CoopParticipant>>,
    floor: Option<Res<FloorNumber>>,
    mut summoners: Query<
        (Entity, &Transform, &mut SummonerState),
        (With<Enemy>, Without<Replicated>),
    >,
    summons: Query<&SummonedBy, Without<Replicated>>,
) {
    let floor_number = floor.as_deref().map(|value| value.0).unwrap_or(1);
    let floor_multiplier = get_floor_difficulty_multiplier(&data, floor_number);
    let coop_hp_mult = if coop_config
        .as_deref()
        .map(|value| {
            matches!(value.mode, NetMode::Host | NetMode::Server) && !coop_players.is_empty()
        })
        .unwrap_or(false)
    {
        2.0
    } else {
        1.0
    };

    for (entity, tf, mut state) in &mut summoners {
        state.summon_timer.tick(time.delta());
        if !state.summon_timer.finished() {
            continue;
        }

        let active_summons = summons
            .iter()
            .filter(|summoned_by| summoned_by.0 == entity)
            .count() as u8;
        if active_summons >= state.max_active_summons {
            continue;
        }

        let remaining_slots = state.max_active_summons.saturating_sub(active_summons);
        let desired_summons: usize = if remaining_slots <= 1 || rng.gen_range_f32(0.0, 1.0) < 0.5 {
            1
        } else {
            2
        };
        let summon_count = desired_summons.min(remaining_slots as usize);
        let base_pos = tf.translation.truncate();
        let base_angle = rng.gen_range_f32(0.0, std::f32::consts::TAU);

        for index in 0..summon_count {
            let angle = base_angle + (index as f32 - (summon_count as f32 - 1.0) * 0.5) * 0.9;
            let offset = Vec2::new(angle.cos(), angle.sin()) * 48.0;
            let summon_pos = clamp_in_room(
                base_pos + offset,
                Vec2::new(ROOM_HALF_WIDTH, ROOM_HALF_HEIGHT),
                28.0,
            );
            let summoned = spawn_enemy(
                &mut commands,
                &assets,
                &data,
                EnemyType::MeleeChaser,
                summon_pos,
                floor_number,
                floor_multiplier,
                coop_hp_mult,
                false,
            );
            commands.entity(summoned).insert(SummonedBy(entity));
        }

        state.summon_timer.reset();
    }
}

fn summoner_death_cleanup(
    mut commands: Commands,
    mut death_events: EventReader<DeathEvent>,
    enemy_kinds: Query<&EnemyKind>,
    summons: Query<(Entity, &SummonedBy)>,
) {
    for death in death_events.read() {
        if enemy_kinds
            .get(death.entity)
            .map(|kind| kind.0 != EnemyType::Summoner)
            .unwrap_or(true)
        {
            continue;
        }

        for (summoned, summoned_by) in &summons {
            if summoned_by.0 == death.entity {
                safe_despawn_recursive(&mut commands, summoned);
            }
        }
    }
}

fn elite_splitting_system(
    mut commands: Commands,
    assets: Res<crate::core::assets::GameAssets>,
    data: Res<GameDataRegistry>,
    mut rng: ResMut<GameRng>,
    mut death_events: EventReader<DeathEvent>,
    coop_config: Option<Res<CoopNetConfig>>,
    coop_players: Query<(), With<CoopParticipant>>,
    floor: Option<Res<FloorNumber>>,
    elites: Query<
        (&EnemyKind, &Transform, &EliteAffixes, &EnemyStats, &Health),
        (With<Enemy>, Without<Replicated>),
    >,
) {
    let floor_number = floor.as_deref().map(|value| value.0).unwrap_or(1);
    let floor_multiplier = get_floor_difficulty_multiplier(&data, floor_number);
    let coop_hp_mult = if coop_config
        .as_deref()
        .map(|value| {
            matches!(value.mode, NetMode::Host | NetMode::Server) && !coop_players.is_empty()
        })
        .unwrap_or(false)
    {
        2.0
    } else {
        1.0
    };

    for death in death_events.read() {
        let Ok((kind, tf, affixes, stats, health)) = elites.get(death.entity) else {
            continue;
        };
        if !affixes.contains(EliteAffix::Splitting) {
            continue;
        }

        if kind.0 == EnemyType::Boss {
            continue;
        }

        let hp_fraction = data
            .elite_affixes
            .param_or(EliteAffix::Splitting, "hp_fraction", 0.50);
        let damage_fraction =
            data.elite_affixes
                .param_or(EliteAffix::Splitting, "damage_fraction", 0.50);
        let count = (data
            .elite_affixes
            .param_or(EliteAffix::Splitting, "count", 2.0)
            .round() as i32)
            .max(0) as u32;
        let mut split_stats = *stats;
        split_stats.max_hp = health.max * hp_fraction;
        split_stats.attack_damage *= damage_fraction;

        let origin = tf.translation.truncate();
        let base_angle = rng.gen_range_f32(0.0, std::f32::consts::TAU);
        for index in 0..count {
            // count == 2 keeps the original tight ±0.55 split for visual
            // continuity; larger counts fan out evenly across TAU.
            let angle = if count == 2 {
                base_angle + if index == 0 { -0.55 } else { 0.55 }
            } else {
                base_angle + std::f32::consts::TAU * index as f32 / count as f32
            };
            let offset = Vec2::new(angle.cos(), angle.sin()) * 32.0;
            let spawn_pos = clamp_in_room(
                origin + offset,
                Vec2::new(ROOM_HALF_WIDTH, ROOM_HALF_HEIGHT),
                28.0,
            );
            let split = spawn_enemy(
                &mut commands,
                &assets,
                &data,
                kind.0,
                spawn_pos,
                floor_number,
                floor_multiplier,
                coop_hp_mult,
                false,
            );
            commands.entity(split).insert((
                Health {
                    current: split_stats.max_hp,
                    max: split_stats.max_hp,
                },
                split_stats,
                SplitSpawn,
                EnemyAttackCooldown {
                    timer: Timer::from_seconds(split_stats.attack_cooldown_s, TimerMode::Once),
                },
            ));
            if kind.0 == EnemyType::Bomber {
                commands.entity(split).insert(BomberState {
                    phase: BomberPhase::Approach,
                    timer: Timer::from_seconds(1.0, TimerMode::Once),
                    explosion_radius: 65.0,
                    explosion_damage: split_stats.attack_damage,
                });
            } else if kind.0 == EnemyType::Summoner {
                commands.entity(split).insert(SummonerState {
                    summon_timer: Timer::from_seconds(
                        split_stats.attack_cooldown_s,
                        TimerMode::Once,
                    ),
                    max_active_summons: 3,
                });
            }
        }
    }
}

fn elite_vampiric_system(
    data: Res<GameDataRegistry>,
    mut damage_events: EventReader<DamageAppliedEvent>,
    mut elites: Query<(&EliteAffixes, &mut Health), Without<Replicated>>,
) {
    let lifesteal_fraction =
        data.elite_affixes
            .param_or(EliteAffix::Vampiric, "lifesteal_fraction", 0.30);
    for event in damage_events.read() {
        if event.target_team != Some(Team::Player) || event.attacker_team != Team::Enemy {
            continue;
        }
        let Some(source) = event.source else {
            continue;
        };
        let Ok((affixes, mut health)) = elites.get_mut(source) else {
            continue;
        };
        if !affixes.contains(EliteAffix::Vampiric) {
            continue;
        }

        let heal = event.amount * lifesteal_fraction;
        health.current = (health.current + heal).min(health.max);
    }
}

fn elite_berserk_system(
    data: Res<GameDataRegistry>,
    mut elites: Query<
        (
            &Health,
            &mut EnemyStats,
            &mut BerserkAffixState,
            &mut Sprite,
        ),
        (With<Enemy>, Without<Replicated>),
    >,
) {
    let threshold = data
        .elite_affixes
        .param_or(EliteAffix::Berserk, "hp_threshold", 0.40);
    let damage_bonus = data
        .elite_affixes
        .param_or(EliteAffix::Berserk, "damage_bonus", 0.50);
    let attack_speed_bonus =
        data.elite_affixes
            .param_or(EliteAffix::Berserk, "attack_speed_bonus", 0.40);
    let move_speed_bonus =
        data.elite_affixes
            .param_or(EliteAffix::Berserk, "move_speed_bonus", 0.20);
    for (health, mut stats, mut state, mut sprite) in &mut elites {
        if state.active || health.max <= 0.0 || health.current / health.max >= threshold {
            continue;
        }

        state.active = true;
        stats.attack_damage *= 1.0 + damage_bonus;
        stats.attack_cooldown_s /= (1.0 + attack_speed_bonus).max(0.1);
        stats.move_speed *= 1.0 + move_speed_bonus;
        sprite.color = Color::srgb(0.96, 0.22, 0.20);
    }
}

fn elite_teleport_system(
    time: Res<Time>,
    mut rng: ResMut<GameRng>,
    player_q: Query<(&GlobalTransform, Option<&GhostState>), (With<Player>, Without<Replicated>)>,
    mut elites: Query<
        (&mut Transform, &mut TeleportAffixTimer),
        (With<Enemy>, Without<Replicated>),
    >,
) {
    let player_positions = player_q
        .iter()
        .filter_map(|(tf, ghost)| {
            (!matches!(ghost, Some(GhostState::Ghost))).then_some(tf.translation().truncate())
        })
        .collect::<Vec<_>>();
    if player_positions.is_empty() {
        return;
    }

    for (mut tf, mut teleport) in &mut elites {
        teleport.timer.tick(time.delta());
        if !teleport.timer.just_finished() {
            continue;
        }

        let pos = tf.translation.truncate();
        let Some(player_pos) = player_positions
            .iter()
            .copied()
            .min_by(|a, b| pos.distance(*a).total_cmp(&pos.distance(*b)))
        else {
            continue;
        };
        let dir = direction_to(pos, player_pos);
        if dir.length_squared() <= f32::EPSILON {
            continue;
        }

        let desired_separation = rng.gen_range_f32(80.0, 120.0);
        let max_blink = rng.gen_range_f32(80.0, 120.0);
        let toward_player = (pos.distance(player_pos) - desired_separation)
            .max(0.0)
            .min(max_blink);
        let mut target_pos = pos + dir * toward_player;
        if toward_player <= f32::EPSILON {
            let side_sign = if rng.gen_bool(0.5) { 1.0 } else { -1.0 };
            let side = Vec2::new(-dir.y, dir.x) * side_sign * 20.0;
            target_pos = player_pos - dir * desired_separation + side;
        }
        let clamped = clamp_in_room(
            target_pos,
            Vec2::new(ROOM_HALF_WIDTH, ROOM_HALF_HEIGHT),
            28.0,
        );
        tf.translation.x = clamped.x;
        tf.translation.y = clamped.y;
    }
}

pub fn enemy_attack_system(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<crate::core::assets::GameAssets>,
    player_q: Query<(&GlobalTransform, Option<&GhostState>), (With<Player>, Without<Replicated>)>,
    mut enemy_sets: ParamSet<(
        Query<(Entity, &EnemyKind, &GlobalTransform), (With<Enemy>, Without<Replicated>)>,
        Query<
            (
                Entity,
                &EnemyKind,
                Option<&Elite>,
                &EnemyStats,
                &GlobalTransform,
                &mut EnemyAttackCooldown,
                Option<&EnemyBuffState>,
                Option<&mut SniperState>,
                Option<&ShielderState>,
                Option<&mut Sprite>,
            ),
            (With<Enemy>, Without<Replicated>),
        >,
    )>,
) {
    let player_positions: Vec<Vec2> = player_q
        .iter()
        .filter_map(|(tf, ghost)| {
            (!matches!(ghost, Some(GhostState::Ghost))).then_some(tf.translation().truncate())
        })
        .collect();
    if player_positions.is_empty() {
        return;
    }
    let enemy_positions = enemy_sets
        .p0()
        .iter()
        .map(|(entity, kind, tf)| (entity, kind.0, tf.translation().truncate()))
        .collect::<Vec<_>>();

    for (entity, kind, elite, stats, tf, mut cd, buff, sniper_state, shielder_state, sprite) in
        &mut enemy_sets.p1()
    {
        let effective_cooldown = effective_enemy_attack_cooldown(stats.attack_cooldown_s, buff);
        cd.timer.tick(time.delta());
        let pos = tf.translation().truncate();
        let (player_pos, dist) = player_positions
            .iter()
            .map(|p| (*p, pos.distance(*p)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        let dir = direction_to(pos, player_pos);
        let mut sniper_state = sniper_state;
        let mut sprite = sprite;

        if let Some(sniper) = sniper_state.as_mut() {
            sniper.timer.tick(time.delta());
            match sniper.phase {
                SniperPhase::Aiming if sniper.timer.finished() => {
                    if let Some(sprite) = sprite.as_mut() {
                        sprite.color = enemy_color(kind.0, elite.is_some());
                    }
                    for (angle, damage_mult) in [(-0.16_f32, 0.75_f32), (0.0, 1.0), (0.16, 0.75)] {
                        let shot_dir = Mat2::from_angle(angle).mul_vec2(sniper.aim_dir);
                        projectiles::spawn_projectile_with_owner(
                            &mut commands,
                            &assets,
                            entity,
                            Team::Enemy,
                            pos + shot_dir * 20.0,
                            shot_dir * stats.projectile_speed,
                            stats.attack_damage * damage_mult,
                        );
                    }
                    sniper.phase = SniperPhase::Recover;
                    sniper.timer = Timer::from_seconds(0.24, TimerMode::Once);
                    sniper.timer.reset();
                    cd.timer = Timer::from_seconds(effective_cooldown, TimerMode::Once);
                    cd.timer.reset();
                    continue;
                }
                SniperPhase::Recover if sniper.timer.finished() => {
                    if let Some(sprite) = sprite.as_mut() {
                        sprite.color = enemy_color(kind.0, elite.is_some());
                    }
                    sniper.phase = SniperPhase::Idle;
                }
                SniperPhase::Aiming | SniperPhase::Recover => continue,
                SniperPhase::Idle => {}
            }
        }

        if !cd.timer.finished() || dist > stats.aggro_range {
            continue;
        }

        match kind.0 {
            EnemyType::MeleeChaser | EnemyType::Flanker | EnemyType::Shielder => {
                if dist <= stats.attack_range {
                    cd.timer = Timer::from_seconds(effective_cooldown, TimerMode::Once);
                    cd.timer.reset();
                    let melee_dir = if kind.0 == EnemyType::Shielder {
                        shielder_state
                            .map(|state| state.facing.normalize_or_zero())
                            .filter(|facing| facing.length_squared() > f32::EPSILON)
                            .unwrap_or(dir)
                    } else {
                        dir
                    };
                    spawn_enemy_melee_hitbox(
                        &mut commands,
                        &assets,
                        entity,
                        pos,
                        melee_dir,
                        if kind.0 == EnemyType::Flanker {
                            stats.attack_damage * 0.92
                        } else {
                            stats.attack_damage
                        },
                    );
                }
            }
            EnemyType::RangedShooter | EnemyType::Lobber => {
                if dist <= stats.attack_range {
                    cd.timer = Timer::from_seconds(effective_cooldown, TimerMode::Once);
                    cd.timer.reset();
                    if kind.0 == EnemyType::Lobber {
                        spawn_lobber_attack(&mut commands, &assets, entity, player_pos, stats);
                    } else {
                        projectiles::spawn_projectile_with_owner(
                            &mut commands,
                            &assets,
                            entity,
                            Team::Enemy,
                            pos + dir * 18.0,
                            dir * stats.projectile_speed,
                            stats.attack_damage,
                        );
                    }
                }
            }
            EnemyType::Sniper => {
                if dist <= stats.attack_range {
                    if let Some(sniper) = sniper_state.as_mut() {
                        sniper.phase = SniperPhase::Aiming;
                        sniper.timer = Timer::from_seconds(0.32, TimerMode::Once);
                        sniper.timer.reset();
                        sniper.aim_dir = dir;
                    }
                    if let Some(sprite) = sprite.as_mut() {
                        sprite.color = Color::srgb(1.0, 0.93, 0.62);
                    }
                }
            }
            EnemyType::SupportCaster => {
                let mut buffed_any = false;
                let nearest_player_pos = Some(player_pos);
                let mut candidates = enemy_positions
                    .iter()
                    .copied()
                    .filter(|(ally_entity, ally_kind, ally_pos)| {
                        *ally_entity != entity
                            && !matches!(ally_kind, EnemyType::Boss | EnemyType::SupportCaster)
                            && pos.distance(*ally_pos) <= stats.attack_range
                    })
                    .collect::<Vec<_>>();
                candidates.sort_by(|a, b| pos.distance(a.2).total_cmp(&pos.distance(b.2)));
                for (ally_entity, _, ally_pos) in candidates.into_iter().take(3) {
                    commands.entity(ally_entity).insert(EnemyBuffState {
                        speed_mult: 1.50,
                        cooldown_mult: 1.67,
                        timer: Timer::from_seconds(3.8, TimerMode::Once),
                    });
                    particles::spawn_hit_particles(
                        &mut commands,
                        &assets,
                        ally_pos,
                        Color::srgba(0.55, 0.95, 0.88, 0.82),
                    );
                    buffed_any = true;
                }
                if let Some(target_pos) = nearest_player_pos {
                    let dir = (target_pos - pos).normalize_or_zero();
                    projectiles::spawn_projectile_with_owner(
                        &mut commands,
                        &assets,
                        entity,
                        Team::Enemy,
                        pos,
                        dir * stats.projectile_speed,
                        stats.attack_damage,
                    );
                }
                cd.timer = Timer::from_seconds(
                    if buffed_any {
                        effective_cooldown
                    } else {
                        (effective_cooldown * 0.42).max(0.55)
                    },
                    TimerMode::Once,
                );
                cd.timer.reset();
            }
            EnemyType::Charger | EnemyType::Bomber | EnemyType::Summoner => {}
            EnemyType::Boss => {}
        }
    }
}

fn spawn_lobber_attack(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    owner: Entity,
    target_pos: Vec2,
    stats: &EnemyStats,
) {
    let radius = lobber_impact_radius();
    commands.spawn((
        SpriteBundle {
            texture: assets.textures.white.clone(),
            transform: Transform::from_translation(target_pos.extend(38.0)),
            sprite: Sprite {
                color: Color::srgba(1.0, 0.18, 0.12, 0.28),
                custom_size: Some(Vec2::splat(radius * 2.0)),
                ..default()
            },
            ..default()
        },
        Lifetime(Timer::from_seconds(
            lobber_warning_seconds(),
            TimerMode::Once,
        )),
        InGameEntity,
        Name::new("LobberWarning"),
    ));
    commands.spawn((
        TransformBundle::from_transform(Transform::from_translation(target_pos.extend(39.0))),
        LobberImpact {
            owner,
            timer: Timer::from_seconds(lobber_warning_seconds(), TimerMode::Once),
            damage: stats.attack_damage,
            radius,
        },
        InGameEntity,
        Name::new("LobberImpactTimer"),
    ));
}

fn resolve_lobber_impacts(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<crate::core::assets::GameAssets>,
    mut impacts: Query<(Entity, &mut LobberImpact, &GlobalTransform), Without<Replicated>>,
) {
    for (entity, mut impact, tf) in &mut impacts {
        impact.timer.tick(time.delta());
        if !impact.timer.finished() {
            continue;
        }
        let pos = tf.translation().truncate();
        commands.spawn((
            SpriteBundle {
                texture: assets.textures.white.clone(),
                transform: Transform::from_translation(pos.extend(40.0)),
                sprite: Sprite {
                    color: Color::srgba(1.0, 0.42, 0.18, 0.42),
                    custom_size: Some(Vec2::splat(impact.radius * 2.0)),
                    ..default()
                },
                ..default()
            },
            Hitbox {
                owner: Some(impact.owner),
                team: Team::Enemy,
                damage_kind: DamageKind::Enemy,
                size: Vec2::splat(impact.radius * 2.0),
                damage: impact.damage,
                knockback: 180.0,
                can_crit: false,
                crit_chance: 0.0,
                crit_multiplier: 1.0,
            },
            Lifetime(Timer::from_seconds(0.12, TimerMode::Once)),
            InGameEntity,
            Name::new("LobberImpact"),
        ));
        safe_despawn_recursive(&mut commands, entity);
    }
}

fn lobber_warning_seconds() -> f32 {
    0.6
}

fn lobber_impact_radius() -> f32 {
    54.0
}

fn spawn_enemy_melee_hitbox(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    owner: Entity,
    pos: Vec2,
    dir: Vec2,
    damage: f32,
) {
    commands.spawn((
        SpriteBundle {
            texture: assets.textures.white.clone(),
            transform: Transform::from_translation((pos + dir * 24.0).extend(55.0)),
            sprite: Sprite {
                color: Color::srgba(1.0, 0.3, 0.25, 0.28),
                custom_size: Some(Vec2::new(40.0, 22.0)),
                ..default()
            },
            ..default()
        },
        Hitbox {
            owner: Some(owner),
            team: Team::Enemy,
            damage_kind: DamageKind::Enemy,
            size: Vec2::new(40.0, 22.0),
            damage,
            knockback: 300.0,
            can_crit: false,
            crit_chance: 0.0,
            crit_multiplier: 1.0,
        },
        Lifetime(Timer::from_seconds(0.10, TimerMode::Once)),
        InGameEntity,
        Name::new("EnemyHitbox"),
    ));
}

fn spawn_enemy_explosion_hitbox(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    owner: Entity,
    pos: Vec2,
    radius: f32,
    damage: f32,
) {
    commands.spawn((
        SpriteBundle {
            texture: assets.textures.white.clone(),
            transform: Transform::from_translation(pos.extend(55.0)),
            sprite: Sprite {
                color: Color::srgba(1.0, 0.42, 0.18, 0.30),
                custom_size: Some(Vec2::splat(radius * 2.0)),
                ..default()
            },
            ..default()
        },
        Hitbox {
            owner: Some(owner),
            team: Team::Enemy,
            damage_kind: DamageKind::Enemy,
            size: Vec2::splat(radius * 2.0),
            damage,
            knockback: 420.0,
            can_crit: false,
            crit_chance: 0.0,
            crit_multiplier: 1.0,
        },
        Lifetime(Timer::from_seconds(0.10, TimerMode::Once)),
        InGameEntity,
        Name::new("BomberExplosion"),
    ));
}

pub fn enemy_death_system(
    mut commands: Commands,
    mut death_events: EventReader<DeathEvent>,
    mut charge_events: EventWriter<ChargeGainEvent>,
    mut _xp_events: EventWriter<XpGainEvent>,
    mut room_cleared: EventWriter<RoomClearedEvent>,
    time: Res<Time>,
    assets: Res<crate::core::assets::GameAssets>,
    room_ctx: (
        Option<Res<FloorLayout>>,
        Option<Res<CurrentRoom>>,
        Option<ResMut<RoomState>>,
    ),
    mut player_q: ParamSet<(
        Query<
            (
                Entity,
                &RewardModifiers,
                &mut PlayerHealth,
                &mut DashCooldown,
                &mut Gold,
                &GlobalTransform,
                Option<&AugmentInventory>,
                Option<&GhostState>,
            ),
            (With<Player>, Without<Replicated>),
        >,
    )>,
    enemy_queries: (
        Query<(&EnemyKind, Option<&Elite>)>,
        Query<(), With<BossSubCore>>,
        Query<Entity, (With<Enemy>, Without<Replicated>)>,
        Query<Entity, With<BossSummoned>>,
        Query<Entity, With<BossSubCore>>,
    ),
    mut grace: ResMut<ClearGrace>,
    mut spawn_count: ResMut<EnemySpawnCount>,
    data: Res<GameDataRegistry>,
    active_event: Res<ActiveEvent>,
    coop_config: Option<Res<CoopNetConfig>>,
    floor: Option<Res<FloorNumber>>,
) {
    for ev in death_events.read() {
        if ev.team != Team::Enemy {
            continue;
        }

        let (kind, is_elite) = enemy_queries
            .0
            .get(ev.entity)
            .ok()
            .map(|(k, e)| (Some(k.0), e.is_some()))
            .unwrap_or((None, false));
        let is_sub_core = enemy_queries.1.get(ev.entity).is_ok();
        let charge_gain = if is_elite {
            data.player.elite_kill_charge_gain
        } else {
            data.player.kill_charge_gain
        };

        // Gold and XP now handled by drops system (src/gameplay/drops/mod.rs)

        for (player_e, mods, mut hp, mut dash_cd, _gold, player_tf, inventory, ghost) in
            &mut player_q.p0()
        {
            if matches!(ghost, Some(GhostState::Ghost)) {
                continue;
            }
            charge_events.send(ChargeGainEvent {
                player: player_e,
                amount: charge_gain,
            });
            if mods.lifesteal_on_kill > 0.0 {
                let previous = hp.current;
                hp.current = (hp.current + mods.lifesteal_on_kill).min(hp.max);
                if hp.current > previous {
                    particles::spawn_hit_particles(
                        &mut commands,
                        &assets,
                        player_tf.translation().truncate(),
                        Color::srgba(0.42, 1.0, 0.52, 0.88),
                    );
                }
            }

            if ev.source != Some(player_e) {
                continue;
            }

            let kill_heal = inventory
                .map(|value| {
                    tuning::kill_heal_amount(&data, value.stacks(AugmentId::KillHeal))
                        + tuning::lifesteal_kill_heal(
                            &data,
                            value.stacks(AugmentId::LifestealSlash),
                        )
                })
                .unwrap_or(0.0);
            if kill_heal > 0.0 {
                hp.current = (hp.current + kill_heal).min(hp.max);
            }

            let dash_reset_stacks = inventory
                .map(|value| value.stacks(AugmentId::DashReset))
                .unwrap_or(0);
            if dash_reset_stacks > 0 {
                let dash_cd_duration = dash_cd.base_duration_s.max(0.01);
                dash_cd.timer = Timer::from_seconds(dash_cd_duration, TimerMode::Once);
                dash_cd
                    .timer
                    .tick(Duration::from_secs_f32(dash_cd_duration));
                if dash_reset_stacks >= 2 {
                    commands.entity(player_e).insert(DashResetSpeedBuff {
                        timer: Timer::from_seconds(2.0, TimerMode::Once),
                        move_speed_mult: 1.30,
                    });
                }
            }
        }
        safe_despawn_recursive(&mut commands, ev.entity);
        if matches!(kind, Some(EnemyType::Boss)) && !is_sub_core {
            for summoned in &enemy_queries.3 {
                if summoned != ev.entity {
                    safe_despawn_recursive(&mut commands, summoned);
                }
            }
            for core_entity in &enemy_queries.4 {
                safe_despawn_recursive(&mut commands, core_entity);
            }
        }
    }

    let (Some(layout), Some(current_room), Some(mut room_state)) = room_ctx else {
        return;
    };

    if matches!(*room_state, RoomState::Locked | RoomState::BossFight) {
        let room = layout.room(current_room.0).unwrap();
        let coop_host_active = coop_config
            .as_deref()
            .map(|value| matches!(value.mode, NetMode::Host | NetMode::Server))
            .unwrap_or(false);
        let room_type = if coop_host_active && room.room_type == RoomType::Event {
            RoomType::Normal
        } else {
            room.room_type
        };
        let is_puzzle_event = room_type == RoomType::Event
            && active_event.room == Some(current_room.0)
            && active_event.event_type.is_some_and(EventType::is_puzzle);
        if is_puzzle_event {
            return;
        }
        if grace.last_room != Some(current_room.0.0) {
            grace.last_room = Some(current_room.0.0);
            grace.timer = Timer::from_seconds(0.20, TimerMode::Once);
            grace.timer.reset();
        }
        if !grace.timer.finished() {
            grace.timer.tick(time.delta());
            return;
        }

        let any_enemy_left = enemy_queries.2.iter().next().is_some();
        if !any_enemy_left {
            *room_state = RoomState::Cleared;
            room_cleared.send(RoomClearedEvent {
                room: current_room.0,
            });
            if room_type == RoomType::Normal {
                let floor_number = floor.as_deref().map(|value| value.0).unwrap_or(1);
                let minimum = get_floor_enemy_count(&data, floor_number)
                    .saturating_sub(1)
                    .max(2);
                let next = spawn_count.current.saturating_sub(1);
                spawn_count.current = next.max(minimum);
            }
        }
    }
}

fn boss_contact_damage_system(
    mut damage_ev: EventWriter<DamageEvent>,
    boss_q: Query<(Entity, &EnemyStats, &Hurtbox, &GlobalTransform), With<BossArchetype>>,
    player_q: Query<
        (
            Entity,
            &Hurtbox,
            &GlobalTransform,
            Option<&InvincibilityTimer>,
            Option<&GhostState>,
        ),
        (With<Player>, Without<Replicated>),
    >,
) {
    for (boss_entity, boss_stats, boss_hurtbox, boss_tf) in &boss_q {
        let boss_aabb = aabb_from_transform_size(boss_tf, boss_hurtbox.size);
        let boss_pos = boss_tf.translation().truncate();
        for (entity, hurtbox, player_tf, inv, ghost) in &player_q {
            if matches!(ghost, Some(GhostState::Ghost)) {
                continue;
            }
            if inv.is_some_and(|timer| !timer.timer.finished()) {
                continue;
            }
            let player_aabb = aabb_from_transform_size(player_tf, hurtbox.size);
            if !boss_aabb.intersects(player_aabb) {
                continue;
            }

            let player_pos = player_tf.translation().truncate();
            damage_ev.send(DamageEvent {
                target: entity,
                source: Some(boss_entity),
                amount: boss_stats.attack_damage * 0.45,
                knockback: direction_to(boss_pos, player_pos) * 120.0,
                team: Team::Enemy,
                kind: DamageKind::Enemy,
                is_crit: false,
            });
        }
    }
}

fn charger_contact_damage_system(
    mut damage_ev: EventWriter<DamageEvent>,
    data: Res<GameDataRegistry>,
    charger_q: Query<
        (
            Entity,
            &EnemyStats,
            &Hurtbox,
            &GlobalTransform,
            &ChargerState,
        ),
        Without<Replicated>,
    >,
    player_q: Query<
        (
            Entity,
            &Hurtbox,
            &GlobalTransform,
            Option<&InvincibilityTimer>,
            Option<&GhostState>,
        ),
        (With<Player>, Without<Replicated>),
    >,
) {
    for (charger_entity, charger_stats, charger_hurtbox, charger_tf, state) in &charger_q {
        if !matches!(state.phase, ChargerPhase::Charging) {
            continue;
        }
        let charger_aabb = aabb_from_transform_size(charger_tf, charger_hurtbox.size);
        let charger_pos = charger_tf.translation().truncate();
        for (entity, hurtbox, player_tf, inv, ghost) in &player_q {
            if matches!(ghost, Some(GhostState::Ghost)) {
                continue;
            }
            if inv.is_some_and(|timer| !timer.timer.finished()) {
                continue;
            }
            let player_aabb = aabb_from_transform_size(player_tf, hurtbox.size);
            if !charger_aabb.intersects(player_aabb) {
                continue;
            }

            let player_pos = player_tf.translation().truncate();
            damage_ev.send(DamageEvent {
                target: entity,
                source: Some(charger_entity),
                amount: charger_stats.attack_damage
                    * data.enemies.charger_config.contact_damage_mult,
                knockback: direction_to(charger_pos, player_pos)
                    * data.enemies.charger_config.contact_knockback,
                team: Team::Enemy,
                kind: DamageKind::Enemy,
                is_crit: false,
            });
        }
    }
}

fn charger_stun_visual_system(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<crate::core::assets::GameAssets>,
    charger_q: Query<(Entity, &ChargerState, Option<&Children>), Without<Replicated>>,
    visual_q: Query<(), With<ChargerStunVisual>>,
    mut visual_tf_q: Query<(&mut Transform, &mut ChargerStunVisual)>,
) {
    for (entity, state, children) in &charger_q {
        let stunned = matches!(state.phase, ChargerPhase::Stunned);
        let existing: Option<Entity> = children
            .into_iter()
            .flatten()
            .copied()
            .find(|c| visual_q.get(*c).is_ok());

        match (stunned, existing) {
            (true, None) => {
                let visual = commands
                    .spawn((
                        SpatialBundle::from_transform(Transform::from_xyz(0.0, 22.0, 1.5)),
                        ChargerStunVisual::default(),
                        InGameEntity,
                    ))
                    .with_children(|parent| {
                        for i in 0..3 {
                            let base_angle = std::f32::consts::TAU * (i as f32 / 3.0);
                            parent.spawn(SpriteBundle {
                                texture: assets.textures.white.clone(),
                                transform: Transform {
                                    translation: Vec3::new(
                                        base_angle.cos() * 14.0,
                                        base_angle.sin() * 14.0,
                                        0.0,
                                    ),
                                    rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_4),
                                    ..default()
                                },
                                sprite: Sprite {
                                    color: Color::srgba(1.0, 0.92, 0.25, 0.95),
                                    custom_size: Some(Vec2::splat(6.0)),
                                    ..default()
                                },
                                ..default()
                            });
                        }
                    })
                    .id();
                commands.entity(entity).add_child(visual);
            }
            (false, Some(visual)) => {
                safe_despawn_recursive(&mut commands, visual);
            }
            _ => {}
        }
    }

    for (mut tf, mut vis) in &mut visual_tf_q {
        vis.spin += time.delta_seconds() * 6.0;
        tf.rotation = Quat::from_rotation_z(vis.spin);
    }
}

fn charger_windup_visual_system(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<crate::core::assets::GameAssets>,
    charger_q: Query<(Entity, &ChargerState, Option<&Children>), Without<Replicated>>,
    visual_q: Query<(), With<ChargerWindupVisual>>,
    mut visual_update_q: Query<(&mut ChargerWindupVisual, &mut Sprite)>,
) {
    for (entity, state, children) in &charger_q {
        let windup = matches!(state.phase, ChargerPhase::Windup);
        let existing: Option<Entity> = children
            .into_iter()
            .flatten()
            .copied()
            .find(|c| visual_q.get(*c).is_ok());

        match (windup, existing) {
            (true, None) => {
                let dir = if state.dir.length_squared() > f32::EPSILON {
                    state.dir.normalize()
                } else {
                    Vec2::X
                };
                let angle = dir.y.atan2(dir.x);
                let visual = commands
                    .spawn((
                        SpriteBundle {
                            texture: assets.textures.white.clone(),
                            transform: Transform {
                                translation: (dir * 38.0).extend(1.4),
                                rotation: Quat::from_rotation_z(angle),
                                ..default()
                            },
                            sprite: Sprite {
                                color: Color::srgba(1.0, 0.35, 0.28, 0.55),
                                custom_size: Some(Vec2::new(72.0, 8.0)),
                                ..default()
                            },
                            ..default()
                        },
                        ChargerWindupVisual::default(),
                        InGameEntity,
                    ))
                    .id();
                commands.entity(entity).add_child(visual);
            }
            (false, Some(visual)) => {
                safe_despawn_recursive(&mut commands, visual);
            }
            _ => {}
        }
    }

    for (mut vis, mut sprite) in &mut visual_update_q {
        vis.pulse += time.delta_seconds() * 8.0;
        let alpha = 0.35 + 0.35 * vis.pulse.sin().abs();
        sprite.color.set_alpha(alpha);
    }
}

fn clear_enemy_attacks_on_room_clear(
    mut commands: Commands,
    mut room_cleared: EventReader<RoomClearedEvent>,
    enemy_attack_q: Query<(Entity, &Hitbox), (Without<Enemy>, Without<Replicated>)>,
) {
    if room_cleared.read().next().is_none() {
        return;
    }

    for (entity, hitbox) in &enemy_attack_q {
        if hitbox.team == Team::Enemy {
            safe_despawn_recursive(&mut commands, entity);
        }
    }
}

fn scaled_enemy_stats(
    data: &GameDataRegistry,
    stats_cfg: &EnemyStatsConfig,
    enemy_type: EnemyType,
    floor_number: u32,
    floor_multiplier: f32,
) -> EnemyStats {
    let (floor_hp, floor_damage, floor_cooldown, floor_projectile) =
        floor_growth_curve(data, floor_number, floor_multiplier);
    let (type_hp, type_damage, type_cooldown, type_projectile, aggro_bonus) =
        enemy_type_curve(data, enemy_type, floor_number);
    EnemyStats {
        max_hp: stats_cfg.max_hp * floor_hp * type_hp,
        move_speed: stats_cfg.move_speed,
        attack_damage: stats_cfg.attack_damage * floor_damage * type_damage,
        attack_cooldown_s: (stats_cfg.attack_cooldown_s * floor_cooldown * type_cooldown).max(0.40),
        aggro_range: stats_cfg.aggro_range + aggro_bonus,
        attack_range: stats_cfg.attack_range,
        projectile_speed: stats_cfg.projectile_speed * floor_projectile * type_projectile,
    }
}

fn elite_chance_for_floor(
    balance: &crate::data::definitions::GameBalanceConfig,
    floor: u32,
) -> f32 {
    let idx = floor.saturating_sub(1) as usize;
    balance
        .elite_chance_by_floor
        .get(idx)
        .or_else(|| balance.elite_chance_by_floor.last())
        .copied()
        .unwrap_or(balance.elite_chance)
}

fn floor_growth_curve(
    data: &GameDataRegistry,
    floor_number: u32,
    floor_multiplier: f32,
) -> (f32, f32, f32, f32) {
    let base_step = if floor_number > 1 {
        (floor_multiplier - 1.0) / floor_number.saturating_sub(1) as f32
    } else {
        data.balance.difficulty_per_floor
    };
    let idx = floor_number.saturating_sub(1) as usize;
    let curve = data
        .balance
        .floor_growth_curves
        .get(idx)
        .or_else(|| data.balance.floor_growth_curves.last())
        .copied()
        .unwrap_or_default();
    (
        1.0 + base_step * curve.hp,
        1.0 + base_step * curve.damage,
        (1.0 - base_step * curve.cooldown).max(0.5),
        1.0 + base_step * curve.projectile,
    )
}

fn enemy_type_curve(
    data: &GameDataRegistry,
    enemy_type: EnemyType,
    floor_number: u32,
) -> (f32, f32, f32, f32, f32) {
    if floor_number < 3 {
        return (1.0, 1.0, 1.0, 1.0, 0.0);
    }
    if let Some(c) = data
        .balance
        .enemy_type_curves
        .iter()
        .find(|c| c.enemy == enemy_type)
    {
        return (c.hp, c.damage, c.cooldown, c.projectile, c.aggro_bonus);
    }
    // SupportCaster gets a small HP bump at floor 4+ when not configured in RON.
    if enemy_type == EnemyType::SupportCaster && floor_number >= 4 {
        return (1.20, 1.0, 1.0, 1.0, 0.0);
    }
    (1.0, 1.0, 1.0, 1.0, 0.0)
}

pub fn effective_enemy_move_speed(stats: &EnemyStats, buff: Option<&EnemyBuffState>) -> f32 {
    stats.move_speed * buff.map(|value| value.speed_mult).unwrap_or(1.0)
}

fn effective_enemy_attack_cooldown(base_cooldown_s: f32, buff: Option<&EnemyBuffState>) -> f32 {
    let cooldown_mult = buff.map(|value| value.cooldown_mult).unwrap_or(1.0);
    (base_cooldown_s / cooldown_mult.max(0.2)).max(0.28)
}

const ELITE_AFFIXES: [EliteAffix; 6] = [
    EliteAffix::Swift,
    EliteAffix::Splitting,
    EliteAffix::Shielded,
    EliteAffix::Vampiric,
    EliteAffix::Berserk,
    EliteAffix::Teleporting,
];

fn elite_affix_count_for_floor(floor_number: u32) -> usize {
    if floor_number >= 3 { 2 } else { 1 }
}

fn pick_elite_affixes(floor_number: u32, rng: &mut GameRng) -> Vec<EliteAffix> {
    let mut affixes = ELITE_AFFIXES.to_vec();
    rng.shuffle(&mut affixes);
    affixes.truncate(elite_affix_count_for_floor(floor_number));
    affixes
}

fn enemy_color(enemy_type: EnemyType, is_elite: bool) -> Color {
    let (r, g, b) = match enemy_type {
        EnemyType::MeleeChaser => (0.95, 0.45, 0.45),
        EnemyType::Lobber => (0.74, 0.56, 0.96),
        EnemyType::RangedShooter => (0.55, 0.65, 0.95),
        EnemyType::Charger => (0.95, 0.75, 0.25),
        EnemyType::Flanker => (0.96, 0.56, 0.78),
        EnemyType::Sniper => (0.70, 0.82, 1.0),
        EnemyType::SupportCaster => (0.55, 0.95, 0.80),
        EnemyType::Bomber => (0.98, 0.38, 0.22),
        EnemyType::Shielder => (0.36, 0.56, 0.78),
        EnemyType::Summoner => (0.64, 0.38, 0.90),
        EnemyType::Boss => (0.85, 0.25, 0.95),
    };
    if !is_elite || enemy_type == EnemyType::Boss {
        return Color::srgb(r, g, b);
    }

    let tint_strength = 0.34;
    Color::srgb(
        r * (1.0 - tint_strength) + 1.0 * tint_strength,
        g * (1.0 - tint_strength) + 0.84 * tint_strength,
        b * (1.0 - tint_strength) + 0.24 * tint_strength,
    )
}

fn scaled_boss_stats(
    data: &GameDataRegistry,
    archetype: BossArchetype,
    floor_multiplier: f32,
    floor_number: u32,
) -> EnemyStats {
    let scaling = (floor_multiplier - 1.0).max(0.0);
    let config = data.bosses.for_floor(floor_number);
    let s = &data.bosses.scaling;
    let hp_scaling = s.hp_factor_for(archetype);
    EnemyStats {
        max_hp: config.max_hp * (1.0 + scaling * hp_scaling),
        move_speed: config.move_speed * (1.0 + scaling * s.move_speed_per_floor),
        attack_damage: config.contact_damage * (1.0 + scaling * s.damage_per_floor),
        attack_cooldown_s: (s.base_cooldown_s / (1.0 + scaling * s.cooldown_inverse_per_floor))
            .max(s.min_cooldown_s),
        aggro_range: s.aggro_range,
        attack_range: s.attack_range_for(archetype),
        projectile_speed: config.projectile_speed * (1.0 + scaling * s.projectile_speed_per_floor),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase3_elites_gain_fixed_second_affix_from_floor_three() {
        let mut rng = GameRng::default();
        rng.reseed(5);

        assert_eq!(pick_elite_affixes(1, &mut rng).len(), 1);
        assert_eq!(pick_elite_affixes(2, &mut rng).len(), 1);
        assert_eq!(pick_elite_affixes(3, &mut rng).len(), 2);
        assert_eq!(pick_elite_affixes(4, &mut rng).len(), 2);
    }

    #[test]
    fn lobber_uses_phase3_warning_window_and_aoe_radius() {
        assert_eq!(lobber_warning_seconds(), 0.6);
        assert_eq!(lobber_impact_radius(), 54.0);
    }
}
