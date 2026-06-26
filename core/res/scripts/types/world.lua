---@meta

-- Type definitions for the Apostasy engine Lua scripting API.
--
-- These stubs exist only to drive the Lua Language Server (autocomplete, hover
-- docs, diagnostics). They are never executed: the `types/` directory is skipped
-- by the script discovery walker (see `discover_lua_scripts` in
-- `core/src/scripting/lua/runtime.rs`).

---A scoped, per-call view of the engine `World`.
---
---The handle passed to `start`, `update`, and `fixed_update` is only valid for
---the duration of that one call — never store it in a global or capture it in a
---callback that outlives the frame.
---@class World
local World = {}

-- ---------------------------------------------------------------------------
-- Entities
-- ---------------------------------------------------------------------------

---Spawns a new, empty entity in cell (0, 0, 0) and returns its handle.
---@return Entity entity
function World:spawn() end

---Spawns a new entity in the cell that contains `position` (a vec3 or
---`{x, y, z}` table). The entity itself has no Transform until you add one.
---@param position vec3|number[]
---@return Entity entity
function World:spawn_at_position(position) end

---Spawns a new entity in a specific 128-unit cell, addressed by integer cell
---coordinates (the world is divided into cells on the XZ plane, infinite in Y).
---@param cx integer
---@param cy integer
---@param cz integer
---@return Entity entity
function World:spawn_in_cell(cx, cy, cz) end

---Despawns an entity, removing it and all of its components from the world.
---@param entity Entity
function World:despawn(entity) end

---Sets the display name of an entity (shown in the editor hierarchy).
---@param entity Entity
---@param name string
function World:set_name(entity, name) end

---Returns an entity's display name, or `nil` if it has none.
---@param entity Entity
---@return string|nil
function World:get_name(entity) end

-- ---------------------------------------------------------------------------
-- Tags (zero-size markers, referenced by name)
-- ---------------------------------------------------------------------------

---Adds a tag to an entity. Tags are zero-size markers used for queries
---(e.g. `"Player"`, `"ActiveCamera"`).
---@param entity Entity
---@param name string
function World:add_tag(entity, name) end

---Removes a tag from an entity.
---@param entity Entity
---@param name string
function World:remove_tag(entity, name) end

---Returns `true` if the entity carries the named tag.
---@param entity Entity
---@param name string
---@return boolean
function World:has_tag(entity, name) end

---Returns the first entity carrying the named tag, or `nil` if none exists.
---@param name string
---@return Entity|nil
function World:get_entity_with_tag(name) end

---Returns an array of every entity carrying the named tag.
---@param name string
---@return Entity[]
function World:get_entities_with_tag(name) end

-- ---------------------------------------------------------------------------
-- Hierarchy (parent/child relationships)
-- ---------------------------------------------------------------------------

---Parents `child` under `parent`. Re-parenting moves the child if it already
---had a parent. Use `detach` to clear a parent. Moving across a cell boundary
---may change the child's id.
---@param child Entity
---@param parent Entity
function World:set_parent(child, parent) end

---Removes an entity's parent, making it a root entity. No-op if already a root.
---@param entity Entity
function World:detach(entity) end

---Returns an entity's parent, or `nil` if it is a root entity.
---@param entity Entity
---@return Entity|nil
function World:get_parent(entity) end

---Returns an array of an entity's direct children (not recursive).
---@param entity Entity
---@return Entity[]
function World:get_children(entity) end

---Returns an array of an entity's ancestors, ordered root first down to the
---immediate parent (the last element is the entity's direct parent).
---@param entity Entity
---@return Entity[]
function World:get_ancestors(entity) end

---Returns an array of all of an entity's descendants (recursive).
---@param entity Entity
---@return Entity[]
function World:get_descendants(entity) end

---Returns an array of every entity that has no parent.
---@return Entity[]
function World:get_root_entities() end

---Returns an array of every entity in the world.
---@return Entity[]
function World:get_all_entities() end

-- ---------------------------------------------------------------------------
-- Components (per-entity data, referenced by name)
-- ---------------------------------------------------------------------------
--
-- `name` resolves against the engine's native components first (any
-- `#[derive(Component)]` type, e.g. `"Transform"`, `"Velocity"`, `"Light"`,
-- `"Camera"`, `"Collider"`, `"ModelRenderer"`), then falls back to script
-- components declared with `register_component`. Native component tables use the
-- same field layout as the scene format — vectors are `[x, y, z]` arrays:
--
-- ```lua
-- world:add_component(e, "Transform", { local_position = { 0, 5, 0 } })
-- local t = world:get_component(e, "Transform")
-- t.local_position[2] = t.local_position[2] + world:delta()   -- read-only copy
-- world:set_component(e, "Transform", { local_position = t.local_position })
-- world:add_component(e, "Velocity", { linear_velocity = { 0, 0, -3 } })
-- ```

---Adds a component to an entity. For native components, fields in `overrides`
---are applied onto a default instance; for script components (declared with
---`register_component`) they're overlaid onto the registered defaults. Omit
---`overrides` to use the defaults as-is.
---@param entity Entity
---@param name string
---@param overrides? table
function World:add_component(entity, name, overrides) end

