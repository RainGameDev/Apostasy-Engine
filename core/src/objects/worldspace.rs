use anyhow::Result;
use apostasy_macros::Resource;
use cgmath::Vector3;
use std::collections::HashMap;

use crate::{
    objects::{
        components::transform::Transform, object_store::ObjectId, tags::Player, world::World,
    },
    rendering::components::camera::ActiveCamera,
    update,
};

/// Side length of an exterior cell in world units (XZ plane, infinite verticality).
pub const CELL_SIZE: f32 = 256.0;

/// Identifies a cell within a worldspace.
/// Exterior worldspaces use `Grid` coordinates on the XZ plane,
/// interior worldspaces use `Named` cells.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CellKey {
    Grid(i32, i32),
    Named(String),
}

impl CellKey {
    /// Returns the grid cell containing a world-space position.
    pub fn from_position(position: Vector3<f32>) -> Self {
        CellKey::Grid(
            (position.x / CELL_SIZE).floor() as i32,
            (position.z / CELL_SIZE).floor() as i32,
        )
    }

    /// Center of the cell on the XZ plane (y = 0). Only meaningful for grid cells.
    pub fn center(&self) -> Vector3<f32> {
        match self {
            CellKey::Grid(x, y) => Vector3::new(
                (*x as f32 + 0.5) * CELL_SIZE,
                0.0,
                (*y as f32 + 0.5) * CELL_SIZE,
            ),
            CellKey::Named(_) => Vector3::new(0.0, 0.0, 0.0),
        }
    }

    pub fn label(&self) -> String {
        match self {
            CellKey::Grid(x, y) => format!("({}, {})", x, y),
            CellKey::Named(name) => name.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WorldspaceKind {
    #[default]
    Exterior,
    Interior,
}

impl WorldspaceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorldspaceKind::Exterior => "exterior",
            WorldspaceKind::Interior => "interior",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "interior" => WorldspaceKind::Interior,
            _ => WorldspaceKind::Exterior,
        }
    }
}

/// Serialized objects of a cell that is not currently instantiated in the world.
#[derive(Clone, Default)]
pub struct CellData {
    pub objects: Vec<serde_yaml::Value>,
}

/// The currently open worldspace. Owns the data of unloaded cells and tracks
/// which root objects belong to each loaded cell.
#[derive(Clone, Resource)]
pub struct ActiveWorldspace {
    pub name: String,
    pub kind: WorldspaceKind,
    /// Serialized data for cells that are not currently loaded.
    pub cells: HashMap<CellKey, CellData>,
    /// Root object ids per loaded cell.
    pub loaded: HashMap<CellKey, Vec<ObjectId>>,
    /// For interior worldspaces: the cell that is currently active.
    pub active_cell: Option<CellKey>,
}

impl ActiveWorldspace {
    pub fn new(name: &str, kind: WorldspaceKind) -> Self {
        Self {
            name: name.to_string(),
            kind,
            cells: HashMap::new(),
            loaded: HashMap::new(),
            active_cell: None,
        }
    }

    /// All cells of the worldspace (loaded and unloaded), sorted for display.
    pub fn all_cell_keys(&self) -> Vec<CellKey> {
        let mut keys: Vec<CellKey> = self
            .cells
            .keys()
            .chain(self.loaded.keys())
            .cloned()
            .collect();
        keys.sort();
        keys.dedup();
        keys
    }

    pub fn is_loaded(&self, key: &CellKey) -> bool {
        self.loaded.contains_key(key)
    }
}

/// How many cells in each direction around the streaming center stay loaded in game mode.
#[derive(Clone, Resource)]
pub struct CellLoadRadius(pub i32);

impl Default for CellLoadRadius {
    fn default() -> Self {
        CellLoadRadius(1)
    }
}

/// Streams exterior cells in and out around the player (or active camera) in game mode.
#[update(mode = "game", priority = 900)]
pub fn worldspace_streaming(world: &mut World) -> Result<()> {
    let kind = match world.get_resource::<ActiveWorldspace>() {
        Ok(ws) => ws.kind,
        Err(_) => return Ok(()),
    };
    if kind != WorldspaceKind::Exterior {
        return Ok(());
    }

    let center = world
        .get_object_with_tag::<Player>()
        .ok()
        .and_then(|obj| obj.get_component::<Transform>().ok())
        .map(|t| t.global_position)
        .or_else(|| {
            world
                .get_object_with_tag::<ActiveCamera>()
                .ok()
                .and_then(|obj| obj.get_component::<Transform>().ok())
                .map(|t| t.global_position)
        });

    let Some(center) = center else {
        return Ok(());
    };

    let radius = world
        .get_resource::<CellLoadRadius>()
        .map(|r| r.0)
        .unwrap_or(1)
        .max(0);

    let CellKey::Grid(cx, cy) = CellKey::from_position(center) else {
        return Ok(());
    };

    let mut desired = Vec::new();
    for x in (cx - radius)..=(cx + radius) {
        for y in (cy - radius)..=(cy + radius) {
            desired.push(CellKey::Grid(x, y));
        }
    }

    let ws = world.get_resource::<ActiveWorldspace>()?;
    let to_unload: Vec<CellKey> = ws
        .loaded
        .keys()
        .filter(|k| matches!(k, CellKey::Grid(_, _)) && !desired.contains(k))
        .cloned()
        .collect();
    let to_load: Vec<CellKey> = desired
        .into_iter()
        .filter(|k| ws.cells.contains_key(k))
        .collect();

    for key in to_unload {
        crate::objects::worldspace_serializer::unload_cell(world, &key)?;
    }
    for key in to_load {
        crate::objects::worldspace_serializer::load_cell(world, &key)?;
    }

    Ok(())
}
