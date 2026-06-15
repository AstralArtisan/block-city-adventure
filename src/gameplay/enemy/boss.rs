#![allow(dead_code)]

use bevy::prelude::*;

use lightyear::prelude::Replicated;

use crate::constants::{ROOM_HALF_HEIGHT, ROOM_HALF_WIDTH};
use crate::coop::components::{CoopParticipant, GhostState};
use crate::coop::net::{CoopNetConfig, NetMode};
use crate::core::events::{BossPhaseChangeEvent, DamageAppliedEvent, DeathEvent};
use crate::data::definitions::BossFloorConfig;
use crate::data::registry::GameDataRegistry;
use crate::gameplay::combat::components::{DamageKind, Hitbox, Hurtbox, Knockback, Lifetime, Team};
use crate::gameplay::combat::projectiles;
use crate::gameplay::effects::flash::Flash;
use crate::gameplay::effects::screen_shake::ScreenShakeRequest;
use crate::gameplay::enemy::components::{
    BossArchetype, BossCoreShield, BossCycleState, BossDecoy, BossDirectionalDefense,
    BossPatternTimer, BossPhase, BossSubCore, BossSummoned, EnemyKind, EnemyStats, EnemyType,
    GuardianShieldIndicator, ShadowTrail, TideHunterPhase, TideHunterState,
};
use crate::gameplay::map::InGameEntity;
use crate::gameplay::player::components::TeamMarker;
use crate::gameplay::player::components::{DashState, Health, Player, Velocity};
use crate::ui::tutorial::{TutorialFlags, TutorialNotification};
use crate::utils::math::direction_to;

pub fn boss_phase_controller(
    mut phase_events: EventWriter<BossPhaseChangeEvent>,
    data: Res<GameDataRegistry>,
    mut q: Query<(&BossArchetype, &Health, &mut BossPhase), With<BossArchetype>>,
) {
    let Ok((archetype, health, mut phase)) = q.get_single_mut() else {
        return;
    };
    let hp_ratio = if health.max > 0.0 {
        health.current / health.max
    } else {
        0.0
    };
    let thresholds = &boss_config(&data, *archetype).phase_thresholds;
    let new_phase = if thresholds.get(1).is_some_and(|t| hp_ratio <= *t) {
        3
    } else if thresholds.first().is_some_and(|t| hp_ratio <= *t) {
        2
    } else {
        1
    };
    if phase.0 != new_phase {
        phase.0 = new_phase;
        phase_events.send(BossPhaseChangeEvent { phase: new_phase });
    }
}

pub fn boss_attack_patterns(
    mut commands: Commands,
    time: Res<Time>,
    data: Res<GameDataRegistry>,
    assets: Res<crate::core::assets::GameAssets>,
    coop_config: Option<Res<CoopNetConfig>>,
    coop_players: Query<(), With<CoopParticipant>>,
    player_q: Query<(&GlobalTransform, Option<&GhostState>), (With<Player>, Without<Replicated>)>,
    summoned_q: Query<Entity, With<BossSummoned>>,
    mut q: Query<
        (
            &mut Transform,
            &BossArchetype,
            &BossPhase,
            &EnemyStats,
            &mut BossPatternTimer,
            &mut BossCycleState,
        ),
        With<BossArchetype>,
    >,
    mut shake_ev: EventWriter<ScreenShakeRequest>,
) {
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
    let player_positions: Vec<Vec2> = player_q
        .iter()
        .filter_map(|(tf, ghost)| {
            (!matches!(ghost, Some(GhostState::Ghost))).then_some(tf.translation().truncate())
        })
        .collect();
    if player_positions.is_empty() {
        return;
    }
    let Ok((mut boss_tf, archetype, phase, stats, mut timer, mut cycle)) = q.get_single_mut()
    else {
        return;
    };
    let boss_pos = boss_tf.translation.truncate();
    let player_pos = player_positions
        .iter()
        .copied()
        .min_by(|a, b| boss_pos.distance(*a).total_cmp(&boss_pos.distance(*b)))
        .unwrap();
    let dir = direction_to(boss_pos, player_pos);
    let summoned_count = summoned_q.iter().count();

    timer.0.tick(time.delta());
    if !timer.0.finished() {
        return;
    }

    match *archetype {
        BossArchetype::Floor1Guardian => {
            run_floor_1_pattern(
                &mut commands,
                &assets,
                boss_pos,
                dir,
                phase.0,
                stats,
                &mut timer.0,
                &mut shake_ev,
            );
        }
        BossArchetype::MirrorWarden => {
            run_floor_2_pattern(
                &mut commands,
                &assets,
                &mut boss_tf,
                boss_pos,
                dir,
                phase.0,
                stats,
                &mut timer.0,
                &mut cycle,
                &mut shake_ev,
            );
        }
        BossArchetype::TideHunter => {
            run_floor_3_pattern(
                &mut commands,
                &assets,
                &data,
                &mut boss_tf,
                boss_pos,
                dir,
                phase.0,
                stats,
                &mut timer.0,
                &mut cycle,
                summoned_count,
                coop_hp_mult,
                &mut shake_ev,
            );
        }
        BossArchetype::CubeCore => {
            run_floor_4_pattern(
                &mut commands,
                &assets,
                &data,
                boss_pos,
                dir,
                phase.0,
                stats,
                &mut timer.0,
                &mut cycle,
                summoned_count,
                coop_hp_mult,
                &mut shake_ev,
            );
        }
    }

    cycle.step = cycle.step.wrapping_add(1);
}

