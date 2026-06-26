---@meta

---A chainable query builder returned by `World:query`. Each filter method
---returns the same query so calls can be chained; `for_each` runs it.
---@class Query
local Query = {}

---Keeps only entities that also carry the named tag.
---@param name string
---@return Query
function Query:with_tag(name) end

---Keeps only entities that do **not** carry the named tag.
---@param name string
---@return Query
function Query:without_tag(name) end

---Keeps only entities that also have the named script component.
---@param name string
---@return Query
function Query:with(name) end

---Keeps only entities that do **not** have the named script component.
---@param name string
---@return Query
function Query:without(name) end

---Runs the query, invoking `callback` once per matching entity.
---
---The callback receives the entity handle followed by one table per component
---named in the originating `World:query(...)` call, in the same order.
---
---```lua
---world:query("Health", "Position"):for_each(function(id, health, position)
---    -- ...
---end)
---```
---
---Note: the component tables are **copies**. Write changes back with
---`World:set_component`. The world may be freely mutated from inside the
---callback (the matching set is snapshotted before iteration begins).
---@param callback fun(entity: Entity, ...: table)
function Query:for_each(callback) end

return Query
