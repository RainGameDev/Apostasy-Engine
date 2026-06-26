# Apostasy Engine

Apostasy is a Rust game engine built for a _Morrowind_ and _Vintage Story_ inspired voxel RPG. It targets Vulkan and is designed around open-world RPG systems, voxel worlds, and scene-driven gameplay while staying data-driven.

## What the engine does

- Windowing and render loops via winit
- Shared `World` containing entities, components, tags, and resources
- System registration at compile time (`#[start]`, `#[update]`, `#[fixed_update]`, `#[late_update]`, `#[prerender]`)
- Asset loading including glTF models, YAML materials, and custom asset loaders
- Vulkan rendering backend with offscreen viewport, shadow maps, and frustum culling
- Chunk-based heightmap terrain and 32³ voxel chunks with threaded meshing
- Physics: AABB collision, raycasting, velocity integration
- Lua scripting with hot-reload, custom components/resources, and full world API
- egui-based editor UI with dockable viewport

## Workspace layout

| Crate        | Purpose                                                                                         |
| ------------ | ----------------------------------------------------------------------------------------------- |
| `core`       | Engine runtime: ECS, rendering, terrain, voxels, physics, UI                                    |
| `macros`     | Proc-macros: `#[derive(Component)]`, `#[derive(Resource)]`, `#[derive(Tag)]`, system attributes |
| `editor`     | Scene editor binary                                                                             |
| `game`       | Game binary                                                                                     |
| `voxel_game` | Standalone voxel demo binary                                                                    |
| `script_api` | Rust library for scripts compiled to WASM (separate from Lua scripting)                         |

## Getting started

```rust
fn main() {
    init_core(
        RenderingBackend::Vulkan,
        vec![Packages::Voxel, Packages::Terrain],
    )
    .unwrap();
}
```

Packages are optional feature bundles (`Packages::Voxel`, `Packages::Terrain`, `Packages::ItemSystem`) that register their own startup systems and resources.

## Entities and Components

The engine uses an ECS-style world. Entities are spawned via `world.spawn()` and identified by a stable `EntityId`.

```rust
let id = world.spawn()
    .with_component(Transform::default())
    .with_component(Velocity::default())
    .with_component(Collider::player())
    .with_tag(Player)
    .build();
```

- **Component** — data attached to an entity; derive `#[derive(Component)]`
- **Tag** — zero-size marker for querying; derive `#[derive(Tag)]`
- **Resource** — singleton data on the world; derive `#[derive(Resource)]`

```rust
// Query entities
let ids = world.get_entities_with_component::<Transform>();
let cam = world.get_entity_with_tag::<ActiveCamera>()?;

// Access components
let t = world.get_component::<Transform>(id);
let t = world.get_component_mut::<Transform>(id);

// Resources
world.insert_resource(ShouldExit);
let dt = world.get_resource::<DeltaTime>()?.0;
```

## Systems

Register systems globally at compile time:

```rust
#[start]
fn setup(world: &mut World) -> Result<()> { ... }

#[update]
fn tick(world: &mut World) -> Result<()> { ... }

#[fixed_update]
fn physics(world: &mut World, delta: f32) -> Result<()> { ... }
```

Optional attributes: `mode = "game" | "editor" | "all"`, `priority = <u32>`.

## Lua Scripting

