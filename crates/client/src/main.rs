use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig};
use bevy::platform::collections::HashSet;
use bevy::platform::prelude::*;
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowMode};
use bevy_asset_loader::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use bevy_egui::EguiPlugin;
use bevy_enhanced_input::prelude::*;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_modern_pixel_camera::prelude::*;
use bevy_panic_handler::PanicHandlerBuilder;
use bevy_rand::prelude::*;
use bevy_seedling::prelude::*;
use noisy_bevy::fbm_simplex_2d_seeded;
use rand::RngCore;

const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 16.0, y: 16.0 };
const CHUNK_SIZE: UVec2 = UVec2 { x: 10, y: 10 };
const CHUNK_RENDER_DISTANCE: UVec2 = UVec2 { x: 2, y: 2 };

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
                    min_fps: 60.0,
                    target_fps: 180.0,
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
        .add_plugins(EnhancedInputPlugin)
        .add_plugins(EntropyPlugin::<WyRand>::with_seed([42; 8]))
        .init_state::<GameState>()
        .insert_resource(ChunkManager::default())
        .insert_resource(WorldSeed::default())
        .add_loading_state(
            LoadingState::new(GameState::Loading)
                .continue_to_state(GameState::Playing)
                .load_collection::<GameAssets>(),
        )
        .add_input_context::<CameraController>()
        .add_systems(OnEnter(GameState::Playing), setup_camera)
        .add_observer(camera_movement)
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

#[derive(Component)]
struct CameraController;

#[derive(InputAction)]
#[action_output(Vec2)]
struct CameraMovement;

#[derive(Default, Debug, Resource)]
struct ChunkManager {
    pub spawned_chunks: HashSet<IVec2>,
}

#[derive(Default, Resource)]
struct WorldSeed {
    seed: u64,
}

#[derive(Component)]
struct ChunkMarker;

#[derive(Component)]
struct TerrainChunk;

fn setup_camera(
    mut commands: Commands,
    mut global_rng: Single<&mut WyRand, With<GlobalRng>>,
    mut world_seed: ResMut<WorldSeed>,
) {
    world_seed.seed = global_rng.next_u64();

    commands.spawn((
        Camera2d,
        Msaa::Off,
        PixelZoom::FitSize {
            width: 320,
            height: 180,
        },
        PixelViewport,
        CameraController,
        actions!(CameraController[
            (
                Action::<CameraMovement>::new(),
                DeadZone::default(),
                SmoothNudge::default(),
                Bindings::spawn((
                    Cardinal::wasd_keys(),
                    Cardinal::arrows(),
                    Axial::left_stick(),
                )),
            ),
        ]),
    ));
}

fn camera_movement(
    input: On<Fire<CameraMovement>>,
    time: Res<Time>,
    mut transform: Single<&mut Transform, With<Camera>>,
) {
    let translation_amount = time.delta_secs() * 200.0;
    transform.translation += Vec3::from((input.value * translation_amount, 0.0));
}

fn camera_pos_to_chunk_pos(camera_pos: &Vec2) -> IVec2 {
    let camera_pos = camera_pos.as_ivec2();
    let chunk_size = IVec2::new(CHUNK_SIZE.x as i32, CHUNK_SIZE.y as i32);
    let tile_size = IVec2::new(TILE_SIZE.x as i32, TILE_SIZE.y as i32);
    camera_pos / (chunk_size * tile_size)
}

// Stable FBM helper
fn fbm_safe(pos: Vec2, octaves: usize, lacunarity: f32, gain: f32, seed: u64) -> f32 {
    let scaled_pos = pos / 10.0;
    let seed_f = (seed % 10000) as f32 / 10000.0;
    let mut sum = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;

    for _ in 0..octaves {
        let value = fbm_simplex_2d_seeded(scaled_pos * frequency, 1, lacunarity, gain, seed_f);
        sum += value * amplitude;
        amplitude *= gain;
        frequency *= lacunarity;
    }

    sum.clamp(-1.0, 1.0)
}

fn get_tile_type(world_x: i32, world_y: i32, seed: u64) -> u32 {
    let scale = 0.08;
    let pos = Vec2::new(world_x as f32 * scale, world_y as f32 * scale);

    let terrain = fbm_safe(pos, 4, 2.0, 0.5, seed);
    let moisture = fbm_safe(pos + Vec2::splat(100.0), 3, 2.0, 0.5, seed + 1000);

    if terrain < -0.25 {
        1
    } else if terrain < 0.0 {
        if moisture > 0.3 { 0 } else { 2 }
    } else if terrain < 0.3 {
        if moisture > 0.1 { 0 } else { 2 }
    } else if terrain < 0.55 {
        if moisture < -0.2 { 4 } else { 3 }
    } else {
        5
    }
}

fn spawn_chunk(
    commands: &mut Commands,
    game_assets: &GameAssets,
    world_seed: u64,
    chunk_pos: IVec2,
) {
    let tilemap_entity = commands.spawn_empty().id();
    let mut tile_storage = TileStorage::empty(CHUNK_SIZE.into());

    for x in 0..CHUNK_SIZE.x {
        for y in 0..CHUNK_SIZE.y {
            let tile_pos = TilePos { x, y };

            let world_x = chunk_pos.x * CHUNK_SIZE.x as i32 + x as i32;
            let world_y = chunk_pos.y * CHUNK_SIZE.y as i32 + y as i32;

            let texture_index = get_tile_type(world_x, world_y, world_seed);

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
        TerrainChunk,
    ));
}

fn spawn_chunks_around_camera(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    world_seed: Res<WorldSeed>,
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
                    spawn_chunk(&mut commands, &game_assets, world_seed.seed, chunk_pos);
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

            if (chunk_coord.x - camera_chunk_pos.x).abs() > CHUNK_RENDER_DISTANCE.x as i32
                || (chunk_coord.y - camera_chunk_pos.y).abs() > CHUNK_RENDER_DISTANCE.y as i32
            {
                chunk_manager.spawned_chunks.remove(&chunk_coord);
                commands.entity(entity).despawn();
            }
        }
    }
}
