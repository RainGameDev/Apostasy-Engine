# Apostasy Lua API type definitions

These `.lua` files are [Lua Language Server](https://luals.github.io/)
definition stubs. They are **not executed** — the engine's script discovery
(`discover_lua_scripts` in `core/src/scripting/lua/runtime.rs`) skips any path
containing a `types/` component. Their only job is to give editors autocomplete,
hover documentation, and diagnostics for the scripting API.

## Setup

Install [`lua-language-server`](https://luals.github.io/#install) and point your
editor's Lua LSP at this `scripts/` folder as the workspace root. The sibling
[`.luarc.json`](../.luarc.json) already wires up:

- `runtime.version`: `Lua 5.4` (matches the embedded `mlua` runtime)
- `workspace.library`: `["types"]` — loads these stubs

## Getting `world:` autocomplete + hover docs

The engine passes the world handle as a **parameter** to your `start` / `update`
/ `fixed_update` functions. Lua Language Server cannot infer that parameter's
type on its own (when you redefine the function it drops the meta signature), so
annotate it once per function with `---@param world World`:

```lua
---@param world World
function start(world)
    world:        -- ← autocompletes every method, with docs
end
```

`test.lua` already does this — copy it as a starting template. Without the
annotation the script still runs fine; you just lose autocomplete/hover on that
`world`.

## Files

| File | Defines |
|---|---|
| `world.lua` | The `World` handle passed to `start`/`update`/`fixed_update` |
| `entity.lua` | The opaque `Entity` handle |
| `query.lua` | The chainable `Query` builder from `world:query(...)` |
| `globals.lua` | Top-level globals (`register_component`, `register_resource`) and the `start`/`update`/`fixed_update` lifecycle entry points |

## Keeping these in sync

The stubs mirror the Rust bindings. When you add or change a method, update the
matching stub:

- `World` methods → `core/src/scripting/lua/world_api.rs`
- `Query` methods → `core/src/scripting/lua/query.rs`
- `register_component` / `register_resource` → `core/src/scripting/lua/runtime.rs`
