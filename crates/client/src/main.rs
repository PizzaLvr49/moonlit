use bevy::prelude::*;
use bevy::window::{PresentMode, WindowMode};
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_panic_handler::PanicHandlerBuilder;
use bevy_seedling::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                present_mode: PresentMode::AutoVsync,
                mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(SeedlingPlugin::default())
        .add_plugins(PanicHandlerBuilder::default().build())
        .add_plugins((EguiPlugin::default(), WorldInspectorPlugin::default()))
        .run();
}