pub fn spawn_boss_bundle(
    _data: &GameDataRegistry,
    archetype: BossArchetype,
) -> (EnemyKind, BossPhase, BossPatternTimer, BossCycleState) {
    let initial_delay = match archetype {
        BossArchetype::Floor1Guardian => 1.35,
        BossArchetype::MirrorWarden => 1.15,
        BossArchetype::TideHunter => 1.05,
        BossArchetype::CubeCore => 0.95,
    };
    (
        EnemyKind(EnemyType::Boss),
        BossPhase(1),
        BossPatternTimer(Timer::from_seconds(initial_delay, TimerMode::Once)),
        BossCycleState {
            step: 0,
            anchor_index: 0,
            rotation: 0.0,
        },
    )
}

pub fn boss_name(archetype: BossArchetype) -> &'static str {
    match archetype {
        BossArchetype::Floor1Guardian => "Floor1Boss",
        BossArchetype::MirrorWarden => "MirrorWarden",
        BossArchetype::TideHunter => "TideHunter",
        BossArchetype::CubeCore => "CubeCore",
    }
}

pub fn boss_color(archetype: BossArchetype) -> Color {
    match archetype {
        BossArchetype::Floor1Guardian => Color::srgb(0.85, 0.25, 0.95),
        BossArchetype::MirrorWarden => Color::srgb(0.58, 0.82, 1.0),
        BossArchetype::TideHunter => Color::srgb(0.5, 0.2, 0.7),
        BossArchetype::CubeCore => Color::srgb(0.92, 0.20, 0.55),
    }
}

pub fn boss_guardian_facing_system(
    player_q: Query<(&GlobalTransform, Option<&GhostState>), (With<Player>, Without<Replicated>)>,
    mut q: Query<(Entity, &Transform, &mut BossDirectionalDefense), With<BossDirectionalDefense>>,
    mut shield_q: Query<
        &mut Transform,
        (
            With<GuardianShieldIndicator>,
            Without<BossDirectionalDefense>,
        ),
    >,
    children_q: Query<&Children>,
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

    for (boss_entity, tf, mut defense) in &mut q {
        let boss_pos = tf.translation.truncate();
        let player_pos = player_positions
            .iter()
            .copied()
            .min_by(|a, b| boss_pos.distance(*a).total_cmp(&boss_pos.distance(*b)))
            .unwrap();
        let dir = direction_to(boss_pos, player_pos);
        // 慢速转向，给玩家绕背时间窗口（约2-3秒完成转向）
        defense.facing = defense.facing.lerp(dir, 0.012).normalize_or_zero();

        // 更新盾牌子实体位置：始终显示在 facing 方向前方
        if let Ok(children) = children_q.get(boss_entity) {
            for &child in children.iter() {
                if let Ok(mut shield_tf) = shield_q.get_mut(child) {
                    let offset = defense.facing * 40.0;
                    shield_tf.translation.x = offset.x;
                    shield_tf.translation.y = offset.y;
                    // 旋转盾牌使其垂直于 facing 方向
                    let angle = defense.facing.y.atan2(defense.facing.x);
                    shield_tf.rotation = Quat::from_rotation_z(angle);
                }
            }
        }
    }
}

