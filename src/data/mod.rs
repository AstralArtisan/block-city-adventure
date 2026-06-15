pub mod definitions;
pub mod loaders;
pub mod registry;

use bevy::prelude::*;

use crate::states::AppState;

pub struct DataPlugin;
pub struct HeadlessDataPlugin;

impl Plugin for DataPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Loading), loaders::load_all_configs);
    }
}

impl Plugin for HeadlessDataPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, loaders::load_all_configs);
    }
}
