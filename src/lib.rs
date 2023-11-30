use core::ops::Deref;
use std::io::{Cursor, ErrorKind};
use std::path::Path;
use std::sync::Arc;

use bevy::core::Name;
use bevy::ecs::query::With;
use bevy::ecs::reflect::{AppTypeRegistry, ReflectComponent};
use bevy::ecs::schedule::IntoSystemConfigs;
use bevy::ecs::world::World;
use bevy::math::Vec3;
use bevy::reflect::Reflect;
use bevy::{
    asset::{io::Reader, AssetLoader, AssetPath, AsyncReadExt},
    log,
    prelude::*,
    reflect::{serde::UntypedReflectDeserializer, TypePath, TypeRegistry},
    utils::{BoxedFuture, HashMap},
};
use bevy_ecs_tilemap::prelude::*;
use serde::de::DeserializeSeed;

use thiserror::Error;

pub mod debug;

pub mod prelude {
    pub use super::{
        RemoveMap, TiledBlueprintsPlugin, debug::TiledBlueprintsDebugDisplayPlugin, TiledLayersStorage, TiledMap, TiledMapBundle,
    };
    pub use bevy_ecs_tilemap;
}

pub struct TiledBlueprintsPlugin;

impl Plugin for TiledBlueprintsPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.init_asset::<TiledMap>()
            .register_asset_loader(TiledLoader)
            .register_type::<RemoveMap>()
            .register_type::<MapObject>()
            .register_type::<TiledLayersStorage>()
            .add_systems(Update, (process_loaded_maps, cleanup_maps).chain());
    }
}

#[derive(TypePath, Asset)]
pub struct TiledMap {
    pub map: tiled::Map,

    pub tilemap_textures: HashMap<usize, TilemapTexture>,

    /// The offset into the tileset_images for each tile id within each tileset.
    #[cfg(not(feature = "atlas"))]
    pub tile_image_offsets: HashMap<(usize, tiled::TileId), u32>,
}

/// Stores a list of tiled layers.
#[derive(Debug, Reflect, Component, Default, Clone)]
#[reflect(Component)]
pub struct TiledLayersStorage {
    pub storage: HashMap<u32, Entity>,
}

#[derive(Default, Bundle)]
pub struct TiledMapBundle {
    pub tiled_map: Handle<TiledMap>,
    pub storage: TiledLayersStorage,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
}

#[derive(Debug, Reflect, Component, Default, Clone)]
#[reflect(Component)]
pub struct RemoveMap;

#[derive(Debug, Reflect, Component, Default, Clone)]
#[reflect(Component)]
pub struct MapObject;

struct BytesResourceReader {
    bytes: Arc<[u8]>,
}

impl BytesResourceReader {
    fn new(bytes: &[u8]) -> Self {
        Self {
            bytes: Arc::from(bytes),
        }
    }
}

impl tiled::ResourceReader for BytesResourceReader {
    type Resource = Cursor<Arc<[u8]>>;
    type Error = std::io::Error;

    fn read_from(&mut self, _path: &Path) -> std::result::Result<Self::Resource, Self::Error> {
        // In this case, the path is ignored because the byte data is already provided.
        Ok(Cursor::new(self.bytes.clone()))
    }
}

pub struct TiledLoader;

#[derive(Debug, Error)]
pub enum TiledAssetLoaderError {
    /// An [IO](std::io) Error
    #[error("Could not load Tiled file: {0}")]
    Io(#[from] std::io::Error),
}

impl AssetLoader for TiledLoader {
    type Asset = TiledMap;
    type Settings = ();
    type Error = TiledAssetLoaderError;

    fn load<'a>(
        &'a self,
        reader: &'a mut Reader,
        _settings: &'a Self::Settings,
        load_context: &'a mut bevy::asset::LoadContext,
    ) -> BoxedFuture<'a, Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;

            let mut loader = tiled::Loader::with_cache_and_reader(
                tiled::DefaultResourceCache::new(),
                BytesResourceReader::new(&bytes),
            );
            let map = loader.load_tmx_map(load_context.path()).map_err(|e| {
                std::io::Error::new(ErrorKind::Other, format!("Could not load TMX map: {e}"))
            })?;

            let mut dependencies = Vec::new();
            let mut tilemap_textures = HashMap::default();
            #[cfg(not(feature = "atlas"))]
            let mut tile_image_offsets = HashMap::default();

