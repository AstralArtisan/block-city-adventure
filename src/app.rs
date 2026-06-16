use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

use crate::coop::{CoopPlugin, HeadlessCoopPlugin};
use crate::core::{
    achievements::AchievementsPlugin,
    assets::{AssetsPlugin, PlaceholderAssetsPlugin},
    audio::AudioPlugin,
    camera::CameraPlugin,
    events::EventsPlugin,
    input::{InputPlugin, PlayerInputState},
    local_debug::LocalDebugPlugin,
    save::SavePlugin,
};
use crate::data::{DataPlugin, HeadlessDataPlugin};
use crate::gameplay::GameplayPlugin;
use crate::pvp::net::{PvpNetConfig, PvpNetState};
use crate::pvp::{HeadlessPvpPlugin, PvpPlugin};
use crate::states::{AppState, GamePhase};
use crate::ui::{HeadlessUiSupportPlugin, UiPlugin};

pub struct GamePlugin;
pub struct DedicatedServerPlugin;

fn configure_shared_game(app: &mut App, initial_state: AppState) -> &mut App {
    app.insert_state(initial_state)
        .add_sub_state::<GamePhase>()
        .init_resource::<crate::core::test_mode::TestMode>()
        .insert_resource({
            let mut cfg = RapierConfiguration::new(100.0);
            cfg.gravity = Vec2::ZERO;
            cfg
        })
}

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        configure_shared_game(app, AppState::Loading).add_plugins((
            EventsPlugin,
            AssetsPlugin,
            DataPlugin,
            InputPlugin,
            AudioPlugin,
            SavePlugin,
            AchievementsPlugin,
            LocalDebugPlugin,
            RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0),
            CameraPlugin,
            GameplayPlugin,
            CoopPlugin,
            PvpPlugin,
            UiPlugin,
        ));
    }
}

impl Plugin for DedicatedServerPlugin {
    fn build(&self, app: &mut App) {
        configure_shared_game(app, AppState::MainMenu)
            .init_resource::<PlayerInputState>()
            .init_resource::<PvpNetConfig>()
            .init_resource::<PvpNetState>()
            .add_plugins((
                EventsPlugin,
                PlaceholderAssetsPlugin,
                HeadlessDataPlugin,
                HeadlessUiSupportPlugin,
                LocalDebugPlugin,
                RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0),
                GameplayPlugin,
                HeadlessCoopPlugin,
                HeadlessPvpPlugin,
            ));
    }
}