pub fn boss_decoy_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut BossDecoy)>,
) {
    for (entity, mut decoy) in &mut q {
        decoy.lifetime.tick(time.delta());
        if decoy.lifetime.finished() {
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn tide_hunter_state_machine(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<crate::core::assets::GameAssets>,
    player_q: Query<
        (&GlobalTransform, Option<&GhostState>, Option<&Velocity>),
        (With<Player>, Without<Replicated>),
    >,
    mut q: Query<
        (
            &mut Transform,
            &EnemyStats,
            &BossPhase,
            &mut TideHunterState,
            &mut Sprite,
        ),
        With<TideHunterState>,
    >,
) {
    let player_data: Vec<(Vec2, Option<Vec2>)> = player_q
        .iter()
        .filter_map(|(tf, ghost, vel)| {
            (!matches!(ghost, Some(GhostState::Ghost)))
                .then_some((tf.translation().truncate(), vel.map(|value| value.0)))
        })
        .collect();
    if player_data.is_empty() {
        return;
    }

    for (mut tf, stats, boss_phase, mut state, mut sprite) in &mut q {
        apply_tide_hunter_phase_params(*boss_phase, &mut state);

        let pos = tf.translation.truncate();
        let (player_pos, player_vel) = player_data
            .iter()
            .copied()
            .min_by(|(a, _), (b, _)| pos.distance(*a).total_cmp(&pos.distance(*b)))
            .unwrap();
        state.timer.tick(time.delta());

        match state.phase {
            TideHunterPhase::Stalk => {
                if state.timer.finished() {
                    state.phase = TideHunterPhase::Telegraph;
                    state.timer = Timer::from_seconds(0.35, TimerMode::Once);
                    state.timer.reset();
                    state.dashes_remaining = state.dashes_per_cycle;
                    state.dash_start = pos;
                    state.dash_target = pos;
                    state.parry_window_active = false;
                }
            }
            TideHunterPhase::Telegraph => {
                tf.translation.x = state.dash_start.x;
                tf.translation.y = state.dash_start.y;
                let predicted_pos = if boss_phase.0 >= 2 {
                    player_vel.map_or(player_pos, |vel| player_pos + vel * 0.3)
                } else {
                    player_pos
                };
                state.dash_target =
                    compute_tide_hunter_dash_target(state.dash_start, predicted_pos, &state);

                if state.timer.finished() {
                    state.phase = TideHunterPhase::ShadowDash;
                    state.timer = Timer::from_seconds(0.18, TimerMode::Once);
                    state.timer.reset();
                    spawn_shadow_trail_line(
                        &mut commands,
                        &assets,
                        state.dash_start,
                        state.dash_target,
                        state.shadow_duration_s,
                        stats.attack_damage * 1.0,
                    );
                    tf.translation.x = state.dash_target.x;
                    tf.translation.y = state.dash_target.y;
                    state.dashes_remaining = state.dashes_remaining.saturating_sub(1);
                }
            }
            TideHunterPhase::ShadowDash => {
                tf.translation.x = state.dash_target.x;
                tf.translation.y = state.dash_target.y;

                if state.timer.finished() {
                    if state.dashes_remaining > 0 {
                        state.phase = TideHunterPhase::Telegraph;
                        state.timer = Timer::from_seconds(0.2, TimerMode::Once);
                        state.timer.reset();
                        state.dash_start = state.dash_target;
                    } else {
                        state.phase = TideHunterPhase::Reposition;
                        state.timer =
                            Timer::from_seconds(state.reposition_duration_s, TimerMode::Once);
                        state.timer.reset();
                        state.parry_window_active = true;
                        spawn_tide_hunter_reposition_projectiles(
                            &mut commands,
                            &assets,
                            state.dash_target,
                            player_pos,
                            boss_phase.0,
                            stats,
                        );
                    }
                }
            }
            TideHunterPhase::Reposition => {
                tf.translation.x = state.dash_target.x;
                tf.translation.y = state.dash_target.y;

                if state.timer.finished() {
                    state.phase = TideHunterPhase::Stalk;
                    state.timer = Timer::from_seconds(state.stalk_duration_s, TimerMode::Once);
                    state.timer.reset();
                    state.parry_window_active = false;
                }
            }
            TideHunterPhase::Stunned => {
                tf.translation.x = state.dash_target.x;
                tf.translation.y = state.dash_target.y;

                if state.timer.finished() {
                    state.phase = TideHunterPhase::Stalk;
                    state.timer = Timer::from_seconds(state.stalk_duration_s, TimerMode::Once);
                    state.timer.reset();
                    state.parry_window_active = false;
                }
            }
        }

        sprite.color = match state.phase {
            TideHunterPhase::Telegraph => {
                if (time.elapsed_seconds() * 14.0).sin().is_sign_positive() {
                    Color::WHITE
                } else {
                    Color::srgb(0.6, 0.3, 0.8)
                }
            }
            TideHunterPhase::ShadowDash => boss_color(BossArchetype::TideHunter).with_alpha(0.4),
            TideHunterPhase::Reposition => Color::srgb(0.6, 0.3, 0.8),
            TideHunterPhase::Stunned => Color::srgb(0.82, 0.82, 0.82),
            TideHunterPhase::Stalk => boss_color(BossArchetype::TideHunter),
        };
    }
}

pub fn tide_hunter_contact_damage_system(
    time: Res<Time>,
    mut tide_q: Query<(Entity, &GlobalTransform, &EnemyStats, &mut TideHunterState)>,
    mut player_q: Query<
        (Entity, &GlobalTransform, &mut Health),
        (With<Player>, Without<Replicated>),
    >,
    mut damage_events: EventWriter<DamageAppliedEvent>,
    mut death_events: EventWriter<DeathEvent>,
) {
    for (boss_entity, boss_tf, stats, mut state) in &mut tide_q {
        state.contact_hit_cooldown.tick(time.delta());
        if state.phase != TideHunterPhase::ShadowDash {
            continue;
        }

        if !state.contact_hit_cooldown.finished() {
            continue;
        }

        let boss_pos = boss_tf.translation().truncate();
        let contact_damage = stats.attack_damage * 0.5;
        for (player_entity, player_tf, mut health) in &mut player_q {
            let player_pos = player_tf.translation().truncate();
            if boss_pos.distance(player_pos) >= 30.0 {
                continue;
            }

            let previous = health.current;
            health.current = (health.current - contact_damage).max(0.0);
            state.contact_hit_cooldown = Timer::from_seconds(0.3, TimerMode::Once);

            damage_events.send(DamageAppliedEvent {
                target: player_entity,
                source: Some(boss_entity),
                amount: contact_damage,
                attacker_team: Team::Enemy,
                kind: DamageKind::Enemy,
                target_team: Some(Team::Player),
                is_crit: false,
                pos: player_pos,
            });

            if previous > 0.0 && health.current <= 0.0 {
                death_events.send(DeathEvent {
                    entity: player_entity,
                    source: Some(boss_entity),
                    team: Team::Player,
                });
            }
            break;
        }
    }
}

pub fn tide_hunter_parry_check(
    player_q: Query<
        (&GlobalTransform, &DashState, Option<&GhostState>),
        (With<Player>, Without<Replicated>),
    >,
    mut boss_q: Query<
        (
            &Transform,
            &mut TideHunterState,
            &mut Sprite,
            Option<&mut Flash>,
        ),
        With<TideHunterState>,
    >,
) {
    for (boss_tf, mut state, mut sprite, flash_opt) in &mut boss_q {
        if state.phase != TideHunterPhase::Reposition || !state.parry_window_active {
            continue;
        }
        let boss_pos = boss_tf.translation.truncate();
        for (player_tf, dash, ghost) in &player_q {
            if matches!(ghost, Some(GhostState::Ghost)) {
                continue;
            }
            let player_pos = player_tf.translation().truncate();
            if dash.active && boss_pos.distance(player_pos) < 60.0 {
                state.phase = TideHunterPhase::Stunned;
                state.timer = Timer::from_seconds(1.4, TimerMode::Once);
                state.timer.reset();
                state.parry_window_active = false;
                state.dash_start = boss_pos;
                state.dash_target = boss_pos;
                sprite.color = Color::srgb(0.82, 0.82, 0.82);
                if let Some(mut flash) = flash_opt {
                    flash.trigger(1.4);
                }
                break;
            }
        }
    }
}

pub fn shadow_trail_fade_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut ShadowTrail, &mut Sprite)>,
) {
    for (entity, mut trail, mut sprite) in &mut q {
        trail.lifetime.tick(time.delta());
        sprite
            .color
            .set_alpha(0.7 * (1.0 - trail.lifetime.fraction()));
        if trail.lifetime.finished() {
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn shadow_trail_damage_system(
    time: Res<Time>,
    mut death_events: EventWriter<DeathEvent>,
    trail_q: Query<(&Transform, &ShadowTrail)>,
    mut player_q: Query<
        (
            Entity,
            &GlobalTransform,
            &mut Health,
            &DashState,
            Option<&GhostState>,
        ),
        (With<Player>, Without<Replicated>),
    >,
) {
    for (entity, player_tf, mut health, dash, ghost) in &mut player_q {
        if matches!(ghost, Some(GhostState::Ghost)) || dash.active {
            continue;
        }

        let player_pos = player_tf.translation().truncate();
        for (trail_tf, trail) in &trail_q {
            let trail_pos = trail_tf.translation.truncate();
            if player_pos.distance(trail_pos) < trail.radius {
                let previous = health.current;
                health.current = (health.current - trail.damage * time.delta_seconds()).max(0.0);
                if previous > 0.0 && health.current <= 0.0 {
                    death_events.send(DeathEvent {
                        entity,
                        source: None,
                        team: Team::Player,
                    });
                }
                break;
            }
        }
    }
}

pub fn boss_subcore_orbit(
    time: Res<Time>,
    boss_q: Query<&Transform, (With<BossArchetype>, Without<BossSubCore>)>,
    mut core_q: Query<(&mut BossSubCore, &mut Transform), Without<BossArchetype>>,
) {
    for (mut core, mut tf) in &mut core_q {
        let Ok(boss_tf) = boss_q.get(core.boss_entity) else {
            continue;
        };
        core.orbit_angle += core.orbit_speed * time.delta_seconds();
        let boss_pos = boss_tf.translation.truncate();
        let new_pos = boss_pos + Vec2::new(core.orbit_angle.cos(), core.orbit_angle.sin()) * 85.0;
        tf.translation.x = new_pos.x;
        tf.translation.y = new_pos.y;
    }
}

pub fn boss_core_shield_update(
    core_q: Query<&BossSubCore>,
    mut boss_q: Query<(Entity, &mut BossCoreShield, &mut Sprite, Option<&mut Flash>)>,
) {
    for (boss_entity, mut shield, mut sprite, flash_opt) in &mut boss_q {
        let alive = core_q
            .iter()
            .filter(|core| core.boss_entity == boss_entity)
            .count() as u8;
        if alive < shield.cores_alive
            && shield.cores_alive > 0
            && let Some(mut flash) = flash_opt
        {
            flash.trigger(0.25);
        }
        shield.cores_alive = alive;
        sprite.color = if alive > 0 {
            Color::srgba(0.60, 0.42, 0.50, 0.92)
        } else {
            boss_color(BossArchetype::CubeCore)
        };
    }
}

pub fn boss_core_phase_respawn(
    mut commands: Commands,
    assets: Res<crate::core::assets::GameAssets>,
    data: Res<crate::data::registry::GameDataRegistry>,
    mut phase_events: EventReader<BossPhaseChangeEvent>,
    boss_q: Query<(Entity, &Transform, &BossArchetype)>,
) {
    for ev in phase_events.read() {
        if ev.phase < 2 {
            continue;
        }
        for (boss_entity, boss_tf, archetype) in &boss_q {
            if *archetype != BossArchetype::CubeCore {
                continue;
            }
            let boss_pos = boss_tf.translation.truncate();
            let count = 2u8;
            for i in 0..count {
                let angle = i as f32 / count as f32 * std::f32::consts::TAU;
                let spawn_pos = boss_pos + Vec2::new(angle.cos(), angle.sin()) * 85.0;
                let core_hp = data.bosses.sub_core_base_hp
                    + ev.phase as f32 * data.bosses.sub_core_hp_per_phase;
                spawn_cube_core_subcore(
                    &mut commands,
                    &assets,
                    boss_entity,
                    spawn_pos,
                    angle,
                    0.65,
                    core_hp,
                );
            }
        }
    }
}

pub fn boss_mechanic_hint_system(
    mut phase_events: EventReader<BossPhaseChangeEvent>,
    mut flags: ResMut<TutorialFlags>,
    mut tutorial_ev: EventWriter<TutorialNotification>,
    decoy_q: Query<(), With<BossDecoy>>,
    tide_q: Query<&TideHunterState>,
) {
    let _phase_changed = phase_events.read().last().is_some();

    if !flags.boss_mirror_warden_mechanic_shown && !decoy_q.is_empty() {
        tutorial_ev.send(TutorialNotification(
            "找到真身！幻象血量较低，会很快消散".to_string(),
        ));
        flags.boss_mirror_warden_mechanic_shown = true;
    }

    if !flags.boss_tide_hunter_mechanic_shown
        && tide_q
            .iter()
            .any(|state| state.phase == TideHunterPhase::Reposition)
    {
        tutorial_ev.send(TutorialNotification(
            "破绽出现！停顿时用【空格】冲刺贴近反制".to_string(),
        ));
        flags.boss_tide_hunter_mechanic_shown = true;
    }
}

fn apply_tide_hunter_phase_params(boss_phase: BossPhase, state: &mut TideHunterState) {
    match boss_phase.0 {
        1 => {
            state.dashes_per_cycle = 1;
            state.shadow_duration_s = 2.5;
            state.stalk_duration_s = 1.2;
            state.reposition_duration_s = 0.9;
        }
        2 => {
            state.dashes_per_cycle = 2;
            state.shadow_duration_s = 3.5;
            state.stalk_duration_s = 0.8;
            state.reposition_duration_s = 0.7;
        }
        _ => {
            state.dashes_per_cycle = 3;
            state.shadow_duration_s = 6.0;
            state.stalk_duration_s = 0.5;
            state.reposition_duration_s = 0.6;
        }
    }
}

fn compute_tide_hunter_dash_target(
    dash_start: Vec2,
    player_pos: Vec2,
    state: &TideHunterState,
) -> Vec2 {
    let toward_player = direction_to(dash_start, player_pos);
    let dash_dir = if toward_player.length_squared() > f32::EPSILON {
        toward_player
    } else {
        Vec2::X
    };
    let lateral = Vec2::new(-dash_dir.y, dash_dir.x);
    let dash_index = state
        .dashes_per_cycle
        .saturating_sub(state.dashes_remaining);

    let target = match state.dashes_per_cycle {
        1 => player_pos + dash_dir * 80.0,
        2 => {
            let side_sign = if dash_index == 0 { 1.0 } else { -1.0 };
            player_pos + lateral * side_sign * 100.0
        }
        _ => {
            let base_angle = (dash_start.y - player_pos.y).atan2(dash_start.x - player_pos.x);
            let angle = base_angle + dash_index as f32 * std::f32::consts::TAU / 3.0;
            player_pos + Vec2::new(angle.cos(), angle.sin()) * 100.0
        }
    };

    clamp_room_position(target, 36.0)
}

fn spawn_shadow_trail_line(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    start: Vec2,
    end: Vec2,
    shadow_duration_s: f32,
    damage: f32,
) {
    let delta = end - start;
    let distance = delta.length();
    let dir = delta.normalize_or_zero();
    let steps = (distance / 25.0).ceil().max(1.0) as u32;

    for index in 0..=steps {
        let travel = (index as f32 * 25.0).min(distance);
        let trail_pos = start + dir * travel;
        commands.spawn((
            SpriteBundle {
                texture: assets.textures.white.clone(),
                transform: Transform::from_translation(trail_pos.extend(0.5)),
                sprite: Sprite {
                    color: Color::srgba(0.4, 0.1, 0.6, 0.7),
                    custom_size: Some(Vec2::splat(20.0)),
                    ..default()
                },
                ..default()
            },
            ShadowTrail {
                lifetime: Timer::from_seconds(shadow_duration_s, TimerMode::Once),
                damage,
                radius: 20.0,
            },
            InGameEntity,
            Name::new("ShadowTrail"),
        ));
    }
}

fn spawn_tide_hunter_reposition_projectiles(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    boss_pos: Vec2,
    player_pos: Vec2,
    boss_phase: u8,
    stats: &EnemyStats,
) {
    if boss_phase < 2 {
        return;
    }

    let dir = direction_to(boss_pos, player_pos);
    let projectile_speed = stats.projectile_speed * 0.65;
    let damage = stats.attack_damage * 0.4;
    let origin = boss_pos + dir * 18.0;

    if boss_phase == 2 {
        projectiles::spawn_projectile(
            commands,
            assets,
            Team::Enemy,
            origin,
            dir * projectile_speed,
            damage,
        );
        return;
    }

    for angle in [-15.0_f32.to_radians(), 0.0, 15.0_f32.to_radians()] {
        let shot_dir = Mat2::from_angle(angle).mul_vec2(dir);
        projectiles::spawn_projectile(
            commands,
            assets,
            Team::Enemy,
            origin,
            shot_dir * projectile_speed,
            damage,
        );
    }
}

fn boss_config(data: &GameDataRegistry, archetype: BossArchetype) -> &BossFloorConfig {
    match archetype {
        BossArchetype::Floor1Guardian => &data.bosses.floor_1,
        BossArchetype::MirrorWarden => &data.bosses.floor_2,
        BossArchetype::TideHunter => &data.bosses.floor_3,
        BossArchetype::CubeCore => &data.bosses.floor_4,
    }
}

fn run_floor_1_pattern(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    boss_pos: Vec2,
    dir: Vec2,
    phase: u8,
    stats: &EnemyStats,
    timer: &mut Timer,
    shake_ev: &mut EventWriter<ScreenShakeRequest>,
) {
    match phase {
        1 => {
            *timer = Timer::from_seconds(1.35, TimerMode::Once);
            timer.reset();
            spawn_fan(
                commands,
                assets,
                boss_pos + dir * 24.0,
                dir,
                stats.projectile_speed,
                stats.attack_damage * 0.55,
                &[-0.28, 0.0, 0.28],
            );
        }
        2 => {
            *timer = Timer::from_seconds(1.50, TimerMode::Once);
            timer.reset();
            spawn_ring(
                commands,
                assets,
                boss_pos,
                stats.projectile_speed * 0.72,
                stats.attack_damage * 0.42,
                8,
                0.0,
            );
            shake_ev.send(ScreenShakeRequest {
                strength: 4.0,
                duration: 0.12,
            });
        }
        _ => {
            *timer = Timer::from_seconds(0.34, TimerMode::Once);
            timer.reset();
            spawn_fan(
                commands,
                assets,
                boss_pos + dir * 24.0,
                dir,
                stats.projectile_speed * 1.08,
                stats.attack_damage * 0.40,
                &[-0.14, 0.0, 0.14],
            );
            shake_ev.send(ScreenShakeRequest {
                strength: 6.0,
                duration: 0.14,
            });
        }
    }
}

fn run_floor_2_pattern(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    boss_tf: &mut Transform,
    boss_pos: Vec2,
    dir: Vec2,
    phase: u8,
    stats: &EnemyStats,
    timer: &mut Timer,
    cycle: &mut BossCycleState,
    shake_ev: &mut EventWriter<ScreenShakeRequest>,
) {
    match phase {
        1 => {
            *timer = Timer::from_seconds(1.18, TimerMode::Once);
            timer.reset();
            let angles: &[f32] = if cycle.step.is_multiple_of(2) {
                &[-0.26, 0.0, 0.26]
            } else {
                &[-0.44, -0.18, 0.0, 0.18, 0.44]
            };
            spawn_fan(
                commands,
                assets,
                boss_pos + dir * 20.0,
                dir,
                stats.projectile_speed,
                stats.attack_damage * 0.48,
                angles,
            );
        }
        2 => {
            *timer = Timer::from_seconds(0.96, TimerMode::Once);
            timer.reset();
            let anchor = mirror_anchor(cycle.anchor_index);
            cycle.anchor_index = (cycle.anchor_index + 1) % 3;
            spawn_mirror_decoy(commands, assets, boss_pos, stats, dir, phase);
            boss_tf.translation.x = anchor.x;
            boss_tf.translation.y = anchor.y;
            if cycle.step.is_multiple_of(2) {
                spawn_cross(
                    commands,
                    assets,
                    anchor,
                    stats.projectile_speed * 0.86,
                    stats.attack_damage * 0.45,
                );
            } else {
                let anchor_dir = direction_to(anchor, anchor + dir);
                spawn_fan(
                    commands,
                    assets,
                    anchor + anchor_dir * 20.0,
                    anchor_dir,
                    stats.projectile_speed * 1.06,
                    stats.attack_damage * 0.50,
                    &[-0.12, 0.0, 0.12],
                );
            }
            shake_ev.send(ScreenShakeRequest {
                strength: 5.0,
                duration: 0.10,
            });
        }
        _ => {
            *timer = Timer::from_seconds(0.84, TimerMode::Once);
            timer.reset();
            let anchor = mirror_anchor(cycle.anchor_index);
            cycle.anchor_index = (cycle.anchor_index + 1) % 3;
            spawn_mirror_decoy(commands, assets, boss_pos, stats, dir, phase);
            boss_tf.translation.x = anchor.x;
            boss_tf.translation.y = anchor.y;
            spawn_fan(
                commands,
                assets,
                anchor + dir * 18.0,
                dir,
                stats.projectile_speed * 1.08,
                stats.attack_damage * 0.46,
                &[-0.20, -0.08, 0.0, 0.08, 0.20],
            );
            spawn_ring(
                commands,
                assets,
                anchor,
                stats.projectile_speed * 0.72,
                stats.attack_damage * 0.24,
                10,
                cycle.rotation,
            );
            cycle.rotation += 0.18;
            shake_ev.send(ScreenShakeRequest {
                strength: 6.5,
                duration: 0.12,
            });
        }
    }
}

fn run_floor_3_pattern(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    data: &GameDataRegistry,
    boss_tf: &mut Transform,
    boss_pos: Vec2,
    dir: Vec2,
    phase: u8,
    stats: &EnemyStats,
    timer: &mut Timer,
    cycle: &mut BossCycleState,
    summoned_count: usize,
    coop_hp_mult: f32,
    shake_ev: &mut EventWriter<ScreenShakeRequest>,
) {
    match phase {
        1 => {
            *timer = Timer::from_seconds(2.0, TimerMode::Once);
            timer.reset();
        }
        2 => {
            *timer = Timer::from_seconds(1.08, TimerMode::Once);
            timer.reset();
            if cycle.step.is_multiple_of(2) {
                perform_charge_burst(commands, assets, boss_tf, boss_pos, dir, stats, 160.0);
                shake_ev.send(ScreenShakeRequest {
                    strength: 7.0,
                    duration: 0.16,
                });
            } else if summoned_count < 1 {
                summon_boss_enemy(
                    commands,
                    assets,
                    data,
                    EnemyType::Flanker,
                    false,
                    boss_pos,
                    coop_hp_mult,
                );
            } else {
                spawn_fan(
                    commands,
                    assets,
                    boss_pos + dir * 18.0,
                    dir,
                    stats.projectile_speed,
                    stats.attack_damage * 0.42,
                    &[-0.18, 0.0, 0.18],
                );
            }
        }
        _ => {
            *timer = Timer::from_seconds(0.92, TimerMode::Once);
            timer.reset();
            if cycle.step.is_multiple_of(2) {
                perform_charge_burst(commands, assets, boss_tf, boss_pos, dir, stats, 195.0);
                spawn_ring(
                    commands,
                    assets,
                    boss_tf.translation.truncate(),
                    stats.projectile_speed * 0.58,
                    stats.attack_damage * 0.20,
                    6,
                    0.0,
                );
            } else if summoned_count < 2 {
                let summon_type = if cycle.step % 4 == 1 {
                    EnemyType::Sniper
                } else {
                    EnemyType::Flanker
                };
                summon_boss_enemy(
                    commands,
                    assets,
                    data,
                    summon_type,
                    false,
                    boss_pos,
                    coop_hp_mult,
                );
            } else {
                spawn_fan(
                    commands,
                    assets,
                    boss_pos + dir * 18.0,
                    dir,
                    stats.projectile_speed * 1.05,
                    stats.attack_damage * 0.46,
                    &[-0.26, -0.08, 0.0, 0.08, 0.26],
                );
            }
            shake_ev.send(ScreenShakeRequest {
                strength: 6.0,
                duration: 0.12,
            });
        }
    }
}

fn run_floor_4_pattern(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    data: &GameDataRegistry,
    boss_pos: Vec2,
    dir: Vec2,
    phase: u8,
    stats: &EnemyStats,
    timer: &mut Timer,
    cycle: &mut BossCycleState,
    summoned_count: usize,
    coop_hp_mult: f32,
    shake_ev: &mut EventWriter<ScreenShakeRequest>,
) {
    match phase {
        1 => {
            *timer = Timer::from_seconds(0.78, TimerMode::Once);
            timer.reset();
            spawn_spiral(
                commands,
                assets,
                boss_pos,
                stats.projectile_speed * 0.70,
                stats.attack_damage * 0.26,
                &mut cycle.rotation,
            );
            if cycle.step.is_multiple_of(2) {
                spawn_aimed_shot(commands, assets, boss_pos, dir, stats, 0.54, 1.12);
            }
        }
        2 => {
            *timer = Timer::from_seconds(1.02, TimerMode::Once);
            timer.reset();
            spawn_bullet_wall(commands, assets, stats, cycle.step, 8);
            spawn_aimed_shot(commands, assets, boss_pos, dir, stats, 0.44, 1.08);
            shake_ev.send(ScreenShakeRequest {
                strength: 7.0,
                duration: 0.14,
            });
        }
        _ => {
            let cycle_mode = cycle.step % 3;
            *timer = Timer::from_seconds(0.92, TimerMode::Once);
            timer.reset();
            match cycle_mode {
                0 => {
                    spawn_spiral(
                        commands,
                        assets,
                        boss_pos,
                        stats.projectile_speed * 0.76,
                        stats.attack_damage * 0.28,
                        &mut cycle.rotation,
                    );
                    spawn_aimed_shot(commands, assets, boss_pos, dir, stats, 0.48, 1.12);
                }
                1 => {
                    spawn_bullet_wall(commands, assets, stats, cycle.step, 10);
                }
                _ => {
                    if summoned_count < 2 {
                        let summon_type = if cycle.step.is_multiple_of(2) {
                            EnemyType::Charger
                        } else {
                            EnemyType::Flanker
                        };
                        summon_boss_enemy(
                            commands,
                            assets,
                            data,
                            summon_type,
                            true,
                            boss_pos,
                            coop_hp_mult,
                        );
                    } else {
                        spawn_ring(
                            commands,
                            assets,
                            boss_pos,
                            stats.projectile_speed * 0.62,
                            stats.attack_damage * 0.22,
                            8,
                            cycle.rotation,
                        );
                    }
                }
            }
            shake_ev.send(ScreenShakeRequest {
                strength: 7.5,
                duration: 0.16,
            });
        }
    }
}

fn spawn_mirror_decoy(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    pos: Vec2,
    stats: &EnemyStats,
    dir: Vec2,
    phase: u8,
) {
    commands.spawn((
        SpriteBundle {
            texture: assets.textures.white.clone(),
            transform: Transform::from_translation(pos.extend(44.0)),
            sprite: Sprite {
                color: Color::srgba(0.58, 0.82, 1.0, 0.45),
                custom_size: Some(Vec2::splat(60.0)),
                ..default()
            },
            ..default()
        },
        Health {
            current: mirror_decoy_max_hp(stats),
            max: mirror_decoy_max_hp(stats),
        },
        Hurtbox {
            team: Team::Enemy,
            size: Vec2::splat(52.0),
        },
        Knockback(Vec2::ZERO),
        Flash::new(0.0),
        BossDecoy {
            lifetime: Timer::from_seconds(2.8, TimerMode::Once),
        },
        InGameEntity,
        Name::new("MirrorDecoy"),
    ));
    spawn_cross(
        commands,
        assets,
        pos,
        stats.projectile_speed * 0.75,
        stats.attack_damage * 0.35,
    );
    if phase >= 3 {
        spawn_fan(
            commands,
            assets,
            pos + dir * 16.0,
            dir,
            stats.projectile_speed * 0.9,
            stats.attack_damage * 0.3,
            &[-0.22, 0.0, 0.22],
        );
    }
}

fn mirror_decoy_max_hp(stats: &EnemyStats) -> f32 {
    stats.max_hp * 0.20
}

#[allow(dead_code)]
fn spawn_melee_hitbox(
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
            transform: Transform::from_translation((pos + dir * 30.0).extend(40.0)),
            sprite: Sprite {
                color: Color::NONE,
                custom_size: Some(Vec2::splat(38.0)),
                ..default()
            },
            ..default()
        },
        Hitbox {
            owner: Some(owner),
            team: Team::Enemy,
            damage_kind: DamageKind::Enemy,
            size: Vec2::splat(38.0),
            damage,
            knockback: 260.0,
            can_crit: false,
            crit_chance: 0.0,
            crit_multiplier: 1.0,
        },
        Lifetime(Timer::from_seconds(0.10, TimerMode::Once)),
        InGameEntity,
        Name::new("TideHunterLungeHitbox"),
    ));
}

pub fn spawn_cube_core_subcore(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    boss_entity: Entity,
    spawn_pos: Vec2,
    orbit_angle: f32,
    orbit_speed: f32,
    core_hp: f32,
) {
    commands.spawn((
        SpriteBundle {
            texture: assets.textures.white.clone(),
            transform: Transform::from_translation(spawn_pos.extend(44.0)),
            sprite: Sprite {
                color: Color::srgb(1.0, 0.55, 0.75),
                custom_size: Some(Vec2::splat(18.0)),
                ..default()
            },
            ..default()
        },
        BossSubCore {
            boss_entity,
            orbit_angle,
            orbit_speed,
        },
        Health {
            current: core_hp,
            max: core_hp,
        },
        EnemyKind(EnemyType::Boss),
        TeamMarker(Team::Enemy),
        Hurtbox {
            team: Team::Enemy,
            size: Vec2::splat(16.0),
        },
        Flash::new(0.0),
        Knockback(Vec2::ZERO),
        InGameEntity,
        Name::new("CubeCoreSubCore"),
    ));
}

fn spawn_fan(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    origin: Vec2,
    dir: Vec2,
    projectile_speed: f32,
    damage: f32,
    angles: &[f32],
) {
    for angle in angles {
        let rot = Mat2::from_angle(*angle);
        projectiles::spawn_projectile(
            commands,
            assets,
            Team::Enemy,
            origin,
            rot.mul_vec2(dir) * projectile_speed,
            damage,
        );
    }
}

fn spawn_ring(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    origin: Vec2,
    projectile_speed: f32,
    damage: f32,
    count: u32,
    start_angle: f32,
) {
    for i in 0..count {
        let angle = start_angle + i as f32 / count as f32 * std::f32::consts::TAU;
        let dir = Vec2::new(angle.cos(), angle.sin());
        projectiles::spawn_projectile(
            commands,
            assets,
            Team::Enemy,
            origin,
            dir * projectile_speed,
            damage,
        );
    }
}

fn spawn_cross(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    origin: Vec2,
    projectile_speed: f32,
    damage: f32,
) {
    for dir in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
        projectiles::spawn_projectile(
            commands,
            assets,
            Team::Enemy,
            origin,
            dir * projectile_speed,
            damage,
        );
    }
}

