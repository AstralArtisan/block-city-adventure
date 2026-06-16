#![allow(dead_code)]

use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::HashMap;

use crate::core::achievements::AchievementId;
use crate::gameplay::enemy::components::{BossArchetype, EnemyType};
use crate::states::AppState;

#[derive(Resource, Clone, Default)]
pub struct GameAssets {
    pub font: Handle<Font>,
    pub textures: TextureHandles,
    #[allow(dead_code)]
    pub audio: AudioHandles,
}

#[derive(Resource, Clone, Default)]
pub struct TextureHandles {
    pub white: Handle<Image>,
    pub white_ring: Handle<Image>,
    pub player: Handle<Image>,
    pub cursor: Handle<Image>,
    pub crosshair: Handle<Image>,
    pub slash: Handle<Image>,
    pub slash_layout: Handle<TextureAtlasLayout>,
    pub enemy_sprites: HashMap<EnemyType, Handle<Image>>,
    pub boss_sprites: HashMap<BossArchetype, Handle<Image>>,
    pub room_background: Handle<Image>,
    pub menu_background: Handle<Image>,
    pub achievement_icons: HashMap<AchievementId, Handle<Image>>,
}

#[allow(dead_code)]
#[derive(Resource, Clone, Default)]
pub struct AudioHandles {
    pub ui_click: Handle<bevy_kira_audio::AudioSource>,
    pub attack: Handle<bevy_kira_audio::AudioSource>,
    pub dash: Handle<bevy_kira_audio::AudioSource>,
    pub hit: Handle<bevy_kira_audio::AudioSource>,
}

pub struct AssetsPlugin;

pub struct PlaceholderAssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Loading), load_game_assets)
            .add_systems(
                Update,
                check_assets_ready.run_if(in_state(AppState::Loading)),
            );
    }
}

impl Plugin for PlaceholderAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameAssets>();
    }
}

pub fn load_game_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let font = asset_server.load("fonts/main_font.ttf");
    let player = asset_server.load("textures/player_hero.png");
    let slash = asset_server.load("textures/effects/melee_slash_sprites.png");
    let slash_layout = atlas_layouts.add(TextureAtlasLayout::from_grid(
        UVec2::new(64, 47),
        3,
        3,
        None,
        None,
    ));
    let cursor = images.add(make_cursor_image());
    let crosshair = images.add(make_crosshair_image());

    let mut enemy_sprites = HashMap::new();
    enemy_sprites.insert(
        EnemyType::MeleeChaser,
        asset_server.load("textures/enemies/melee_chaser.png"),
    );
    enemy_sprites.insert(
        EnemyType::RangedShooter,
        asset_server.load("textures/enemies/ranged_shooter.png"),
    );

    let mut boss_sprites = HashMap::new();
    boss_sprites.insert(
        BossArchetype::Floor1Guardian,
        asset_server.load("textures/bosses/floor1_guardian.png"),
    );

    let room_background = images.add(make_room_background_image());
    let menu_background = asset_server.load("textures/menu.png");
    let achievement_icons = achievement_icon_paths()
        .into_iter()
        .map(|(id, path)| (id, asset_server.load(path)))
        .collect();

    let white = images.add(Image::new_fill(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ));
    let white_ring = images.add(make_white_ring_image());

    commands.insert_resource(GameAssets {
        font,
        textures: TextureHandles {
            white,
            white_ring,
            player,
            cursor,
            crosshair,
            slash,
            slash_layout,
            enemy_sprites,
            boss_sprites,
            room_background,
            menu_background,
            achievement_icons,
        },
        audio: AudioHandles::default(),
    });
}

pub fn check_assets_ready(
    assets: Res<GameAssets>,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if asset_server.is_loaded_with_dependencies(&assets.font)
        && asset_server.is_loaded_with_dependencies(&assets.textures.player)
        && asset_server.is_loaded_with_dependencies(&assets.textures.slash)
        && asset_server.is_loaded_with_dependencies(&assets.textures.menu_background)
        && assets
            .textures
            .achievement_icons
            .values()
            .all(|icon| asset_server.is_loaded_with_dependencies(icon))
    {
        next_state.set(AppState::MainMenu);
    }
}

