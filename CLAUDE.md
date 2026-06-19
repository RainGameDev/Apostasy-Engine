# Apostasy Engine — CLAUDE.md

Apostasy is a Rust game engine for a Morrowind/Vintage Story inspired voxel RPG. The engine targets Vulkan and is split into a workspace of focused crates.

## Workspace layout

| Crate | Purpose |
|---|---|
| `core` | Engine runtime: world, rendering, ECS, terrain, voxels, physics, UI |
| `macros` | Proc-macros: `#[derive(Component)]`, `#[derive(Resource)]`, `#[derive(Tag)]`, `#[start]`, `#[update]`, `#[fixed_update]`, `#[late_update]`, `#[prerender]` |
| `editor` | Scene editor binary — launches core in `EngineMode::Editor` |
| `game` | Game binary — launches core in `EngineMode::Game` |
| `voxel_game` | Standalone voxel-only demo binary |

Each binary crate has its own `Cargo.lock` and `res/` directory.

## Core modules (`core/src/`)

- `lib.rs` — `Core` struct (winit `ApplicationHandler`), main frame loop, `init_core` / `init_core_with_mode` entry points
- `objects/` — World, Object, Component, Tag, Resource, system registration, worldspace/scene, cells
- `rendering/` — `RenderingAPI` trait (backend abstraction), Vulkan impl, shared push constants, materials, frustum culling, lighting, shadow maps
- `terrain/` — Chunk-based heightmap terrain: atlas, mesh building, paint, persistence, texture library
- `voxels/` — 32³ voxel chunks: meshing (threaded), water mesh, texture atlas
- `physics/` — Collision (AABB), friction, inertia, raycasting
- `assets/` — `AssetManager`, glTF model loading, `ModelRegistry`, custom asset loaders (including `MaterialLoader`)
- `ui/` — egui integration, `EguiContext`, `ViewportSize`/`ViewportTexture`, font registry, profiler panel
- `packages/` — Optional feature bundles (`Packages::Voxel`, `Packages::Terrain`, `Packages::ItemSystem`)
- `states/` — `ShouldExit` and similar marker resources
- `utils/` — Profiler, misc helpers

## Object / Component / Tag / Resource system

Everything lives in `World`. The engine is not a true ECS — it stores `Object`s, each of which holds a `Vec<BoxedComponent>` and a `Vec<BoxedTag>`.

- **Component**: data + behaviour, derive `#[derive(Component)]`. The macro enforces `Clone + Send + Sync + 'static`. You must also `impl Default`, `impl Debug`, and provide a `fn deserialize(&mut self, value: &serde_yaml::Value) -> Result<()>` method (can be a no-op stub). Registered globally via `inventory`.
- **Tag**: zero-size marker, derive `#[derive(Tag)]`. Used to query objects (e.g. `ActiveCamera`, `EditorCamera`, `NeedsRemeshing`).
- **Resource**: singleton data stored on `World`, derive `#[derive(Resource)]`. The macro enforces `Clone + Send + Sync + 'static`. Access via `world.get_resource::<T>()` / `world.insert_resource(...)`.

### World query API

```rust
// Objects by component
world.get_objects_with_component::<T>()               // Vec<&Object>
world.get_objects_with_component_mut::<T>()           // Vec<&mut Object>
world.get_objects_with_component_with_ids::<T>()      // Vec<(ObjectId, &Object)>

// Objects by tag
world.get_objects_with_tag::<T>()                     // Vec<&Object>
world.get_objects_with_tag_mut::<T>()                 // Vec<&mut Object>
world.get_objects_with_tag_with_ids::<T>()            // Vec<(ObjectId, &Object)>
world.get_object_with_tag::<T>()                      // Result<&Object>  (first match)
world.get_object_with_tag_mut::<T>()                  // Result<&mut Object>

// By ID
world.get_object(id)                                  // Option<&Object>
world.get_object_mut(id)                              // Option<&mut Object>
world.add_object(object)                              // ObjectId
world.add_child_object(parent_id, object)             // Result<ObjectId>
world.remove_object(id)

// Hierarchy
world.get_children(id) / world.get_children_ids(id)
world.get_parent(id) / world.get_parent_id(id)
world.set_parent(child_id, Some(parent_id))
world.get_all_objects()                               // Vec<(ObjectId, &Object)>

// Resources
world.insert_resource(value)
world.get_resource::<T>()                             // Result<&T>
world.get_resource_mut::<T>()                         // Result<&mut T>
world.has_resource::<T>()                             // bool
world.remove_resource::<T>()
```

### Cells

The worldspace is divided into 128-unit XZ cells (infinite in Y). `ObjectId` embeds a `CellCoord` — moving an object across a cell boundary changes its ID. Most users just call `world.add_object(object)` which places into cell (0,0,0). Use `world.add_object_to_cell(coord, object)` to place into a specific cell.

### Exit

To quit the engine from a system: `world.insert_resource(ShouldExit);` — the frame loop checks this each redraw.

## System registration

Systems are registered globally at compile time through `inventory`. Decorate a free function with:

```rust
#[start]              // runs once on startup
#[prerender]          // runs before each frame's render
#[update]             // runs each frame (after prerender)
#[fixed_update]       // runs at fixed timestep (20 Hz)
#[late_update]        // runs at end of frame
```

All except `#[fixed_update]` take `fn(world: &mut World) -> Result<()>`.
`#[fixed_update]` takes `fn(world: &mut World, delta: f32) -> Result<()>` where `delta` is the fixed timestep (~0.05s at 20 Hz).

