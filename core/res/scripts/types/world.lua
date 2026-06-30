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
-- same field layout as the worldspace format — vectors are `[x, y, z]` arrays:
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

---Builds a 3D direction vector from six keybind names, same semantics as
---`input_vector_2d` but with an extra (z_pos, z_neg) axis.
---@param x_pos string
---@param x_neg string
---@param y_pos string
---@param y_neg string
---@param z_pos string
---@param z_neg string
---@return { x: number, y: number, z: number }
function World:input_vector_3d(x_pos, x_neg, y_pos, y_neg, z_pos, z_neg) end

---Cursor position in physical pixels relative to the top-left of the window.
---@return { x: number, y: number }
function World:mouse_position() end

---Raw mouse delta accumulated this frame, not affected by cursor
---acceleration or OS pointer speed use this for camera look.
---@return { x: number, y: number }
function World:mouse_delta() end

---Scroll wheel delta accumulated this frame, in pixels.
---@return { x: number, y: number }
function World:scroll_delta() end

---Marks `name` as an active input context for the remainder of this frame.
---Binds registered with `opts.context == name` only fire while their
---context is active; call this before checking those binds.
---@param name string
function World:set_input_context(name) end

---@class KeybindOpts
---@field ctrl? boolean            Only fires while Ctrl is held (Press/Release binds).
---@field shift? boolean           Only fires while Shift is held (Press/Release binds).
---@field alt? boolean             Only fires while Alt is held (Press/Release binds).
---@field context? string          Only fires while this context is active (see `set_input_context`).
---@field repeat_delay? number     Seconds held before a Press bind starts auto-repeating.
---@field repeat_rate? number      Repeats per second once auto-repeat has started.

---Registers a new keybind. Fails (returns `false` and logs) if `name` is
---already registered use `rebind_key` to overwrite an existing bind.
---`key` is a physical key name (e.g. `"Space"`, `"KeyW"`, `"ShiftLeft"`),
---`action` is one of `"Press"`, `"Release"`, `"Hold"`.
---
---```lua
---world:register_keybind("Jump", "Space", "Press")
---world:register_keybind("Save", "KeyS", "Press", { ctrl = true })
---world:register_keybind("Sprint", "ShiftLeft", "Hold")
---world:register_keybind("Interact", "KeyE", "Press", { context = "gameplay" })
---```
---@param name   string
---@param key    string
---@param action "Press"|"Release"|"Hold"
---@param opts?  KeybindOpts
---@return boolean registered
function World:register_keybind(name, key, action, opts) end

---Registers a default keybind only takes effect if `name` has no bind yet
---(e.g. nothing was loaded from the keybinds file). Use for bootstrapping
---defaults on startup; never errors.
---@param name   string
---@param key    string
---@param action "Press"|"Release"|"Hold"
---@param opts?  KeybindOpts
function World:register_default_keybind(name, key, action, opts) end

---Overwrites the bind for `name` and persists the change. Use this for
---remapping from a settings/preferences UI.
---@param name   string
---@param key    string
---@param action "Press"|"Release"|"Hold"
---@param opts?  KeybindOpts
function World:rebind_key(name, key, action, opts) end

---Mouse-button equivalent of `register_keybind`. `button` is one of
---`"Left"`, `"Right"`, `"Middle"`, `"Back"`, `"Forward"`.
---@param name   string
---@param button string
---@param action "Press"|"Release"|"Hold"
---@param opts?  KeybindOpts
---@return boolean registered
function World:register_mousebind(name, button, action, opts) end

---Mouse-button equivalent of `register_default_keybind`.
---@param name   string
---@param button string
---@param action "Press"|"Release"|"Hold"
---@param opts?  KeybindOpts
function World:register_default_mousebind(name, button, action, opts) end

---Mouse-button equivalent of `rebind_key`. Use for remapping mouse buttons.
---@param name   string
---@param button string
---@param action "Press"|"Release"|"Hold"
---@param opts?  KeybindOpts
function World:rebind_mouse(name, button, action, opts) end

-- ---------------------------------------------------------------------------
-- UI (egui immediate-mode)
-- ---------------------------------------------------------------------------
--
-- UI calls are only valid during `update` and `fixed_update` (the engine
-- opens the egui frame before update systems run and closes it after
-- fixed_update).  Calling them in `prerender` or `late_update` is a no-op.
--
-- The `UiHandle` passed to a callback is invalidated once the callback
-- returns — never store it in a global or capture it in a closure that
-- outlives the frame.
--
-- ```lua
-- function update(world)
--     world:ui_window("HUD", function(ui)
--         ui:label("Health: " .. player.hp)
--         ui:separator()
--         if ui:button("Use potion") then heal(world) end
--         local spd = ui:slider("Speed", walk_speed, 0.0, 10.0)
--         walk_speed = spd
--     end)
-- end
-- ```