fn achievement_icon_paths() -> [(AchievementId, &'static str); 9] {
    [
        (
            AchievementId::FirstBlood,
            "textures/achievements/firstblood.png",
        ),
        (
            AchievementId::EliteSlayer,
            "textures/achievements/eliteslayer.png",
        ),
        (AchievementId::Combo10, "textures/achievements/combo10.png"),
        (AchievementId::Rich, "textures/achievements/rich.png"),
        (AchievementId::Shopper, "textures/achievements/shopper.png"),
        (
            AchievementId::PuzzleSolver,
            "textures/achievements/puzzlesolver.png",
        ),
        (
            AchievementId::BossSlayer,
            "textures/achievements/bossslayer.png",
        ),
        (
            AchievementId::Untouchable,
            "textures/achievements/untouchable.png",
        ),
        (AchievementId::Victory, "textures/achievements/victory.png"),
    ]
}

fn make_cursor_image() -> Image {
    let size = 28u32;
    let mut data = vec![0; (size * size * 4) as usize];

    let shadow = [0, 0, 0, 120];
    let fill = [34, 31, 42, 245];
    let outline = [246, 230, 210, 255];
    let accent = [196, 66, 72, 255];
    let core = [255, 247, 236, 255];

    let center = 13;
    for y in 0..size as i32 {
        for x in 0..size as i32 {
            let shadow_distance = (x - (center + 1)).abs() + (y - (center + 1)).abs();
            if shadow_distance <= 8 {
                set_icon_pixel(&mut data, size, x, y, shadow);
            }

            let distance = (x - center).abs() + (y - center).abs();
            if distance <= 7 {
                let color = if x <= center && y <= center && distance >= 3 {
                    accent
                } else {
                    fill
                };
                set_icon_pixel(&mut data, size, x, y, color);
            }

            if (7..=8).contains(&distance) {
                set_icon_pixel(&mut data, size, x, y, outline);
            }
            if distance <= 1 {
                set_icon_pixel(&mut data, size, x, y, core);
            }
        }
    }

    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn make_crosshair_image() -> Image {
    let size = 36u32;
    let mut data = vec![0; (size * size * 4) as usize];

    let shadow = [0, 0, 0, 120];
    let outline = [255, 255, 255, 255];

    draw_crosshair_layer(&mut data, size, 1, 1, shadow);
    draw_crosshair_layer(&mut data, size, 0, 0, outline);

    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn make_room_background_image() -> Image {
    Image::new_fill(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[34, 36, 44, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn make_white_ring_image() -> Image {
    let size = 64u32;
    let mut data = vec![0; (size * size * 4) as usize];
    let center = (size as f32 - 1.0) * 0.5;
    let radius = center - 1.0;
    let thickness = 4.0;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let edge_delta = (dist - radius).abs();
            if edge_delta <= thickness {
                let alpha = ((thickness - edge_delta) / thickness * 255.0).round() as u8;
                let index = ((y * size + x) * 4) as usize;
                data[index..index + 4].copy_from_slice(&[255, 255, 255, alpha]);
            }
        }
    }

    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn draw_crosshair_layer(data: &mut [u8], size: u32, offset_x: i32, offset_y: i32, color: [u8; 4]) {
    draw_hline(
        data,
        size,
        4 + offset_x,
        11 + offset_x,
        7 + offset_y,
        2,
        color,
    );
    draw_hline(
        data,
        size,
        24 + offset_x,
        31 + offset_x,
        7 + offset_y,
        2,
        color,
    );
    draw_hline(
        data,
        size,
        4 + offset_x,
        11 + offset_x,
        28 + offset_y,
        2,
        color,
    );
    draw_hline(
        data,
        size,
        24 + offset_x,
        31 + offset_x,
        28 + offset_y,
        2,
        color,
    );

    draw_vline(
        data,
        size,
        7 + offset_x,
        4 + offset_y,
        11 + offset_y,
        2,
        color,
    );
    draw_vline(
        data,
        size,
        28 + offset_x,
        4 + offset_y,
        11 + offset_y,
        2,
        color,
    );
    draw_vline(
        data,
        size,
        7 + offset_x,
        24 + offset_y,
        31 + offset_y,
        2,
        color,
    );
    draw_vline(
        data,
        size,
        28 + offset_x,
        24 + offset_y,
        31 + offset_y,
        2,
        color,
    );

    draw_hline(
        data,
        size,
        15 + offset_x,
        20 + offset_x,
        17 + offset_y,
        2,
        color,
    );
    draw_vline(
        data,
        size,
        17 + offset_x,
        15 + offset_y,
        20 + offset_y,
        2,
        color,
    );
}

fn draw_hline(
    data: &mut [u8],
    size: u32,
    start_x: i32,
    end_x: i32,
    y: i32,
    thickness: i32,
    color: [u8; 4],
) {
    for dy in 0..thickness {
        for x in start_x..=end_x {
            set_icon_pixel(data, size, x, y + dy, color);
        }
    }
}

fn draw_vline(
    data: &mut [u8],
    size: u32,
    x: i32,
    start_y: i32,
    end_y: i32,
    thickness: i32,
    color: [u8; 4],
) {
    for dx in 0..thickness {
        for y in start_y..=end_y {
            set_icon_pixel(data, size, x + dx, y, color);
        }
    }
}

fn set_icon_pixel(data: &mut [u8], size: u32, x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= size as i32 || y >= size as i32 {
        return;
    }

    let index = ((y as u32 * size + x as u32) * 4) as usize;
    data[index..index + 4].copy_from_slice(&color);
}
