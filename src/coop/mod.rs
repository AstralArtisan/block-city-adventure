pub mod components;
pub mod net;
pub mod runtime;
pub mod ui;

use bevy::prelude::*;

use crate::states::AppState;

pub struct CoopPlugin;
pub struct HeadlessCoopPlugin;

impl Plugin for CoopPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((net::CoopLightyearPlugin, runtime::CoopRuntimePlugin))
            .add_systems(OnEnter(AppState::CoopMenu), ui::setup_coop_menu)
            .add_systems(
                Update,
                ui::coop_menu_input_system.run_if(in_state(AppState::CoopMenu)),
            )
            .add_systems(OnExit(AppState::CoopMenu), ui::cleanup_coop_menu)
            .add_systems(OnEnter(AppState::CoopLobby), ui::setup_coop_lobby)
            .add_systems(
                Update,
                (ui::coop_lobby_ui_system, ui::coop_lobby_input_system)
                    .run_if(in_state(AppState::CoopLobby)),
            )
            .add_systems(OnExit(AppState::CoopLobby), ui::cleanup_coop_lobby)
            .add_systems(OnEnter(AppState::CoopGame), ui::setup_coop_game_ui)
            .add_systems(
                Update,
                ui::ensure_local_control_marker
                    .run_if(in_state(AppState::CoopLobby).or_else(in_state(AppState::CoopGame))),
            )
            .add_systems(
                Update,
                (
                    ui::sync_host_authority_visibility,
                    ui::attach_replicated_visuals,
                    ui::filter_replicated_player_duplicates
                        .after(ui::attach_replicated_visuals)
                        .before(ui::sync_replicated_visuals),
                    ui::predict_local_player_animation.before(ui::sync_replicated_visuals),
                    ui::sync_replicated_visuals,
                    ui::update_replicated_door_visuals,
                    ui::update_remote_health_bars,
                    ui::update_coop_overlay,
                    ui::handle_coop_overlay_input,
                    ui::client_receive_damage_events,
                )
                    .run_if(in_state(AppState::CoopGame)),
            )
            .add_systems(OnExit(AppState::CoopGame), ui::cleanup_coop_game_ui)
            .add_systems(OnEnter(AppState::MainMenu), runtime::reset_coop_runtime);
    }
}

impl Plugin for HeadlessCoopPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((net::CoopLightyearPlugin, runtime::CoopRuntimePlugin))
            .add_systems(OnEnter(AppState::MainMenu), runtime::reset_coop_runtime);
    }
}