---A transient handle to an egui `Ui` context, only valid inside its
---callback. Every method is a no-op if called on a stale handle.
---@class UiHandle
local UiHandle = {}

-- -- text ---------------------------------------------------------------------

---@param text string
function UiHandle:label(text) end

---Large heading text.
---@param text string
function UiHandle:heading(text) end

---Small sub-label text.
---@param text string
function UiHandle:small(text) end

---Colored label. Components are 0–255 integers.
---@param color { r: integer, g: integer, b: integer, a: integer }|integer[]
---@param text string
function UiHandle:colored_label(color, text) end

-- -- interactive widgets -------------------------------------------------------

---Returns `true` on the frame the button is clicked.
---@param text string
---@return boolean
function UiHandle:button(text) end

---Checkbox. Returns the new value.
---@param text string
---@param value boolean
---@return boolean
function UiHandle:checkbox(text, value) end

---Horizontal slider. Returns the (possibly changed) value.
---@param label string
---@param value number
---@param min number
---@param max number
---@return number
function UiHandle:slider(label, value, min, max) end

---Drag-number field. `speed` is change-per-pixel (default 1.0). Returns new value.
---@param label string
---@param value number
---@param speed? number
---@return number
function UiHandle:drag(label, value, speed) end

---Single-line text field. Label and field are on the same row.
---Returns the (possibly edited) string.
---@param label string
---@param text string
---@return string
function UiHandle:text_input(label, text) end

---Dropdown combo box. `options` is a 1-indexed string array. `selected` is
---1-based. Returns the new 1-based selected index.
---@param label string
---@param options string[]
---@param selected integer
---@return integer
function UiHandle:combo_box(label, options, selected) end

---Horizontal progress bar. `fraction` is clamped to 0.0–1.0.
---@param fraction number
function UiHandle:progress_bar(fraction) end

-- -- layout containers ---------------------------------------------------------

---Lays out widgets in a horizontal row.
---@param callback fun(ui: UiHandle)
function UiHandle:horizontal(callback) end

---Explicit vertical stack (useful inside a `horizontal` to re-enter vertical flow).
---@param callback fun(ui: UiHandle)
function UiHandle:vertical(callback) end

---Splits into `n` equal columns. `callback` receives a 1-indexed table of
---`UiHandle`s — one per column.
---@param n integer
---@param callback fun(cols: UiHandle[])
function UiHandle:columns(n, callback) end

---Collapsible section with a clickable heading.
---@param label string
---@param callback fun(ui: UiHandle)
function UiHandle:collapsing(label, callback) end

-- -- misc ---------------------------------------------------------------------

---Horizontal separator line.
function UiHandle:separator() end

---Adds `amount` egui points of blank space.
---@param amount number
function UiHandle:space(amount) end

-- -- World UI methods ----------------------------------------------------------
--
-- Only valid during `update` / `fixed_update` (the egui pass is open between
-- `begin_ui` and `end_ui`, which bracket those two system stages).

---@class UiWindowOpts
---@field anchor?       "top_left"|"top_right"|"bottom_left"|"bottom_right"|"center"
---@field offset?       number[]   -- {x, y} pixel offset from the anchor point
---@field pos?          number[]   -- {x, y} fixed screen position (overrides anchor)
---@field size?         number[]   -- {w, h} default window size
---@field no_title_bar? boolean    -- hide the title bar
---@field resizable?    boolean    -- allow / prevent resizing (default true)

---Opens a floating egui window and calls `callback` with a `UiHandle`.
---
---```lua
-----  simple
---world:ui_window("HUD", function(ui) ui:label("hello") end)
---
-----  pinned to top-right with no title bar
---world:ui_window("HUD", { anchor="top_right", offset={-10,10}, no_title_bar=true },
---    function(ui) ui:label("HP: 87") end)
---```
---@param title    string
---@param opts_or_callback UiWindowOpts|fun(ui: UiHandle)
---@param callback? fun(ui: UiHandle)
function World:ui_window(title, opts_or_callback, callback) end

-- ---------------------------------------------------------------------------
-- worldspaces
-- ---------------------------------------------------------------------------

---Loads a worldspace/worldspace by name, despawning the current scene's entities
---first. `retain_tags` is an optional list of tag names whose entities survive
---the switch; omit it to clear everything. Falls back to the `"default"` worldspace
---if `name` is not found. Returns `true` if a worldspace was loaded.
---
---```lua
-----  switch worldspaces, keeping the player entity
---world:load_worldspace("dungeon_01", { "Player" })
---```
---@param name        string
---@param retain_tags? string[]
---@return boolean loaded
function World:load_worldspace(name, retain_tags) end

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
