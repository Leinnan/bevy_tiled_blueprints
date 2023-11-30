# Bevy Tiled Blueprints

Ability to read properties from Tiled maps objects straight into Bevy Engine.

> I am waiting with release on crates.io until [this PR](https://github.com/StarArawn/bevy_ecs_tilemap/pull/489) gets merged so Cargo.toml can have dependency on specific version from crates.io instead of git branch from other repo. 

![simple example](simple_example.png)

![Tiled example](simple_example_tiled.png)

Think like:

| Tiled | Bevy |
|-----|----|
| Object | Entity |
| object.name | Name component |
| Custom property | Component |
| Custom property name | Component struct name |
| Custom property value | Component serialized in ron format |

Supported custom property values:
- empty for unit-like structs without any fields
- int/bool/float for tuple structs with one unnamed fields
- [ron](https://github.com/ron-rs/ron) strings for regular structs  

Custom properties added to the layer or the map itself would be added in the same way to the corresponding entities.

## Usage

Debug rendering of Objects placement can be enabled by adding `TiledBlueprintsDebugDisplayPlugin` plugin to the application.
There is example in `examples/simple.rs`. 

---

Big thanks to the authors of [bevy_ecs_tilemap](https://github.com/StarArawn/bevy_ecs_tilemap), this whole thing is based on expanding the tiled example from there.

---

Web example has issues with rendering but if you look into console you can see it works.