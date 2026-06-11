---
name: worldspace-cell-system
description: Invariants of the Worldspace/Cell system that replaced Scene (composite ObjectId, intra-cell hierarchy)
metadata:
  type: project
---

The `Scene` system was replaced (branch `worldspace-rework`) with a Creation-Engine-style
Worldspace→Cell model. `World` owns one active `Worldspace { cells: HashMap<Vector3<i32>, Cell> }`;
cells are 128×∞×128 (X/Z tiled, Y always 0) and created lazily.

Non-obvious invariants future work must respect:
- `ObjectId` is now composite `{ cell: Vector3<i32>, key: DefaultKey }` (was a bare `DefaultKey`).
  `Object.id` is now actually populated (it wasn't before).
- **Object hierarchies never span cells.** Children always live in the parent's cell. Cross-cell
  `set_parent` migrates the child subtree into the parent's cell via `Worldspace::move_subtree_to_cell`.
- **Crossing a 128-unit boundary changes an object's `ObjectId`** (its `cell` field). Use
  `Worldspace::rehome_by_position` / `move_subtree_to_cell`; any cached id becomes stale after migration.
  `cell_streaming_system` (`#[late_update(mode="all")]`, in `objects/cell_streaming.rs`) auto-migrates root
  objects each frame and publishes `old->new` ids in the `CellMigrations` resource (replaced per frame).
  Any code caching an `ObjectId` (editor selection, etc.) must remap via `CellMigrations` — the editor does
  this in `remap_selection_after_migration` (priority 10000, runs before the inspector). Only roots migrate
  (children are pinned to the parent's cell), so remapping root ids covers editor selection.
- `transform_update` solves each cell independently (relies on the intra-cell invariant).
- Serialization: one file per worldspace, `class: worldspace`, `cells:` map keyed by `"x,y,z"`.
  `worldspace_serializer::load_worldspace` also accepts the legacy flat `objects:` list (`class: scene`
  files migrated to `class: worldspace` keep working, auto-placed by transform position).
- `WorldspaceLoader`/`WorldspaceRegistry` (registry field `worldspaces`) replaced `SceneLoader`.
  Editor saves to `res/worldspaces/`. `EditorPreferences::last_scene` was kept as-is (just a persisted name).

Pre-existing, unrelated build breakage (NOT caused by this rework): `game` and `voxel_game` crates fail
on egui API drift (`Frame::none`, `CentralPanel::show(&ctx,...)`) and `ItemRegistry`/`BiomeRegistry` not
impl'ing `Resource`, `Collider::half_extents` called as a field. core + editor build clean.
