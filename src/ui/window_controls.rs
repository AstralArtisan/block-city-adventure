use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowMode};

use crate::core::assets::GameAssets;
use crate::ui::widgets;

#[derive(Component)]
pub struct WindowControlsRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowControlAction {
    Minimize,
    Maximize,
    Close,
}

#[derive(Component)]
pub struct WindowControlButton(WindowControlAction);

#[derive(Resource, Debug, Default)]
pub struct WindowControlState {
    maximized: bool,
}

pub fn ensure_window_controls(
    mut commands: Commands,
    assets: Option<Res<GameAssets>>,
    root_q: Query<(), With<WindowControlsRoot>>,
    window_q: Query<(), With<PrimaryWindow>>,
) {
    if root_q.iter().next().is_some() || window_q.iter().next().is_none() {
        return;
    }
    let Some(assets) = assets else {
        return;
    };

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    top: Val::Px(10.0),
                    right: Val::Px(12.0),
                    height: Val::Px(34.0),
                    column_gap: Val::Px(6.0),
                    justify_content: JustifyContent::FlexEnd,
                    align_items: AlignItems::Center,
                    ..default()
                },
                z_index: ZIndex::Global(1000),
                ..default()
            },
            WindowControlsRoot,
            Name::new("WindowControlsRoot"),
        ))
        .with_children(|root| {
            spawn_window_button(root, &assets, "-", WindowControlAction::Minimize);
            spawn_window_button(root, &assets, "□", WindowControlAction::Maximize);
            spawn_window_button(root, &assets, "X", WindowControlAction::Close);
        });
}

pub fn window_control_button_system(
    mut interaction_q: Query<
        (&Interaction, &WindowControlButton, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
    mut state: ResMut<WindowControlState>,
    mut exit: EventWriter<AppExit>,
) {
    for (interaction, action, mut color) in &mut interaction_q {
        color.0 = match (*interaction, action.0) {
            (Interaction::Hovered, WindowControlAction::Close) => {
                widgets::button_danger_hover_color()
            }
            (Interaction::Pressed, WindowControlAction::Close) => widgets::button_danger_color(),
            (Interaction::Hovered, _) => widgets::button_hover_color(),
            (Interaction::Pressed, _) => widgets::button_selected_color(),
            (Interaction::None, WindowControlAction::Close) => widgets::button_danger_color(),
            (Interaction::None, _) => Color::srgba(0.08, 0.10, 0.14, 0.82),
        };

        if *interaction != Interaction::Pressed {
            continue;
        }

        match action.0 {
            WindowControlAction::Minimize => {
                if let Ok(mut window) = window_q.get_single_mut() {
                    window.set_minimized(true);
                }
            }
            WindowControlAction::Maximize => {
                if let Ok(mut window) = window_q.get_single_mut() {
                    state.maximized = !state.maximized;
                    if state.maximized {
                        window.mode = WindowMode::Windowed;
                        window.set_maximized(true);
                    } else {
                        window.set_maximized(false);
                        window.mode = WindowMode::BorderlessFullscreen;
                    }
                }
            }
            WindowControlAction::Close => {
                let _ = exit.send(AppExit::Success);
            }
        }
    }
}

fn spawn_window_button(
    parent: &mut ChildBuilder,
    assets: &GameAssets,
    label: &str,
    action: WindowControlAction,
) {
    parent
        .spawn((
            ButtonBundle {
                style: Style {
                    width: Val::Px(34.0),
                    height: Val::Px(28.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                background_color: BackgroundColor(match action {
                    WindowControlAction::Close => widgets::button_danger_color(),
                    _ => Color::srgba(0.08, 0.10, 0.14, 0.82),
                }),
                border_color: BorderColor(Color::srgba(0.75, 0.80, 0.90, 0.45)),
                ..default()
            },
            WindowControlButton(action),
        ))
        .with_children(|button| {
            button.spawn(widgets::title_text(assets, label, 16.0));
        });
}