            for (tileset_index, tileset) in map.tilesets().iter().enumerate() {
                let tilemap_texture = match &tileset.image {
                    None => {
                        #[cfg(feature = "atlas")]
                        {
                            log::info!("Skipping image collection tileset '{}' which is incompatible with atlas feature", tileset.name);
                            continue;
                        }

                        #[cfg(not(feature = "atlas"))]
                        {
                            let mut tile_images: Vec<Handle<Image>> = Vec::new();
                            for (tile_id, tile) in tileset.tiles() {
                                if let Some(img) = &tile.image {
                                    // The load context path is the TMX file itself. If the file is at the root of the
                                    // assets/ directory structure then the tmx_dir will be empty, which is fine.
                                    let tmx_dir = load_context
                                        .path()
                                        .parent()
                                        .expect("The asset load context was empty.");
                                    let tile_path = tmx_dir.join(&img.source);
                                    let asset_path = AssetPath::from(tile_path);
                                    log::info!("Loading tile image from {asset_path:?} as image ({tileset_index}, {tile_id})");
                                    let texture: Handle<Image> =
                                        load_context.load(asset_path.clone());
                                    tile_image_offsets
                                        .insert((tileset_index, tile_id), tile_images.len() as u32);
                                    tile_images.push(texture.clone());
                                    dependencies.push(asset_path);
                                }
                            }

                            TilemapTexture::Vector(tile_images)
                        }
                    }
                    Some(img) => {
                        // The load context path is the TMX file itself. If the file is at the root of the
                        // assets/ directory structure then the tmx_dir will be empty, which is fine.
                        let tmx_dir = load_context
                            .path()
                            .parent()
                            .expect("The asset load context was empty.");
                        let tile_path = tmx_dir.join(&img.source);
                        let asset_path = AssetPath::from(tile_path);
                        let texture: Handle<Image> = load_context.load(asset_path.clone());
                        dependencies.push(asset_path);

                        TilemapTexture::Single(texture.clone())
                    }
                };

                tilemap_textures.insert(tileset_index, tilemap_texture);
            }

            let asset_map = TiledMap {
                map,
                tilemap_textures,
                #[cfg(not(feature = "atlas"))]
                tile_image_offsets,
            };

            log::info!("Loaded map: {}", load_context.path().display());
            Ok(asset_map)
        })
    }

    fn extensions(&self) -> &[&str] {
        static EXTENSIONS: &[&str] = &["tmx"];
        EXTENSIONS
    }
}

pub fn cleanup_maps(mut commands: Commands, q: Query<Entity, With<RemoveMap>>) {
    for e in q.iter() {
        commands.entity(e).despawn_recursive();
    }
}

