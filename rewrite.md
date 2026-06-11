# Worldspace + Cell system (replaces the Scene system)

## Goal

Replace the current single-`Scene` model with a **Worldspace → Cell** model inspired by
Bethesda's Creation Engine (Morrowind/Skyrim). A worldspace is an effectively infinite
grid of cells; cells are created and loaded lazily, only when an object occupies them.
A cell holds objects exactly the way `Scene` does today (objects with components, tags,
and a parent/child hierarchy).

The public API surface on `World` must stay the same as much as possible — the same
function names with the same signatures — so existing systems (`get_objects_with_component`,
`add_object`, hierarchy ops, voxel helpers, etc.) keep compiling. Where a method now needs
to know *which* cell to act on, add cell-scoped variants rather than breaking the global ones.

## Concrete current state (do not rediscover)

- `core/src/objects/scene.rs`: `Scene { objects: SlotMap<ObjectId, Object> }`,
  `pub type ObjectId = slotmap::DefaultKey`. ~25 methods (add/remove/hierarchy/query).
- `core/src/objects/world.rs`: `World` owns `scene: Scene`, `resources: ResourceMap`,
  `chunk_position_index: HashMap<(i32,i32,i32), ObjectId>`, and the system vecs.
  Nearly every `World` object method is a thin proxy to `self.scene`.
- `core/src/objects/scene_serializer.rs`: YAML `save_scene` / `load_scene`. Writes a doc
  with `class: scene` and a flat `objects:` tree. Component (de)serialization is dispatched
  by short type name.
- `core/src/assets/loaders/scene_loader.rs`: `SceneLoader` + `SceneRegistry { scenes: HashMap<String, Value> }`,
  keyed off `class: "scene"`.
- Editor: `editor/src/ui/cell_panel.rs` already exists (`CellSearchState`, "Cell Panel"
  window) but currently lists **scenes**. `editor/src/systems/editor_scene.rs` registers
  loaders and loads/saves scenes to `res/scenes/{name}.yaml`.
- Existing spatial precedent to mirror: `core/src/voxels/chunk_loader.rs`
  (`ChunkPositionMap { position_to_id: HashMap<Vector3<i32>, ObjectId> }`, `ChunkLoadBounds`)
  and `World::chunk_position_index`. The voxel chunk streaming is the model for cell streaming.

## Cell geometry

- Each cell is **128 (X) × ∞ (Y) × 128 (Z)** world units. Cells tile the XZ plane only;
  a cell spans all of Y.
- Cell coordinate of a world position: `cell.x = floor(world.x / 128)`,
  `cell.z = floor(world.z / 128)`, using `i32::div_euclid(128)` (matches the voxel code's
  `div_euclid`/`rem_euclid` style). **`cell.y` is always 0.**
- Keep the key type as `Vector3<i32>` per the spec (forward-compat), but document that
  `.y == 0` invariably. (If we later decide Y is meaningful, only the coord function changes.)

## Data model

```rust
// core/src/objects/cell.rs  (rename/replace scene.rs)
pub struct Cell {
    pub coord: Vector3<i32>,
    pub objects: SlotMap<ObjectId, Object>,   // same storage Scene used
    // optional cell-level metadata later (name, persistent flag, etc.)
}

// core/src/objects/worldspace.rs
pub struct Worldspace {
    pub name: String,
    pub cells: HashMap<Vector3<i32>, Cell>,   // <-- the spec's HashMap<Vector3<i32>, Cell>
}
```

