use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig};
use bevy::platform::collections::HashSet;
use bevy::platform::prelude::*;
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowMode};
use bevy_asset_loader::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_modern_pixel_camera::prelude::*;
use bevy_panic_handler::PanicHandlerBuilder;
use bevy_seedling::prelude::*;

const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 16.0, y: 16.0 };
const CHUNK_SIZE: UVec2 = UVec2 { x: 10, y: 10 };
const CHUNK_RENDER_DISTANCE: UVec2 = UVec2 { x: 2, y: 1 };

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        present_mode: PresentMode::AutoVsync,
                        mode: WindowMode::BorderlessFullscreen(MonitorSelection::Current),
                        title: "Moonlit".to_string(),
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(TilemapPlugin)
        .add_plugins(SeedlingPlugin::default())
        .add_plugins(PanicHandlerBuilder::default().build())
        .add_plugins(PixelCameraPlugin)
        .add_plugins((EguiPlugin::default(), WorldInspectorPlugin::default()))
        .add_plugins(FpsOverlayPlugin {
            config: FpsOverlayConfig {
                frame_time_graph_config: FrameTimeGraphConfig {
                    min_fps: 30.0,
                    target_fps: 60.0,
                    ..default()
                },
                text_color: Color::WHITE,
                text_config: TextFont {
                    font_size: 42.0,
                    ..default()
                },
                ..default()
            },
        })
        .init_state::<GameState>()
        .insert_resource(ChunkManager::default())
        .add_loading_state(
            LoadingState::new(GameState::Loading)
                .continue_to_state(GameState::Playing)
                .load_collection::<GameAssets>(),
        )
        .add_systems(OnEnter(GameState::Playing), setup_camera)
        .add_systems(Update, camera_movement.run_if(in_state(GameState::Playing)))
        .add_systems(
            Update,
            spawn_chunks_around_camera.run_if(in_state(GameState::Playing)),
        )
        .add_systems(
            Update,
            despawn_outofrange_chunks.run_if(in_state(GameState::Playing)),
        )
        .run();
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum GameState {
    #[default]
    Loading,
    Playing,
}

#[derive(AssetCollection, Resource)]
struct GameAssets {
    #[asset(path = "tiles.png")]
    tileset: Handle<Image>,
}

#[derive(Default, Debug, Resource)]
struct ChunkManager {
    pub spawned_chunks: HashSet<IVec2>,
}

#[derive(Component)]
struct ChunkMarker;

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Msaa::Off,
        PixelZoom::FitSize {
            width: 320,
            height: 180,
        },
        PixelViewport,
    ));
}

fn camera_movement(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Transform, With<Camera>>,
) {
    let mut direction = Vec3::ZERO;
    let speed = 200.0;

    if keyboard_input.pressed(KeyCode::KeyW) {
        direction.y += 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyS) {
        direction.y -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyA) {
        direction.x -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyD) {
        direction.x += 1.0;
    }

    if direction != Vec3::ZERO {
        direction = direction.normalize();
        for mut transform in query.iter_mut() {
            transform.translation += direction * speed * time.delta_secs();
        }
    }
}

fn camera_pos_to_chunk_pos(camera_pos: &Vec2) -> IVec2 {
    let camera_pos = camera_pos.as_ivec2();
    let chunk_size: IVec2 = IVec2::new(CHUNK_SIZE.x as i32, CHUNK_SIZE.y as i32);
    let tile_size: IVec2 = IVec2::new(TILE_SIZE.x as i32, TILE_SIZE.y as i32);
    camera_pos / (chunk_size * tile_size)
}

