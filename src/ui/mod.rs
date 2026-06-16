pub mod augment_select;
pub mod character_panel;
pub mod cursor;
pub mod event_room;
pub mod feedback;
pub mod game_over;
pub mod hud;
pub mod levelup_select;
pub mod menu;
pub mod notifications;
pub mod pause;
pub mod reward_select;
pub mod shop;
pub mod skill_select;
pub mod tooltip;
pub mod tutorial;
pub mod widgets;
pub mod window_controls;

use bevy::prelude::*;

use crate::gameplay::effects::screen_flash::clear_screen_flash;
use crate::states::{AppState, GamePhase};

pub struct UiPlugin;
pub struct HeadlessUiSupportPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<skill_select::SkillEquippedEvent>()
            .add_event::<skill_select::SkillEquipCancelledEvent>()
            .add_event::<feedback::UiFeedbackEvent>()
            .init_resource::<feedback::ActiveUiFeedback>()
            .add_plugins(tutorial::TutorialPlugin)
            .add_plugins(tooltip::TooltipPlugin)
            .init_resource::<menu::MainMenuScreen>()
            .init_resource::<window_controls::WindowControlState>()
            .add_systems(
                Update,
                (
                    cursor::ensure_cursor_visuals,
                    cursor::sync_window_cursor_visibility,
                    cursor::update_custom_cursor,
                    cursor::update_crosshair,
                    window_controls::ensure_window_controls,
                    window_controls::window_control_button_system,
                ),
            )
            .add_systems(OnEnter(AppState::MainMenu), menu::setup_main_menu)
            .add_systems(
                Update,
                (menu::menu_button_system, menu::update_main_menu_content)
                    .run_if(in_state(AppState::MainMenu)),
            )
            .add_systems(OnExit(AppState::MainMenu), menu::cleanup_main_menu)
            .add_systems(Update, notifications::ensure_notification_root)
            .add_systems(Update, notifications::handle_achievement_notifications)
            .add_systems(Update, notifications::update_notifications)
            .add_systems(Update, feedback::ensure_feedback_root)
            .add_systems(
                Update,
                feedback::handle_ui_feedback_events
                    .run_if(in_state(AppState::InGame).or_else(in_state(AppState::CoopGame))),
            )
            .add_systems(Update, feedback::update_feedback_toasts)
            .add_systems(OnEnter(AppState::InGame), hud::setup_hud)
            .add_systems(OnEnter(AppState::CoopGame), hud::setup_hud)
            .add_systems(
                OnEnter(GamePhase::EventRoom),
                event_room::setup_event_room_ui,
            )
            .add_systems(
                Update,
                event_room::event_room_ui_input_system.run_if(in_state(GamePhase::EventRoom)),
            )
            .add_systems(
                Update,
                (
                    hud::update_health_bar,
                    hud::update_health_text,
                    hud::update_experience_bar,
                    hud::update_experience_text,
                    hud::update_gold_text,
                    hud::update_energy_text,
                    hud::update_dash_cooldown_ui,
                    hud::update_skill_bar_ui,
                    hud::update_floor_text,
                    hud::update_room_text,
                    hud::update_enemy_count_text,
                    hud::update_hint_text,
                    hud::update_boss_health_bar,
                    hud::update_stage_progress,
                )
                    .run_if(in_state(AppState::InGame).or_else(in_state(AppState::CoopGame))),
            )
            .add_systems(OnExit(AppState::InGame), hud::cleanup_hud)
            .add_systems(OnExit(AppState::CoopGame), hud::cleanup_hud)
            .add_systems(
                OnExit(GamePhase::EventRoom),
                event_room::cleanup_event_room_ui,
            )
            .add_systems(Update, pause::toggle_pause)
            .add_systems(OnEnter(GamePhase::Paused), pause::setup_pause_menu)
            .add_systems(
                Update,
                pause::pause_menu_keyboard_system.run_if(in_state(GamePhase::Paused)),
            )
            .add_systems(OnExit(GamePhase::Paused), pause::cleanup_pause_menu)
            .add_systems(OnEnter(GamePhase::Feedback), feedback::setup_feedback_card)
            .add_systems(
                Update,
                feedback::feedback_input_system.run_if(in_state(GamePhase::Feedback)),
            )
            .add_systems(OnExit(GamePhase::Feedback), feedback::cleanup_feedback_card)
            .add_systems(OnEnter(GamePhase::Shop), shop::setup_shop_ui)
            .add_systems(
                Update,
                shop::shop_ui_input_system.run_if(in_state(GamePhase::Shop)),
            )
            .add_systems(
                Update,
                shop::update_shop_ui.run_if(in_state(GamePhase::Shop)),
            )
            .add_systems(OnExit(GamePhase::Shop), shop::cleanup_shop_ui)
            // Augment select
            .init_resource::<augment_select::AugmentChoices>()
            .add_systems(
                OnEnter(GamePhase::AugmentSelect),
                (augment_select::setup_augment_select_ui, clear_screen_flash),
            )
            .add_systems(
                Update,
                augment_select::augment_select_input.run_if(in_state(GamePhase::AugmentSelect)),
            )
            .add_systems(
                OnExit(GamePhase::AugmentSelect),
                augment_select::cleanup_augment_select_ui,
            )
            // Skill select
            .init_resource::<skill_select::SkillChoices>()
            .add_systems(
                OnEnter(GamePhase::SkillSelect),
                (skill_select::setup_skill_select_ui, clear_screen_flash),
            )
            .add_systems(
                Update,
                skill_select::skill_select_input.run_if(in_state(GamePhase::SkillSelect)),
            )
            .add_systems(
                OnExit(GamePhase::SkillSelect),
                skill_select::cleanup_skill_select_ui,
            )
            // Level-up select
            .init_resource::<levelup_select::LevelUpChoices>()
            .add_systems(
                OnEnter(GamePhase::LevelUpSelect),
                (levelup_select::setup_levelup_ui, clear_screen_flash),
            )
            .add_systems(
                Update,
                levelup_select::levelup_input.run_if(in_state(GamePhase::LevelUpSelect)),
            )
            .add_systems(
                OnExit(GamePhase::LevelUpSelect),
                levelup_select::cleanup_levelup_ui,
            )
            //
            .add_systems(
                OnEnter(GamePhase::GameOver),
                game_over::setup_game_over_screen,
            )
            .add_systems(OnEnter(GamePhase::Victory), game_over::setup_victory_screen)
            .add_systems(
                Update,
                game_over::end_screen_input_system
                    .run_if(in_state(GamePhase::GameOver).or_else(in_state(GamePhase::Victory))),
            );
    }
}

impl Plugin for HeadlessUiSupportPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<skill_select::SkillEquippedEvent>()
            .add_event::<skill_select::SkillEquipCancelledEvent>()
            .add_event::<feedback::UiFeedbackEvent>()
            .add_event::<tutorial::TutorialNotification>()
            .init_resource::<tutorial::TutorialFlags>()
            .init_resource::<augment_select::AugmentChoices>()
            .init_resource::<skill_select::SkillChoices>()
            .init_resource::<levelup_select::LevelUpChoices>();
    }
}