Apostasy supports runtime Lua scripting via [mlua](https://github.com/mlua-rs/mlua). Scripts are plain `.lua` files placed in `res/scripts/` and loaded automatically at startup. The engine hot-reloads any script whose file changes on disk each frame.

Scripts declare top-level functions that the engine calls at the appropriate lifecycle stage:

```lua
-- res/scripts/my_script.lua

-- Declare a custom Lua-side component with default fields.
register_component("Orbit", { radius = 5.0, speed = 30.0, angle = 0.0 })

-- Declare a Lua-side global resource.
register_resource("Config", { paused = false })

function start(world)
    local e = world:spawn()
    world:set_name(e, "Planet")
    world:add_component(e, "Transform", { local_position = { 6, 0, 0 } })
    world:add_component(e, "Orbit", { radius = 6.0, speed = 40.0 })
end

function update(world)
    local dt = world:delta()
    world:query("Orbit"):for_each(function(id, orbit)
        orbit.angle = (orbit.angle + orbit.speed * dt) % 360.0
        local rot = quat.from_axis_angle(vec3.up, orbit.angle)
        world:set_component(id, "Transform", { local_position = rot:rotate(vec3(orbit.radius, 0, 0)) })
        world:set_component(id, "Orbit", orbit)
    end)
end

function fixed_update(world, delta) end
function late_update(world) end
function prerender(world) end
```

### World API (available on the `world` argument)

| Method                                          | Description                                          |
| ----------------------------------------------- | ---------------------------------------------------- |
| `world:spawn()`                                 | Spawn an entity, returns an Entity handle            |
| `world:spawn_at_position(vec3)`                 | Spawn into the cell containing a position            |
| `world:despawn(entity)`                         | Remove an entity                                     |
| `world:set_name(entity, name)`                  | Set entity display name                              |
| `world:get_name(entity)`                        | Get entity display name                              |
| `world:add_component(entity, "Name", {fields})` | Add a component with field values                    |
| `world:set_component(entity, "Name", {fields})` | Update component fields                              |
| `world:get_component(entity, "Name")`           | Read component fields as a table                     |
| `world:add_tag(entity, "TagName")`              | Add a tag                                            |
| `world:has_tag(entity, "TagName")`              | Check if entity has a tag                            |
| `world:set_parent(entity, parent)`              | Set parent entity                                    |
| `world:get_children(entity)`                    | List of child entities                               |
| `world:get_ancestors(entity)`                   | List of ancestor entities                            |
| `world:get_all_entities()`                      | All entity handles in the world                      |
| `world:query("ComponentName")`                  | Returns a query; call `:for_each(fn(id, data))`      |
| `world:raycast(origin, dir, max_dist, exclude)` | Raycast; returns `{entity, distance, normal}` or nil |
| `world:get_resource("Name")`                    | Get a Lua resource table                             |
| `world:set_resource("Name", table)`             | Update a Lua resource                                |
| `world:set_material_color("MatName", {r,g,b})`  | Override a material's color at runtime               |
| `world:log(msg)`                                | Print to the engine log                              |
| `world:delta()`                                 | Seconds since last frame                             |
| `world:time()`                                  | Total elapsed engine time in seconds                 |

### Math globals

The prelude exposes `vec2`, `vec3`, `vec4`, and `quat` as global constructors with operator overloading (`+`, `-`, `*`, `/`, unary `-`).

```lua
local p = vec3(1, 0, 0)
local q = quat.from_axis_angle(vec3.up, 45.0)
local rotated = q:rotate(p)
local d = vec3.forward:dot(vec3.up)  -- 0
```

| Global | Constants / helpers                                                                                                                    |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------- |
| `vec3` | `.zero`, `.one`, `.up`, `.down`, `.forward`, `.back`, `.left`, `.right`; `:length()`, `:normalized()`, `:dot()`, `:cross()`, `:lerp()` |
| `quat` | `.identity()`, `.from_axis_angle(axis, degrees)`, `.from_euler(x, y, z)`; `:rotate(vec3)`, `:normalized()`, `:inverse()`               |
| `math` | Extended with `math.clamp`, `math.lerp`, `math.sign`                                                                                   |

## Build & run

```sh
# Editor
cargo run --manifest-path editor/Cargo.toml

# Voxel demo
cargo run --manifest-path voxel_game/Cargo.toml

# Game
cargo run --manifest-path game/Cargo.toml
```

## Requirements

- Rust stable, edition 2024
- Vulkan-capable GPU and drivers
- `cargo` on PATH

## Current limitations

Apostasy is not a finished engine:

- Limited shader variety
- No audio system
- Editor is still early-stage
- Performance is not yet optimized