fn spawn_spiral(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    origin: Vec2,
    projectile_speed: f32,
    damage: f32,
    rotation: &mut f32,
) {
    for i in 0..7 {
        let angle = *rotation + i as f32 * 0.58;
        let dir = Vec2::new(angle.cos(), angle.sin());
        projectiles::spawn_projectile(
            commands,
            assets,
            Team::Enemy,
            origin,
            dir * projectile_speed,
            damage,
        );
    }
    *rotation += 0.38;
}

fn spawn_aimed_shot(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    origin: Vec2,
    dir: Vec2,
    stats: &EnemyStats,
    damage_mult: f32,
    speed_mult: f32,
) {
    projectiles::spawn_projectile(
        commands,
        assets,
        Team::Enemy,
        origin + dir * 20.0,
        dir * stats.projectile_speed * speed_mult,
        stats.attack_damage * damage_mult,
    );
}

fn perform_charge_burst(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    boss_tf: &mut Transform,
    boss_pos: Vec2,
    dir: Vec2,
    stats: &EnemyStats,
    distance: f32,
) {
    let new_pos = clamp_room_position(boss_pos + dir * distance, 52.0);
    boss_tf.translation.x = new_pos.x;
    boss_tf.translation.y = new_pos.y;
    spawn_fan(
        commands,
        assets,
        new_pos,
        dir,
        stats.projectile_speed * 0.70,
        stats.attack_damage * 0.30,
        &[-0.24, 0.0, 0.24],
    );
}