Optional args:
- `mode = "game" | "editor" | "all"` — restricts which `EngineMode` runs this system
- `priority = <u32>` — higher priority runs first

Common time resources available in systems:
- `world.get_resource::<DeltaTime>()?.0` — seconds since last frame (f32)
- `world.get_resource::<EngineTimer>()?.0` — total elapsed seconds since start (f32)

`world.build_systems()` collects and sorts all registered systems; call it before the event loop.

## Frame loop order (per `RedrawRequested`)

1. Prerender systems (`world.prerender()`)
2. Shadow pre-pass (directional CSM + point-light cubemap)
3. Viewport render (models → terrain → voxels → water)
4. Update systems (`world.update()`)
5. Fixed update systems (`world.fixed_update()`)
6. Late update systems (`world.late_update()`)
7. Profiler sample push

## Rendering pipeline

Only Vulkan is implemented. `RenderingBackend::OpenGl` is a stub.

The `RenderingAPI` trait in `core/src/rendering/mod.rs` is the backend boundary — all render calls go through it.

Viewport is rendered offscreen and composited into egui via `ViewportTexture`. The editor shows this as a dockable panel.

Shader hot-reload: insert `ReloadShadersRequest(true)` as a resource (or press F5 in editor mode). Shaders are resolved by name, e.g. `"sdr_default_terrain"` → `sdr_default_terrain.vert` / `sdr_default_terrain.frag`.

Material overrides: `ModelRenderer.material_override` holds a material name. Resolved first from loaded glTF meshes, then from YAML materials via `MaterialLoader`.

## Packages

Packages are feature bundles added at startup:

- `Packages::Voxel` — voxel chunk pipeline (remeshing, water, texture atlas)
- `Packages::Terrain` — heightmap terrain (chunk mesh, texture atlas, paint tools)
- `Packages::ItemSystem` — item inventory system

The core frame loop guards voxel/terrain render passes with `self.packages.contains(...)`.

## EngineMode

- `EngineMode::Game` — default; runs game systems only
- `EngineMode::Editor` — runs editor systems; camera selection prefers `EditorCamera` over `ActiveCamera`
- `EngineMode::All` — system runs in both modes

## Assets

`AssetManager` lives as a resource on `World`. Models are loaded from:
1. `res/` relative to the CWD (app assets)
2. `{CARGO_MANIFEST_DIR}/res/` (core/editor built-in assets)

glTF files in `res/` are auto-discovered and loaded into `ModelRegistry`. Reference them in `ModelRenderer.model_path`.

## Built-in components and resources

### Components (in `core`)

| Component | Location | Purpose |
|---|---|---|
| `Transform` | `objects/components/transform.rs` | Position/rotation/scale; local fields set manually, global fields derived each frame |
| `ModelRenderer` | `rendering/components/model_renderer.rs` | Renders a glTF model; set `model_path`, optionally `material_override` |
| `Camera` | `rendering/components/camera.rs` | Perspective camera; pair with `ActiveCamera` tag |
| `Light` | `rendering/components/lighting.rs` | `LightType::Directional`, `Point { radius }`, `Spot { length, angle }` |
| `Collider` | `physics/collider.rs` | Shapes: `Cuboid`, `Sphere`, `Capsule`, `Cylinder`, `Mesh` |
| `Velocity` | `physics/velocity.rs` | Linear velocity (integrated by physics system) |
| `VoxelTransform` | `voxels/mod.rs` | Chunk-space position (in 32-unit chunks) for voxel objects |
| `TerrainChunk` | `terrain/chunk.rs` | Data for a heightmap terrain chunk |

### Key tags

| Tag | Meaning |
|---|---|
| `ActiveCamera` | Camera used for rendering |
| `EditorCamera` | Marks the editor's fly camera; takes priority over `ActiveCamera` in editor mode |
| `NeedsRemeshing` | Marks a voxel chunk that needs its mesh rebuilt |
| `NeedsTerrainRebuild` | Marks a terrain chunk that needs its mesh rebuilt |

### Key resources (always present)

| Resource | Type | Notes |
|---|---|---|
| `DeltaTime` | `f32` | Seconds since last frame |
| `EngineTimer` | `f32` | Total elapsed seconds |
| `InputManager` | struct | Keybind queries (`is_keybind_active`, `is_key_pressed`, mouse delta) |
| `WindowInfo` | struct | Current window size as `(f32, f32)` |
| `AntiAliasing` | struct | Current AA setting |
| `ShadowDistance` | struct | Shadow distance, cascade count, bias |
| `AssetManager` | struct | Model/shader/texture loading |
| `EguiContext` | wrapper | egui `Context` for UI rendering |
| `ViewportSize` | struct | Pixel dimensions of the editor viewport |
| `ViewportTexture` | wrapper | egui `TextureId` of the offscreen viewport image |

## Editor

The editor binary (`editor/src/main.rs`) uses `EngineMode::Editor` + `Packages::ItemSystem` + `Packages::Terrain`. Editor-specific systems are in `editor/src/systems/`, UI panels in `editor/src/ui/`.

Keybinds are saved/loaded from `res/.editor/keybinds.yaml`.

## Build & run

```sh
# editor
cargo run --manifest-path editor/Cargo.toml

# voxel demo
cargo run --manifest-path voxel_game/Cargo.toml

# game
cargo run --manifest-path game/Cargo.toml
```

Requires a Vulkan-capable GPU and drivers. Rust stable, edition 2024.
