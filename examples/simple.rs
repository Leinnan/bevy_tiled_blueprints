use bevy::prelude::*;
use serde::Serialize;

fn startup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2dBundle::default());

    let map_handle: Handle<bevy_tiled_blueprints::TiledMap> = asset_server.load("map.tmx");

    commands.spawn(bevy_tiled_blueprints::TiledMapBundle {
        tiled_map: map_handle,
        ..Default::default()
    });
}

#[derive(Debug, Reflect, Component, Default, Clone)]
#[reflect(Component)]
pub struct ExampleComponent;

#[derive(Debug, Reflect, Component, Default, Clone)]
#[reflect(Component)]
pub struct ExampleComponentWithInt(pub i32);

#[derive(Debug, Reflect, Component, Default, Clone)]
#[reflect(Component)]
pub struct ExampleBoolComponent(pub bool);

#[derive(Debug, Reflect, Component, Default, Clone, Serialize)]
#[reflect(Component)]
pub struct ComplexType {
    pub name: String,
    pub strength: i32,
    pub dexterity: f32,
}

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .register_type::<ExampleComponent>()
        .register_type::<ExampleComponentWithInt>()
        .register_type::<ExampleBoolComponent>()
        .register_type::<ComplexType>()
        .add_plugins(bevy_tiled_blueprints::prelude::bevy_ecs_tilemap::TilemapPlugin)
        .add_plugins(bevy_tiled_blueprints::prelude::TiledBlueprintsPlugin)
        .add_plugins(bevy_tiled_blueprints::prelude::TiledBlueprintsDebugDisplayPlugin)
        .add_systems(Startup, startup);

    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(bevy_inspector_egui::quick::WorldInspectorPlugin::new());

    app.run();
}
