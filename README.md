# Apostasy Engine

Apostasy is a Rust game engine built specifically for the game *Apostasy*, A *Morrowind* and *Vintage Story* inspired voxel game developed with 1990s and 2000s era design goals. The engine is designed to support open-world RPG systems, Voxel Worlds and scene-driven Game Object gameplay whilst staying data-driven.

## What the engine does:

- windowing and render loops 
- maintains a shared `World` containing objects 
- update, start, fixed update and late update systems 
- loading of assets, including custom asset loaders 
- renders content through a Vulkan backend (soon to support other backends)

The engine is split into several parts, the Core and Macros support the base of engine management, rendering, voxels and scenes, Game is customisable to what you need and Editor will be a full scene based editor.

## How to use it:

The base of the engine can be launched via this code

```rust

fn main() {
    init_core(
        RenderingBackend::Vulkan,
        vec![*packages*],
    )
    .unwrap();
}
```

Packages can be empty or have different defined packages, packages contain a bunch of startup commands to help clean up startup code.


### Objects and Components: 

Apostasy runs off an object and component system, objects are defined with a name, id, set of components and a set of tags.
Components are a set of data similar to the average ECS data, tags are tags theyre empty components that are used to find specific objects

Objects can be created via the following code:

```rust


let player = Object::new()
    .add_component(transform)
    .add_component(Velocity::default())
    .add_component(Gravity::default())
    .add_component(Collider::player())
    .add_tag(Player);

world.add_object(player);
```

this creates an object (`player`) and then adds it to the world.
Objects can be read with `world.get_object(id)` or modified via `world.get_object_mut(id)`.

## Current limitations

Apostasy is not a finished engine. Existing limitations include:
- limited shader support,
- limited rendering capabilities
- performance
- ease of use
- an in built editor

## Requirements

- Rust toolchain (stable, edition 2024)
- Vulkan-capable system and drivers
- `cargo` available on PATH

