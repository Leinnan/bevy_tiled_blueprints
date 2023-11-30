# Bevy Tiled Blueprints

Ability to read properties from Tiled maps objects straight into Bevy Engine.

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


## Usage

Debug rendering of Objects placement can be enabled by adding `TiledBlueprintsDebugDisplayPlugin` plugin to the application.
There is example in `examples/simple.rs`. 

---

Web example has issues with rendering but if you look into console you can see it works.