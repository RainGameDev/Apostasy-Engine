# Apostasy Engine

Apostasy is a Rust game engine built specifically for the game *Apostasy*, A *Morrowind* and *Vintage Story* inspired voxel game developed with 1990s and 2000s era design goals. The engine is designed to support open-world RPG systems, Voxel Worlds and scene-driven Game Object gameplay whilst staying data-driven.

The engine supports both traditional 3D games and voxel-based games, featuring a complete Entity Component System (ECS) with extensive capabilities for both approaches.

## What the engine does:

- **Entity Component System (ECS)**: Objects, components, and tags system for game entities
- **Physics System**: Collision detection, velocity, gravity, and physics queries
- **Rendering**: Vulkan-based rendering with model rendering and camera systems
- **UI System**: Egui-based user interface with viewport rendering
- **Asset Management**: Custom asset loaders for voxels, materials, structures, and items
- **Update Systems**: Start, update, fixed update, and late update systems
- **Input Management**: Keyboard and mouse input handling with bindings
- **Voxel Capabilities**: Complete voxel world with chunks, biomes, structures, and terrain generation
- **Item System**: Item containers, voxel components, and inventory management
- **Terrain Tools**: Structure building, heightmap editing, and terrain manipulation
- **World Streaming**: Dynamic chunk loading and world streaming for large open worlds

## How to use it:

The engine can be launched via this code:

```rust
use apostasy_core::{init_core, packages::Packages, rendering::RenderingBackend};

fn main() {
    init_core(
        RenderingBackend::Vulkan,
        vec![Packages::Voxel, Packages::ItemSystem],
    )
    .unwrap();
}
```

Packages provide different system bundles:
- `Packages::Voxel`: Voxel world and terrain systems
- `Packages::ItemSystem`: Item and inventory management systems

You can use any combination of packages based on your game's needs. For traditional 3D games, you might only need basic rendering and physics. For voxel games, you'll want the Voxel package. For RPG-style games, you'll want both.

### Objects and Components:

Apostasy runs off an entity and component system, entities are defined with a name, id, set of components and a set of tags.
Components are a set of data similar to the average ECS data, tags are tags theyre empty components that are used to find specific entities

Entities can be created via the following code:

```rust
let player = Object::new()
    .add_component(transform)
    .add_component(Velocity::default())
    .add_component(Gravity::default())
    .add_component(Collider::player())
    .add_tag(Player);

world.add_object(player);
```

This creates an entity (`player`) and then adds it to the world.
Entities can be read with `world.get_object(id)` or modified via `world.get_object_mut(id)`.

## Current capabilities:

Apostasy is a fully functional engine with extensive capabilities:

- **ECS**: Complete entity-component-system with queries and tags
- **Rendering**: Vulkan backend with model rendering and camera controls
- **Physics**: Realistic physics with collision detection and response
- **UI System**: Egui-based interface with viewport rendering
- **Asset Pipeline**: Custom loaders for voxels, materials, structures, and items
- **Update Systems**: Start, update, fixed update, and late update systems
- **Input Management**: Keyboard and mouse input handling with bindings
- **Voxel World**: Complete voxel terrain with chunk-based rendering and modification
- **Biomes**: Environment-specific voxel generation with unique properties
- **Structures**: Build and save voxel structures for world decoration
- **Items**: Full item system with containers, stacks, and voxel components
- **Terrain Tools**: Heightmap editing, structure placement, and terrain manipulation
- **World Streaming**: Dynamic chunk loading for seamless large worlds

## Requirements

- Rust toolchain (stable, edition 2024)
- Vulkan-capable system and drivers
- `cargo` available on PATH

## Example Voxel World Features:

The engine includes a complete voxel game implementation with:

- **Chunk-based world**: Efficient rendering and modification of large voxel worlds
- **Biome system**: Different environment types with unique generation rules
- **Structure building**: Place and save voxel structures for world decoration
- **Item system**: Pick up, carry, and use items with voxel components
- **Physics interactions**: Realistic physics for voxel manipulation and movement
- **Terrain editing**: Modify terrain with tools like heightmaps and structure placement
- **World streaming**: Load and unload chunks dynamically for performance

While the engine excels at voxel games, it also supports traditional 3D games with its complete ECS, physics, rendering, and UI systems.

## Packages

The engine includes several system packages:

- **Voxel Package**: Core voxel world, chunk loading, biomes, and terrain generation
- **Item System Package**: Item management, containers, and inventory systems

These packages can be combined when initializing the engine to get the desired functionality. For traditional 3D games, you might only need basic rendering and physics. For voxel games, you'll want the Voxel package. For RPG-style games, you'll want both.