fn spawn_bullet_wall(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    stats: &EnemyStats,
    step: u8,
    lanes: u32,
) {
    let gap = (step as u32 % lanes).min(lanes.saturating_sub(1));
    let vertical = step.is_multiple_of(2);
    for lane in 0..lanes {
        if lane == gap {
            continue;
        }
        let t = if lanes <= 1 {
            0.5
        } else {
            lane as f32 / (lanes - 1) as f32
        };
        let (origin, velocity) = if vertical {
            let x = -ROOM_HALF_WIDTH + t * ROOM_HALF_WIDTH * 2.0;
            (
                Vec2::new(x, ROOM_HALF_HEIGHT + 40.0),
                -Vec2::Y * stats.projectile_speed * 0.82,
            )
        } else {
            let y = -ROOM_HALF_HEIGHT + t * ROOM_HALF_HEIGHT * 2.0;
            (
                Vec2::new(ROOM_HALF_WIDTH + 40.0, y),
                -Vec2::X * stats.projectile_speed * 0.82,
            )
        };
        projectiles::spawn_projectile(
            commands,
            assets,
            Team::Enemy,
            origin,
            velocity,
            stats.attack_damage * 0.22,
        );
    }
}

fn summon_boss_enemy(
    commands: &mut Commands,
    assets: &crate::core::assets::GameAssets,
    data: &GameDataRegistry,
    enemy_type: EnemyType,
    elite: bool,
    boss_pos: Vec2,
    coop_hp_mult: f32,
) {
    let floor_number: u32 = match enemy_type {
        EnemyType::Sniper | EnemyType::SupportCaster => 4,
        _ => 3,
    };
    let floor_multiplier =
        1.0 + (floor_number.saturating_sub(1) as f32) * data.balance.difficulty_per_floor;
    let offset = if boss_pos.x > 0.0 {
        Vec2::new(-120.0, 90.0)
    } else {
        Vec2::new(120.0, -90.0)
    };
    let summon_pos = clamp_room_position(boss_pos + offset, 32.0);
    let summoned = super::systems::spawn_enemy(
        commands,
        assets,
        data,
        enemy_type,
        summon_pos,
        floor_number,
        floor_multiplier,
        coop_hp_mult,
        elite && enemy_type != EnemyType::SupportCaster,
    );
    commands.entity(summoned).insert(BossSummoned);
}

fn mirror_anchor(index: usize) -> Vec2 {
    match index % 3 {
        0 => Vec2::new(-220.0, 140.0),
        1 => Vec2::new(220.0, -120.0),
        _ => Vec2::new(120.0, 165.0),
    }
}

fn clamp_room_position(pos: Vec2, margin: f32) -> Vec2 {
    Vec2::new(
        pos.x
            .clamp(-ROOM_HALF_WIDTH + margin, ROOM_HALF_WIDTH - margin),
        pos.y
            .clamp(-ROOM_HALF_HEIGHT + margin, ROOM_HALF_HEIGHT - margin),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_decoy_uses_phase3_twenty_percent_hp() {
        let stats = EnemyStats {
            max_hp: 500.0,
            move_speed: 0.0,
            attack_damage: 0.0,
            attack_cooldown_s: 0.0,
            aggro_range: 0.0,
            attack_range: 0.0,
            projectile_speed: 0.0,
        };

        assert_eq!(mirror_decoy_max_hp(&stats), 100.0);
    }
}
