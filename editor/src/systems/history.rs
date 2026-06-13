use anyhow::Result;
use apostasy_core::{
    objects::{
        Object,
        cell::{CellCoord, ObjectId},
        cell_streaming::CellMigrations,
        components::transform::Transform,
        resources::input_manager::{InputManager, KeyAction, KeyBind},
        world::World,
    },
    start, update,
    winit::keyboard::{KeyCode, PhysicalKey},
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
    /// Called after cell migration to update any stale ObjectIds.
    fn remap_ids(&mut self, _lookup: &dyn Fn(ObjectId) -> Option<ObjectId>) {}
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
pub struct AddObjectCmd {
    pub object: Object,
    pub cell: Option<CellCoord>,
    added_id: Option<ObjectId>,
}

impl AddObjectCmd {
    pub fn new(object: Object, cell: Option<CellCoord>) -> Self {
        Self { object, cell, added_id: None }
    }
}

impl EditorCommand for AddObjectCmd {
    fn execute(&mut self, world: &mut World) -> Result<()> {
        let id = match self.cell {
            Some(coord) => world.add_object_to_cell(coord, self.object.clone()),
            None => world.add_object(self.object.clone()),
        };
        self.added_id = Some(id);
        Ok(())
    }

    fn undo(&mut self, world: &mut World) -> Result<()> {
        if let Some(id) = self.added_id.take() {
            world.remove_object(id);
        }
        Ok(())
    }

    fn remap_ids(&mut self, lookup: &dyn Fn(ObjectId) -> Option<ObjectId>) {
        if let Some(id) = self.added_id {
            if let Some(new_id) = lookup(id) {
                self.added_id = Some(new_id);
            }
        }
    }
}

#[derive(Clone)]
pub struct RemoveObjectCmd {
    pub object: Object,
    // Tracks the live ObjectId: starts as the original, updated each time undo re-adds the object.
    // Slotmap assigns a new key on every insertion, so we must follow it across undo/redo cycles.
    current_id: ObjectId,
}

impl RemoveObjectCmd {
    pub fn new(id: ObjectId, world: &World) -> Option<Self> {
        let object = world.get_object(id)?.clone();
        Some(Self { object, current_id: id })
    }
}

impl EditorCommand for RemoveObjectCmd {
    fn execute(&mut self, world: &mut World) -> Result<()> {
        world.remove_object(self.current_id);
        Ok(())
    }

    fn undo(&mut self, world: &mut World) -> Result<()> {
        self.current_id = world.add_object(self.object.clone());
        Ok(())
    }

    fn remap_ids(&mut self, lookup: &dyn Fn(ObjectId) -> Option<ObjectId>) {
        if let Some(new_id) = lookup(self.current_id) {
            self.current_id = new_id;
        }
    }
}

#[derive(Clone)]
pub struct RenameObjectCmd {
    pub id: ObjectId,
    pub old_name: String,
    pub new_name: String,
}

impl EditorCommand for RenameObjectCmd {
    fn execute(&mut self, world: &mut World) -> Result<()> {
        if let Some(obj) = world.get_object_mut(self.id) {
            obj.name = self.new_name.clone();
        }
        Ok(())
    }

    fn undo(&mut self, world: &mut World) -> Result<()> {
        if let Some(obj) = world.get_object_mut(self.id) {
            obj.name = self.old_name.clone();
        }
        Ok(())
    }

    fn remap_ids(&mut self, lookup: &dyn Fn(ObjectId) -> Option<ObjectId>) {
        if let Some(new_id) = lookup(self.id) {
            self.id = new_id;
        }
    }
}

#[derive(Clone)]
pub struct MoveObjectCmd {
    pub id: ObjectId,
    pub old_transform: Transform,
    pub new_transform: Transform,
}

impl EditorCommand for MoveObjectCmd {
    fn execute(&mut self, world: &mut World) -> Result<()> {
        if let Some(obj) = world.get_object_mut(self.id) {
            if let Ok(t) = obj.get_component_mut::<Transform>() {
                *t = self.new_transform.clone();
            }
        }
        Ok(())
    }

    fn undo(&mut self, world: &mut World) -> Result<()> {
        if let Some(obj) = world.get_object_mut(self.id) {
            if let Ok(t) = obj.get_component_mut::<Transform>() {
                *t = self.old_transform.clone();
            }
        }
        Ok(())
    }

    fn remap_ids(&mut self, lookup: &dyn Fn(ObjectId) -> Option<ObjectId>) {
        if let Some(new_id) = lookup(self.id) {
            self.id = new_id;
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

    let lookup = |id: ObjectId| remap.get(&id).copied();

    if let Ok(history) = world.get_resource_mut::<History>() {
        for cmd in history.undo_stack.iter_mut().chain(history.redo_stack.iter_mut()) {
            cmd.remap_ids(&lookup);
        }
    }

    Ok(())
}

#[update(mode = "editor")]
pub fn handle_undo_redo(world: &mut World) -> Result<()> {
    let (undo, redo) = {
        let inputs = world.get_resource::<InputManager>()?;
        (inputs.is_keybind_active("Undo"), inputs.is_keybind_active("Redo"))
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