pub fn process_loaded_maps(
    mut commands: Commands,
    mut map_events: EventReader<AssetEvent<TiledMap>>,
    maps: Res<Assets<TiledMap>>,
    tile_storage_query: Query<(Entity, &TileStorage)>,
    mut map_query: Query<(&Handle<TiledMap>, &mut TiledLayersStorage, Entity)>,
    new_maps: Query<&Handle<TiledMap>, Added<Handle<TiledMap>>>,
    type_registry: Res<AppTypeRegistry>,
) {
    let mut changed_maps = Vec::<AssetId<TiledMap>>::default();
    for event in map_events.read() {
        match event {
            AssetEvent::Added { id } => {
                log::info!("Map added!");
                changed_maps.push(*id);
            }
            AssetEvent::Modified { id } => {
                log::info!("Map changed!");
                changed_maps.push(*id);
            }
            AssetEvent::Removed { id } => {
                log::info!("Map removed!");
                // if mesh was modified and removed in the same update, ignore the modification
                // events are ordered so future modification events are ok
                changed_maps.retain(|changed_handle| changed_handle == id);
            }
            _ => continue,
        }
    }

    // If we have new map entities add them to the changed_maps list.
    for new_map_handle in new_maps.iter() {
        changed_maps.push(new_map_handle.id());
    }

    for changed_map in changed_maps.iter() {
        for (map_handle, mut layer_storage, map_entity) in map_query.iter_mut() {
            // only deal with currently changed map
            if map_handle.id() != *changed_map {
                continue;
            }
            if let Some(tiled_map) = maps.get(map_handle) {
                for layer_entity in layer_storage.storage.values() {
                    if let Ok((_, layer_tile_storage)) = tile_storage_query.get(*layer_entity) {
                        for tile in layer_tile_storage.iter().flatten() {
                            commands.entity(*tile).despawn_recursive()
                        }
                    }
                    commands.entity(*layer_entity).insert(RemoveMap);
                    // commands.entity(*layer_entity).despawn_recursive();
                }

                // The TilemapBundle requires that all tile images come exclusively from a single
                // tiled texture or from a Vec of independent per-tile images. Furthermore, all of
                // the per-tile images must be the same size. Since Tiled allows tiles of mixed
                // tilesets on each layer and allows differently-sized tile images in each tileset,
                // this means we need to load each combination of tileset and layer separately.
                for (tileset_index, tileset) in tiled_map.map.tilesets().iter().enumerate() {
                    let Some(tilemap_texture) = tiled_map.tilemap_textures.get(&tileset_index)
                    else {
                        log::warn!("Skipped creating layer with missing tilemap textures.");
                        continue;
                    };

                    let tile_size = TilemapTileSize {
                        x: tileset.tile_width as f32,
                        y: tileset.tile_height as f32,
                    };

                    let tile_spacing = TilemapSpacing {
                        x: tileset.spacing as f32,
                        y: tileset.spacing as f32,
                    };

                    let map_size = TilemapSize {
                        x: tiled_map.map.width,
                        y: tiled_map.map.height,
                    };

                    let grid_size = TilemapGridSize {
                        x: tiled_map.map.tile_width as f32,
                        y: tiled_map.map.tile_height as f32,
                    };

                    let map_type = match tiled_map.map.orientation {
                        tiled::Orientation::Hexagonal => TilemapType::Hexagon(HexCoordSystem::Row),
                        tiled::Orientation::Isometric => {
                            TilemapType::Isometric(IsoCoordSystem::Diamond)
                        }
                        tiled::Orientation::Staggered => {
                            TilemapType::Isometric(IsoCoordSystem::Staggered)
                        }
                        tiled::Orientation::Orthogonal => TilemapType::Square,
                    };

                    // Once materials have been created/added we need to then create the layers.
                    for (layer_index, layer) in tiled_map.map.layers().enumerate() {
                        let offset_x = layer.offset_x;
                        let offset_y = layer.offset_y;
                        let center = get_tilemap_center_transform(
                            &map_size,
                            &grid_size,
                            &map_type,
                            layer_index as f32,
                        ) * Transform::from_xyz(offset_x, -offset_y, -1.0);
                        let layer_world_size = center.translation.abs() * 2.0;
                        let layer_entity = commands
                            .spawn(Name::new(format!("Layer-{}", layer.name)))
                            .insert(TransformBundle::from_transform(center))
                            .set_parent(map_entity)
                            .id();
                        if let tiled::LayerType::Objects(obj_layer) = layer.layer_type() {
                            let type_registry = type_registry.read();
                            for obj in obj_layer.objects() {

                                let pos = Vec3::new(obj.x, -obj.y + layer_world_size.y, 0.0);
                                let name = Name::new(
                                    if obj.name.is_empty() {
                                        "Object".to_string()
                                    } else {
                                        obj.name.clone()
                                    });
                                let e = commands
                                    .spawn((
                                        name,
                                        TransformBundle::from_transform(
                                            Transform::from_translation(pos),
                                        ),
                                        MapObject,
                                    ))
                                    .set_parent(layer_entity)
                                    .id();
                                add_properties(&obj.properties, e, &type_registry, &mut commands);
                            }

                            layer_storage
                                .storage
                                .insert(layer_index as u32, layer_entity);
                            continue;
                        }
                        let tiled::LayerType::Tiles(tile_layer) = layer.layer_type() else {
                            log::info!(
                                "Skipping layer {} because only tile layers are supported.",
                                layer.id()
                            );
                            continue;
                        };

                        let tiled::TileLayer::Finite(layer_data) = tile_layer else {
                            log::info!(
                                "Skipping layer {} because only finite layers are supported.",
                                layer.id()
                            );
                            continue;
                        };

                        let mut tile_storage = TileStorage::empty(map_size);

                        for x in 0..map_size.x {
                            for y in 0..map_size.y {
                                // Transform TMX coords into bevy coords.
                                let mapped_y = tiled_map.map.height - 1 - y;

                                let mapped_x = x as i32;
                                let mapped_y = mapped_y as i32;

                                let layer_tile = match layer_data.get_tile(mapped_x, mapped_y) {
                                    Some(t) => t,
                                    None => {
                                        continue;
                                    }
                                };
                                if tileset_index != layer_tile.tileset_index() {
                                    continue;
                                }
                                let layer_tile_data =
                                    match layer_data.get_tile_data(mapped_x, mapped_y) {
                                        Some(d) => d,
                                        None => {
                                            continue;
                                        }
                                    };

                                let texture_index = match tilemap_texture {
                                    TilemapTexture::Single(_) => layer_tile.id(),
                                    #[cfg(not(feature = "atlas"))]
                                    TilemapTexture::Vector(_) =>
                                        *tiled_map.tile_image_offsets.get(&(tileset_index, layer_tile.id()))
                                        .expect("The offset into to image vector should have been saved during the initial load."),
                                    #[cfg(not(feature = "atlas"))]
                                    _ => unreachable!()
                                };

                                let tile_pos = TilePos { x, y };
                                let tile_entity = commands
                                    .spawn((
                                        TileBundle {
                                            position: tile_pos,
                                            tilemap_id: TilemapId(layer_entity),
                                            texture_index: TileTextureIndex(texture_index),
                                            flip: TileFlip {
                                                x: layer_tile_data.flip_h,
                                                y: layer_tile_data.flip_v,
                                                d: layer_tile_data.flip_d,
                                            },
                                            ..Default::default()
                                        },
                                        Name::new(format!("tile-{}x{}", x, y)),
                                    ))
                                    .id();
                                commands.entity(tile_entity).set_parent(layer_entity);
                                tile_storage.set(&tile_pos, tile_entity);
                            }
                        }

                        commands.entity(layer_entity).insert(TilemapBundle {
                            grid_size,
                            size: map_size,
                            storage: tile_storage,
                            texture: tilemap_texture.clone(),
                            tile_size,
                            spacing: tile_spacing,
                            transform: get_tilemap_center_transform(
                                &map_size,
                                &grid_size,
                                &map_type,
                                layer_index as f32,
                            ) * Transform::from_xyz(offset_x, -offset_y, 0.0),
                            map_type,
                            ..Default::default()
                        });

                        layer_storage
                            .storage
                            .insert(layer_index as u32, layer_entity);
                    }
                }
            }
        }
    }
}