fn spawn_chunk(commands: &mut Commands, game_assets: &GameAssets, chunk_pos: IVec2) {
    let tilemap_entity = commands.spawn_empty().id();
    let mut tile_storage = TileStorage::empty(CHUNK_SIZE.into());

    for x in 0..CHUNK_SIZE.x {
        for y in 0..CHUNK_SIZE.y {
            let tile_pos = TilePos { x, y };

            let chunk_x = chunk_pos.x.wrapping_mul(CHUNK_SIZE.x as i32) as u32;
            let chunk_y = chunk_pos.y.wrapping_mul(CHUNK_SIZE.y as i32) as u32;
            let texture_index = x
                .wrapping_add(y)
                .wrapping_add(chunk_x)
                .wrapping_add(chunk_y)
                % 6;

            let tile_entity = commands
                .spawn(TileBundle {
                    position: tile_pos,
                    tilemap_id: TilemapId(tilemap_entity),
                    texture_index: TileTextureIndex(texture_index),
                    ..default()
                })
                .id();

            commands.entity(tilemap_entity).add_child(tile_entity);
            tile_storage.set(&tile_pos, tile_entity);
        }
    }

    let transform = Transform::from_translation(Vec3::new(
        chunk_pos.x as f32 * CHUNK_SIZE.x as f32 * TILE_SIZE.x,
        chunk_pos.y as f32 * CHUNK_SIZE.y as f32 * TILE_SIZE.y,
        0.0,
    ));

    commands.entity(tilemap_entity).insert((
        TilemapBundle {
            grid_size: TILE_SIZE.into(),
            size: CHUNK_SIZE.into(),
            storage: tile_storage,
            texture: TilemapTexture::Single(game_assets.tileset.clone()),
            tile_size: TILE_SIZE,
            transform,
            render_settings: TilemapRenderSettings {
                render_chunk_size: CHUNK_SIZE,
                ..Default::default()
            },
            ..Default::default()
        },
        ChunkMarker,
    ));
}

fn spawn_chunks_around_camera(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    camera_query: Query<&Transform, With<Camera>>,
    mut chunk_manager: ResMut<ChunkManager>,
) {
    for transform in camera_query.iter() {
        let camera_chunk_pos = camera_pos_to_chunk_pos(&transform.translation.xy());

        for y in (camera_chunk_pos.y - CHUNK_RENDER_DISTANCE.y as i32)
            ..=(camera_chunk_pos.y + CHUNK_RENDER_DISTANCE.y as i32)
        {
            for x in (camera_chunk_pos.x - CHUNK_RENDER_DISTANCE.x as i32)
                ..=(camera_chunk_pos.x + CHUNK_RENDER_DISTANCE.x as i32)
            {
                let chunk_pos = IVec2::new(x, y);
                if !chunk_manager.spawned_chunks.contains(&chunk_pos) {
                    chunk_manager.spawned_chunks.insert(chunk_pos);
                    spawn_chunk(&mut commands, &game_assets, chunk_pos);
                }
            }
        }
    }
}

fn despawn_outofrange_chunks(
    mut commands: Commands,
    camera_query: Query<&Transform, With<Camera>>,
    chunks_query: Query<(Entity, &Transform), With<ChunkMarker>>,
    mut chunk_manager: ResMut<ChunkManager>,
) {
    for camera_transform in camera_query.iter() {
        let camera_chunk_pos = camera_pos_to_chunk_pos(&camera_transform.translation.xy());

        for (entity, chunk_transform) in chunks_query.iter() {
            let chunk_pos = chunk_transform.translation.xy();
            let x = (chunk_pos.x / (CHUNK_SIZE.x as f32 * TILE_SIZE.x)).floor() as i32;
            let y = (chunk_pos.y / (CHUNK_SIZE.y as f32 * TILE_SIZE.y)).floor() as i32;
            let chunk_coord = IVec2::new(x, y);

            let distance_x = (chunk_coord.x - camera_chunk_pos.x).abs();
            let distance_y = (chunk_coord.y - camera_chunk_pos.y).abs();

            if distance_x > CHUNK_RENDER_DISTANCE.x as i32
                || distance_y > CHUNK_RENDER_DISTANCE.y as i32
            {
                chunk_manager.spawned_chunks.remove(&chunk_coord);
                commands.entity(entity).despawn();
            }
        }
    }
}
