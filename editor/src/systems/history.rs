use anyhow::Result;
use apostasy_core::{
    ecs::{
        cell::{CellCoord, EntityBlob, EntityId},
        components::transform::Transform,
        resources::input_manager::{InputManager, KeyAction, KeyBind},
        world::World,
    },
    start, ui::ui_context::EguiContext, update,
    winit::keyboard::{KeyCode, PhysicalKey},
    worldspaces::cell_streaming::CellMigrations,
};
use apostasy_macros::Resource;

pub trait EditorCommandClone {
    fn clone_box(&self) -> Box<dyn EditorCommand>;
}

impl<T: EditorCommand + Clone + 'static> EditorCommandClone for T {
    fn clone_box(&self) -> Box<dyn EditorCommand> {
        Box::new(self.clone())
    }
}

pub trait EditorCommand: Send + Sync + EditorCommandClone {
    fn execute(&mut self, world: &mut World) -> Result<()>;
    fn undo(&mut self, world: &mut World) -> Result<()>;
    /// Called after cell migration to update any stale EntityIds.
    fn remap_ids(&mut self, _lookup: &dyn Fn(EntityId) -> Option<EntityId>) {}
}

impl Clone for Box<dyn EditorCommand> {
    fn clone(&self) -> Box<dyn EditorCommand> {
        self.as_ref().clone_box()
    }
}

#[derive(Resource, Default, Clone)]
pub struct History {
    pub undo_stack: Vec<Box<dyn EditorCommand>>,
    pub redo_stack: Vec<Box<dyn EditorCommand>>,
}

impl History {
    pub fn push(&mut self, cmd: Box<dyn EditorCommand>) {
        self.undo_stack.push(cmd);
        self.redo_stack.clear();
    }
}

// ========== Commands ==========

#[derive(Clone)]
pub struct AddEntityCmd {
    pub blob: EntityBlob,
    pub cell: Option<CellCoord>,
    pub(crate) added_id: Option<EntityId>,
}

impl AddEntityCmd {
    pub fn new(blob: EntityBlob, cell: Option<CellCoord>) -> Self {
        Self { blob, cell, added_id: None }
    }
}

impl EditorCommand for AddEntityCmd {
    fn execute(&mut self, world: &mut World) -> Result<()> {
        let id = match self.cell {
            Some(coord) => world.spawn_from_blob_in_cell(&self.blob, coord),
            None => world.spawn_from_blob(&self.blob),
        };
        self.added_id = Some(id);
        Ok(())
    }

    fn undo(&mut self, world: &mut World) -> Result<()> {
        if let Some(id) = self.added_id.take() {
            world.despawn(id);
        }
        Ok(())
    }

    fn remap_ids(&mut self, lookup: &dyn Fn(EntityId) -> Option<EntityId>) {
        if let Some(id) = self.added_id {
            if let Some(new_id) = lookup(id) {
                self.added_id = Some(new_id);
            }
        }
    }
}

#[derive(Clone)]
pub struct RemoveEntityCmd {
    pub blob: EntityBlob,
    current_id: EntityId,
}

impl RemoveEntityCmd {
    pub fn new(id: EntityId, world: &World) -> Option<Self> {
        let blob = world.capture_entity(id)?;
        Some(Self { blob, current_id: id })
    }
}

impl EditorCommand for RemoveEntityCmd {
    fn execute(&mut self, world: &mut World) -> Result<()> {
        world.despawn(self.current_id);
        Ok(())
    }

    fn undo(&mut self, world: &mut World) -> Result<()> {
        self.current_id = world.spawn_from_blob(&self.blob);
        Ok(())
    }

    fn remap_ids(&mut self, lookup: &dyn Fn(EntityId) -> Option<EntityId>) {
        if let Some(new_id) = lookup(self.current_id) {
            self.current_id = new_id;
        }
    }
}

#[derive(Clone)]
pub struct RenameEntityCmd {
    pub id: EntityId,
    pub old_name: String,
    pub new_name: String,
}

impl EditorCommand for RenameEntityCmd {
    fn execute(&mut self, world: &mut World) -> Result<()> {
        world.set_name(self.id, &self.new_name);
        Ok(())
    }

    fn undo(&mut self, world: &mut World) -> Result<()> {
        world.set_name(self.id, &self.old_name);
        Ok(())
    }

