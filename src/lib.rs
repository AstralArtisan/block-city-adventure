mod app;
mod constants;
mod coop;
mod core;
mod data;
mod gameplay;
mod prelude;
mod pvp;
mod states;
mod ui;
mod utils;

use std::path::{Path, PathBuf};

use bevy::app::ScheduleRunnerPlugin;
use bevy::asset::AssetPlugin;
use bevy::hierarchy::HierarchyPlugin;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::transform::TransformPlugin;
use bevy::utils::Duration;
use bevy::window::WindowMode;

use crate::app::{DedicatedServerPlugin, GamePlugin};
use crate::constants::{WINDOW_CLEAR_COLOR, WINDOW_HEIGHT, WINDOW_WIDTH};

pub fn run_game() {
    App::new()
        .insert_resource(ClearColor(WINDOW_CLEAR_COLOR))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(primary_window_settings()),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: resolve_asset_path(),
                    ..default()
                }),
        )
        .add_plugins(GamePlugin)
        .run();
}

pub fn run_dedicated_server() {
    App::new()
        .insert_resource(ClearColor(WINDOW_CLEAR_COLOR))
        .add_plugins(bevy::MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(
            Duration::from_secs_f64(1.0 / 60.0),
        )))
        .add_plugins((
            LogPlugin::default(),
            TransformPlugin,
            HierarchyPlugin,
            StatesPlugin,
        ))
        .add_plugins(DedicatedServerPlugin)
        .run();
}

fn primary_window_settings() -> Window {
    Window {
        title: "勇闯方块城".to_string(),
        mode: WindowMode::BorderlessFullscreen,
        resolution: (WINDOW_WIDTH, WINDOW_HEIGHT).into(),
        resizable: true,
        ..default()
    }
}

fn resolve_asset_path() -> String {
    let mut candidates = Vec::new();

    if let Ok(current_dir) = std::env::current_dir() {
        push_asset_candidates(&mut candidates, &current_dir);
    }
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
    {
        push_asset_candidates(&mut candidates, exe_dir);
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.is_dir())
        .unwrap_or_else(|| PathBuf::from("assets"))
        .to_string_lossy()
        .into_owned()
}

fn push_asset_candidates(candidates: &mut Vec<PathBuf>, base: &Path) {
    candidates.push(base.join("assets"));
    if let Some(parent) = base.parent() {
        candidates.push(parent.join("assets"));
        if let Some(grandparent) = parent.parent() {
            candidates.push(grandparent.join("assets"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_window_defaults_to_fullscreen_design_resolution() {
        let window = primary_window_settings();

        assert_eq!(window.mode, WindowMode::BorderlessFullscreen);
        assert_eq!(window.resolution.width(), WINDOW_WIDTH);
        assert_eq!(window.resolution.height(), WINDOW_HEIGHT);
    }

    #[test]
    fn asset_candidates_include_base_and_two_parents() {
        let mut candidates = Vec::new();
        push_asset_candidates(&mut candidates, Path::new("target/release"));

        assert!(candidates.contains(&PathBuf::from("target/release/assets")));
        assert!(candidates.contains(&PathBuf::from("target/assets")));
        assert!(candidates.contains(&PathBuf::from("assets")));
    }
}
