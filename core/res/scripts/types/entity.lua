---@meta

---An opaque handle to an entity in the world.
---
---Obtained from `World:spawn`, `World:get_entity_with_tag`,
---`World:get_entities_with_tag`, or the first argument of a `Query:for_each`
---callback. Supports `tostring(entity)` and `==` comparison between handles.
---@class Entity
local Entity = {}

return Entity