    fn remap_ids(&mut self, lookup: &dyn Fn(EntityId) -> Option<EntityId>) {
        if let Some(new_id) = lookup(self.id) {
            self.id = new_id;
        }
    }
}

#[derive(Clone)]
pub struct MoveEntityCmd {
    pub id: EntityId,
    pub old_transform: Transform,
    pub new_transform: Transform,
}

impl EditorCommand for MoveEntityCmd {
    fn execute(&mut self, world: &mut World) -> Result<()> {
        if let Some(t) = world.get_component_mut::<Transform>(self.id) {
            *t = self.new_transform.clone();
        }
        Ok(())
    }

    fn undo(&mut self, world: &mut World) -> Result<()> {
        if let Some(t) = world.get_component_mut::<Transform>(self.id) {
            *t = self.old_transform.clone();
        }
        Ok(())
    }

    fn remap_ids(&mut self, lookup: &dyn Fn(EntityId) -> Option<EntityId>) {
        if let Some(new_id) = lookup(self.id) {
            self.id = new_id;
        }
    }
}

#[derive(Clone)]
pub struct SetParentCmd {
    pub id: EntityId,
    pub old_parent: Option<EntityId>,
    pub new_parent: Option<EntityId>,
}

impl EditorCommand for SetParentCmd {
    fn execute(&mut self, world: &mut World) -> Result<()> {
        world.set_parent(self.id, self.new_parent)
    }

    fn undo(&mut self, world: &mut World) -> Result<()> {
        world.set_parent(self.id, self.old_parent)
    }

    fn remap_ids(&mut self, lookup: &dyn Fn(EntityId) -> Option<EntityId>) {
        if let Some(new_id) = lookup(self.id) {
            self.id = new_id;
        }
        if let Some(old_parent) = self.old_parent
            && let Some(new_id) = lookup(old_parent)
        {
            self.old_parent = Some(new_id);
        }
        if let Some(new_parent) = self.new_parent
            && let Some(new_id) = lookup(new_parent)
        {
            self.new_parent = Some(new_id);
        }
    }
}

// ========== Systems ==========

#[start(mode = "editor")]
pub fn init_history(world: &mut World) -> Result<()> {
    world.insert_resource(History::default());

    let inputs = world.get_resource_mut::<InputManager>().unwrap();
    inputs.register_default_keybind(
        "Undo",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyZ), KeyAction::Press).with_ctrl(),
    );
    inputs.register_default_keybind(
        "Redo",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyY), KeyAction::Press).with_ctrl(),
    );
    Ok(())
}

#[update(mode = "editor", priority = 10000)]
pub fn remap_history_after_migration(world: &mut World) -> Result<()> {
    let remap = match world.get_resource::<CellMigrations>() {
        Ok(m) if !m.remap.is_empty() => m.remap.clone(),
        _ => return Ok(()),
    };

    let lookup = |id: EntityId| remap.get(&id).copied();

    if let Ok(history) = world.get_resource_mut::<History>() {
        for cmd in history.undo_stack.iter_mut().chain(history.redo_stack.iter_mut()) {
            cmd.remap_ids(&lookup);
        }
    }

    Ok(())
}

#[update(mode = "editor")]
pub fn handle_undo_redo(world: &mut World) -> Result<()> {
    let wants_keyboard = world
        .get_resource::<EguiContext>()
        .map(|ctx| ctx.0.egui_wants_keyboard_input())
        .unwrap_or(false);
    let (undo, redo) = {
        let inputs = world.get_resource::<InputManager>()?;
        (
            !wants_keyboard && inputs.is_keybind_active("Undo"),
            !wants_keyboard && inputs.is_keybind_active("Redo"),
        )
    };

    if undo {
        let cmd = world.get_resource_mut::<History>()?.undo_stack.pop();
        if let Some(mut cmd) = cmd {
            cmd.undo(world)?;
            world.get_resource_mut::<History>()?.redo_stack.push(cmd);
        }
    } else if redo {
        let cmd = world.get_resource_mut::<History>()?.redo_stack.pop();
        if let Some(mut cmd) = cmd {
            cmd.execute(world)?;
            world.get_resource_mut::<History>()?.undo_stack.push(cmd);
        }
    }

    Ok(())
}