---Returns a copy of the named component's data as a table, or `nil` if the
---entity does not have it (or the native component isn't serializable). Mutating
---the returned table does **not** write back — use `set_component` to persist.
---@param entity Entity
---@param name string
---@return table|nil
function World:get_component(entity, name) end

---Writes the named component on an entity, creating it if absent. Only the
---fields present in `value` are changed (a partial update, not a full replace).
---@param entity Entity
---@param name string
---@param value table
function World:set_component(entity, name, value) end

---Removes the named component from an entity.
---@param entity Entity
---@param name string
function World:remove_component(entity, name) end

---Returns `true` if the entity has the named component.
---@param entity Entity
---@param name string
---@return boolean
function World:has_component(entity, name) end

-- ---------------------------------------------------------------------------
-- Queries
-- ---------------------------------------------------------------------------

---Begins a query over entities that have **all** of the named components.
---Names resolve to native components (e.g. `"Transform"`) or script components
---declared with `register_component` — either kind can be fetched or filtered.
---Chain `:with`, `:without`, `:with_tag`, `:without_tag` to refine, then call
---`:for_each` to iterate.
---
---```lua
---world:query("Transform"):with_tag("Player"):for_each(function(id, t)
---    t.local_position[2] = t.local_position[2] + world:delta()
---    world:set_component(id, "Transform", { local_position = t.local_position })
---end)
---```
---@param ... string component names to fetch (and pass to the `for_each` callback)
---@return Query
function World:query(...) end

-- ---------------------------------------------------------------------------
-- Physics
-- ---------------------------------------------------------------------------

---A single ray/collider intersection. `point` and `normal` are `[x, y, z]`
---sequences — wrap with `vec3(...)` for arithmetic.
---@class RaycastHit
---@field entity Entity the entity that was struck
---@field point number[] world-space hit point `[x, y, z]`
---@field normal number[] surface normal at the hit `[x, y, z]`
---@field distance number distance along the ray to the hit
---@field face integer struck face: 0=-X 1=+X 2=-Y 3=+Y 4=-Z 5=+Z (0 for spheres/meshes)

---Casts a ray against every collider in the world and returns the nearest hit
---within `max_distance`, or `nil` if nothing was struck. `origin`/`direction`
---accept a vec3 or `{x, y, z}` table; the direction is normalized internally.
---Pass `ignore` to skip an entity (commonly the caster itself).
---
---```lua
---local t = world:get_component(self, "Transform")
---local hit = world:raycast(t.global_position, vec3.forward, 100, self)
---if hit then world:log("hit " .. tostring(hit.entity) .. " at " .. hit.distance) end
---```
---@param origin vec3|number[]
---@param direction vec3|number[]
---@param max_distance number
---@param ignore? Entity
---@return RaycastHit|nil
function World:raycast(origin, direction, max_distance, ignore) end

-- ---------------------------------------------------------------------------
-- Materials
-- ---------------------------------------------------------------------------

---Sets a loaded material's RGBA color. `color` is an `{r, g, b, a}` table or an
---`[r, g, b, a]` sequence (missing alpha defaults to 1.0). The render loop reads
---material colors each frame, so this re-tints every entity whose
---`ModelRenderer.material_override` names this material — immediately. No-op if
---no material with that name is loaded.
---@param name string
---@param color number[]|{r:number, g:number, b:number, a:number}
function World:set_material_color(name, color) end

---Returns a material's current color as an `[r, g, b, a]` sequence, or nil if no
---material with that name is loaded.
---@param name string
---@return number[]|nil
function World:get_material_color(name) end

-- ---------------------------------------------------------------------------
-- Time
-- ---------------------------------------------------------------------------

---Seconds elapsed since the previous frame.
---@return number
function World:delta() end

---Total seconds elapsed since the engine started.
---@return number
function World:time() end

-- ---------------------------------------------------------------------------
-- Global script resources (a single shared blackboard, not per-entity)
-- ---------------------------------------------------------------------------

---Returns a global script resource as a table, or `nil` if unset. Resources are
---seeded at script top-level with `register_resource`.
---@param name string
---@return table|nil
function World:get_resource(name) end

---Sets (or replaces) a global script resource.
---@param name string
---@param value table
function World:set_resource(name, value) end

---Returns `true` if a global script resource with this name exists.
---@param name string
---@return boolean
function World:has_resource(name) end

---Removes a global script resource.
---@param name string
function World:remove_resource(name) end

-- ---------------------------------------------------------------------------
-- Input
-- ---------------------------------------------------------------------------

---Returns `true` if the named keybind is currently active.
---@param name string
---@return boolean
function World:is_keybind_active(name) end

---Returns `true` if the named mouse bind is currently active.
---@param name string
---@return boolean
function World:is_mousebind_active(name) end

---Builds a 2D direction vector from four keybind names. Each axis is the
---difference of its two binds, so the result components are in `[-1, 1]`.
---@param left string
---@param right string
---@param up string
---@param down string
---@return { x: number, y: number }
function World:input_vector_2d(left, right, up, down) end

-- ---------------------------------------------------------------------------
-- Logging
-- ---------------------------------------------------------------------------

---Logs an informational message, prefixed with `[lua]`.
---@param message string
function World:log(message) end

---Logs a warning, prefixed with `[lua]`.
---@param message string
function World:log_warn(message) end

---Logs an error, prefixed with `[lua]`.
---@param message string
function World:log_error(message) end

return World