`World` owns the active worldspace(s). Replace `scene: Scene` with a worldspace it operates
on (start with a single active `Worldspace`; multi-worldspace can come later but design the
types so it's not painful).

## Object identity — the decision the old prompt skipped

`ObjectId` is currently a global `slotmap::DefaultKey`. If each `Cell` owns its own `SlotMap`,
keys collide across cells, and `Object.parent` / `Object.children` (cross-cell refs) plus
`World::chunk_position_index` break.

**Recommended (Creation-Engine-faithful): composite id.**
```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId {
    pub cell: Vector3<i32>,
    pub key: slotmap::DefaultKey,
}
```
- `World`/`Cell` resolve an object by routing `id.cell → Cell → cell.objects[id.key]`.
- Update everything that assumes `ObjectId = DefaultKey`: `fmt_key` (objects/mod.rs),
  `chunk_position_index` values, raycast/voxel-raycast hit ids, editor `ObjectRefEntry`.
- **Cross-cell hierarchy:** an object and its children may end up in different cells when
  moved. Decide one rule and document it: simplest is *children always live in the parent's
  cell* (re-home descendants on parent move). Note this so the implementer doesn't silently
  orphan children.
- **Moving across a cell boundary** changes an object's `ObjectId` (its `cell` field). Provide
  a `World::move_object_to_cell(old_id, new_cell) -> ObjectId` migration helper and have the
  transform/physics update path call it when a position crosses 128-unit boundaries.

*Alternative (lower churn, less faithful):* keep one global `SlotMap<ObjectId, Object>` in the
worldspace and make `Cell` a pure spatial bucket (`HashSet<ObjectId>`), grouping by cell only
at serialization time. This keeps `ObjectId = DefaultKey` and avoids re-homing, but cells no
longer "own" their object data at runtime. **Pick one before implementing** — this is the
single most consequential choice.

## World API changes

Keep all existing signatures working against the *currently loaded* set of cells:

- `add_object(obj)` / `add_new_object()`: place the object in the cell computed from its
  `Transform.local_position` (cell `(0,0,0)` if it has no `Transform`). Create the cell lazily
  if absent.
- `get_object`, `get_object_mut`, `remove_object`, all hierarchy ops: resolve through the
  owning cell (composite id makes this O(1)).
- `get_objects_with_component`, `get_objects_with_tag`, `get_all_objects`, `get_root_objects`,
  and the `_with_ids` variants: iterate over **all loaded cells** and concatenate. Behavior is
  unchanged for callers; only the iteration source changes.

Add cell-scoped variants for callers that care about locality (and for streaming/perf):

- `get_or_create_cell(coord) -> &mut Cell`, `get_cell(coord) -> Option<&Cell>`,
  `loaded_cells() -> impl Iterator`, `unload_cell(coord)`.
- `get_objects_with_component_in_cell::<T>(coord)`, etc.
- A helper to map world position → cell coord, reused by add/move/query.

## Lazy creation & streaming

- A cell exists in the worldspace map only once something is placed in it; querying an empty
  coord returns `None` / creates on demand via `get_or_create_cell`.
- Loading/unloading should reuse the chunk-streaming pattern in `chunk_loader.rs`: a
  load radius around the player/camera cell, load on enter, unload (serialize out) on exit.
  Reconcile this with the voxel chunk index so a chunk's owning object lives in the right cell.
- Decide and document the policy for cells with no objects left after a move (drop empty cells).

## Serialization (per the spec: one file per worldspace)

- New `worldspace_serializer.rs` (replaces `scene_serializer.rs`): one file per worldspace,
  e.g. `res/worldspaces/{name}.yaml` (replace `res/scenes`). Top-level doc:
  `class: worldspace`, `name:`, and `cells:` = the serialized `HashMap<Vector3<i32>, Cell>`.
  Serialize the `Vector3<i32>` key as e.g. `[x, y, z]` or `"x,y,z"`.
- Each cell serializes its object tree using the **existing** per-object/per-component YAML
  format (reuse `serialize_object` / `serialize_component` / `load_object` verbatim — only the
  surrounding container changes). Preserve the `SkipsSerilization` / `EditorCamera` skip logic.
- `load_scene`'s "keep objects tagged X, remove the rest" behavior becomes per-worldspace load.
- Rename `SceneLoader`/`SceneRegistry`/`SceneClass("scene")` → `WorldspaceLoader` /
  `WorldspaceRegistry` / `class: "worldspace"`. Update `editor_scene.rs` registration.

## Editor integration

- Repurpose the existing `cell_panel.rs` "Cell Panel": left list = worldspaces (was scenes),
  and surface the loaded cells / object placement per cell. `CellSearchState`, `scene_load` /
  `scene_delete` / `scene_rename`, and `EditorPreferences::last_scene` get worldspace equivalents.
- Object search/teleport should show and jump to an object's cell coord.

## Suggested implementation order

1. Land the `ObjectId` decision (composite vs. global+buckets); update `fmt_key`,
   `chunk_position_index`, raycast hit types so the tree compiles.
2. Add `Cell` (rename `Scene`) and `Worldspace`; swap `World.scene` for the active worldspace;
   reimplement the proxied methods over loaded cells. Keep signatures identical.
3. Cell coord math + lazy `get_or_create_cell` + add/move-on-boundary migration.
4. `worldspace_serializer.rs` + `WorldspaceLoader`; migrate `res/scenes` → `res/worldspaces`.
5. Wire streaming (load radius) reusing the chunk-loader pattern; reconcile the voxel index.
6. Editor: rename scene→worldspace in `cell_panel.rs` / `editor_scene.rs`, add cell view.

## Open decisions to confirm before coding

- Composite `ObjectId` vs. global slotmap + spatial buckets (the alternative above).
- Cross-cell parent/child rule (recommend: descendants re-home into parent's cell).
- Whether `World` supports multiple loaded worldspaces at once now, or just one active.
- Cell unload trigger/policy (radius + serialize-out, drop-when-empty).
