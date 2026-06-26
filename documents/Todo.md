# Engine

- [x] World
  - [x] Ability to access renderer in systems
  - [x] Manual system registration
  - [x] Scene hierarchy (parent/child entities)
  - [x] Tags system
  - [x] States system

- [x] Rendering
  - [x] Render settings
  - [x] Anti-aliasing
  - [x] Frustum culling
  - [x] Shader registry and loader
  - [x] GLTF / model loading
  - [x] Lighting
    - [x] Phase 1 - Forward Lighting (no shadows)
    - [x] Phase 2 - Shadow maps (directional/sun)
    - [x] Phase 3 - Point / spot shadows
    - [ ] Ambient effects
      - [ ] Ambient lighting
      - [ ] Fog
  - [ ] Particle system
  - [x] GLTF loading textures
  - [ ] Custom material shaders

- [-] Systems
  - [x] Prerender systems
  - [ ] Render systems

- [x] Physics
  - [x] Basic collisions
    - [x] Entity resolution
    - [x] Entity shapes
    - [x] Offsets
  - [x] Friction
  - [x] Inertia
  - [x] Raycasting

- [x] Scene serialization
  - [x] Save scene to file
  - [x] Load scene from file

- [ ] Audio system

# Editor

- [-] Editor UI
  - [x] Top bar menu
  - [x] Inspector
    - [x] Entities displaying
    - [x] Entity selection
    - [x] Entity deletion
    - [x] Entity creation
  - [-] Entity Editor
    - [x] Name bar (inline rename)
    - [x] Component adding
    - [x] Component editing
    - [x] Component removal
    - [x] Component copy/paste/cut
    - [x] Tag adding
    - [x] Tag removal
  - [x] Assets panel (loaded assets display)
  - [x] Viewport panel
  - [x] Cell / Entities panel
  - [x] Preferences panel
    - [x] Font loading and themes
    - [x] Camera speed setting
    - [x] Graphics settings (AA, supersampling)
    - [x] Saving and loading preferences
  - [-] Scenes panel
    - [x] Scene creation
    - [ ] Scene deletion
    - [x] Scene switching
  - [ ] UI Editor
  - [ ] Dockable Panels

- [-] Editor Camera
  - [x] Camera rotation
  - [x] Entity focusing
  - [x] Camera speed
  - [ ] Save the cameras position and rotate

- [x] Editor systems
  - [x] Copy/paste entities (keybinds)
  - [x] Raycasting (viewport entity selection)
  - [x] Layout saving and loading

- [x] Viewport Gizmos
  - [x] Translate handle
  - [x] Rotate handle
  - [x] Scale handle
  - [x] Hover highlighting
  - [x] Global / Local space mode
  - [ ] Lighting gizmo
  - [ ] Ability to drag edges of a colldier to change its size

- [ ] Play mode
  - [ ] Play / Pause / Stop
  - [ ] Switch between editor and game camera

- [x] Undo / Redo

# Apostasy

# Lua Scripting

Implemented as global lifecycle scripts: every `.lua` under `res/scripts/` runs
in its own sandbox and may define `start`/`prerender`/`update`/`fixed_update`/
`late_update`. Lua-defined components are declared with `register_component` and
stored on entities as `ScriptComponents` (editable in the editor inspector and
persisted by the worldspace serializer). Global state uses `register_resource`.

- [x] Add mlua as a dependency to core
- [x] LuaRuntime resource — holds the Lua state, loads and caches script files
- [x] Lifecycle systems — start / prerender / update / fixed_update / late_update (game mode)
  - [x] Registration runs in all modes so the editor learns component/resource schemas
- [x] Expose World to Lua as a userdata type
  - [x] world:get_resource(name) / set_resource(name, table)
  - [x] world:get_entity_with_tag / get_entities_with_tag / get_all_entities
  - [x] world:get_component(id, name) — returns a Lua table copy
  - [x] world:set_component(id, name, table) / add_component / remove_component
  - [x] world:add_tag(id, name) / remove_tag(id, name)
  - [x] world:spawn() / spawn_at_position(pos) — returns an EntityId
  - [x] world:despawn(id)
  - [x] world:set_name / get_name
  - [x] Hierarchy — set_parent / get_children / get_ancestors
  - [x] world:raycast(origin, dir, max, ignore)
  - [x] world:set_material_color(name, color)
  - [x] world:delta() / world:time() / world:log(...)
- [x] Query builder from Lua
  - [x] world:query(...component names) — returns a QueryBuilder userdata
  - [x] :for_each(function(id, ...components)) — iterates results
- [x] Lua-defined components — register_component(name, defaults), stored as ScriptComponents
  - [x] Editor inspector add/edit/remove for Lua components
  - [x] Serialization round-trips through the worldspace serializer
- [x] Global resources — register_resource(name, defaults)
- [x] Script hot-reload — re-execs changed script files without restart
- [x] Math prelude — vec2 / vec3 / vec4 / quat helpers
- [x] In-engine console
- [x] LSP type stubs (res/scripts/types/*.lua + .luarc.json)
- [ ] Hot-reload: pick up newly added / deleted script files at runtime
- [ ] Per-entity script logic — run a script's update only for entities that hold it
- [ ] :with_tag / :without_tag / :with / :without query filters
