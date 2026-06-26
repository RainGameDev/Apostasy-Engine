---@meta

-- Top-level globals available to every script, plus the lifecycle entry points
-- the engine calls by name.

---Declares a script component schema. Call this at script top-level (not inside
---a function) — it runs at load time in both editor and game mode, so the editor
---learns the component's fields without executing gameplay logic.
---
---The `defaults` table defines the component's fields and their default values.
---`World:add_component` starts from these defaults and overlays any overrides.
---
---```lua
---register_component("Health", { current = 100, max = 100 })
---```
---@param name string
---@param defaults table
function register_component(name, defaults) end

---Declares a global script resource and seeds its default value. Call this at
---script top-level. The value is only seeded if the resource does not already
---exist, so live values survive hot-reload.
---
---```lua
---register_resource("Score", { value = 0 })
---```
---@param name string
---@param defaults table
function register_resource(name, defaults) end

-- Lifecycle entry points. Define any subset as a top-level function; the engine
-- calls each by name on the matching frame phase. All run in game mode only
-- (the editor loads scripts for their `register_*` declarations but never runs
-- gameplay). The per-frame order is: prerender → update → fixed_update → late_update.

---@type fun(world: World)
---Called once per script when the engine starts. Not re-invoked on hot-reload
---(reloading re-runs the script's top-level code, but not `start`).
start = nil

---@type fun(world: World)
---Called once per script each frame, before the frame is rendered.
prerender = nil

---@type fun(world: World)
---Called once per script every frame, after `prerender`. Scripts are
---hot-reloaded just before this runs.
update = nil

---@type fun(world: World, delta: number)
---Called once per script at the fixed timestep (~20 Hz). `delta` is the fixed
---timestep in seconds (~0.05). Use this for physics-rate gameplay logic.
fixed_update = nil

---@type fun(world: World)
---Called once per script at the end of each frame.
late_update = nil