fn add_properties(
    properties: &std::collections::HashMap<String, tiled::PropertyValue>,
    e: Entity,
    type_registry: &impl Deref<Target = TypeRegistry>,
    commands: &mut Commands,
) {
    for (k, value) in properties.iter() {
        if let Some(type_registration) = type_registry.get_with_short_type_path(k) {
            let parsed_value = match value {
                tiled::PropertyValue::BoolValue(b) => b.to_string(),
                tiled::PropertyValue::FloatValue(f) => f.to_string(),
                tiled::PropertyValue::IntValue(i) => i.to_string(),
                tiled::PropertyValue::StringValue(s) => s.to_string(),
                // tiled::PropertyValue::ColorValue(_) => todo!(),
                // tiled::PropertyValue::FileValue(_) => todo!(),
                // tiled::PropertyValue::ObjectValue(_) => todo!(),
                _ => "".to_string(),
            }
            .trim()
            .to_string();
            let matches = (parsed_value.starts_with('('), parsed_value.ends_with(')'));
            let type_path = type_registration.type_info().type_path();

            let ron_string = match matches {
                (true, true) => format!("{{ \"{}\":{} }}", type_path, parsed_value),
                (false, false) => format!("{{ \"{}\":({}) }}", type_path, parsed_value),
                _ => {
                    log::error!("Failed to deserialize component {}: {}", k, parsed_value);
                    continue;
                }
            };

            let mut deserializer = ron::de::Deserializer::from_str(&ron_string).unwrap();
            let reflect_deserializer = UntypedReflectDeserializer::new(type_registry);
            let component = reflect_deserializer
                .deserialize(&mut deserializer)
                .unwrap_or_else(|_| {
                    panic!("Failed to deserialize component {}: {}", k, parsed_value)
                });
            let result = type_registry
                .get(type_registration.type_id())
                .unwrap()
                .data::<ReflectComponent>()
                .unwrap()
                .clone();

            commands.add(move |world: &mut World| {
                let mut entity_mut = world.entity_mut(e);
                result.insert(&mut entity_mut, &*component);
            });
            log::info!("Added {}", type_registration.type_info().type_path())
        }
    }
}