use mlua::{MetaMethod, UserData, UserDataMethods, UserDataRef};

use crate::ecs::{World, cell::EntityId};

/// An entity reference for lua.
#[derive(Copy, Clone)]
pub struct EntityHandle(pub EntityId);

impl UserData for EntityHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::ToString, |_, this, ()| {
            Ok(format!(
                "Entity({}:{})",
                this.0.entity.index, this.0.entity.generation
            ))
        });
        methods.add_meta_method(
            MetaMethod::Eq,
            |_, this, other: UserDataRef<EntityHandle>| Ok(this.0 == other.0),
        );
    }
}

/// A scoped, per-call view of the engine World. The raw pointer is only valid
/// for the duration of one Lua call (enforced by `lua.scope`).
pub struct WorldHandle {
    pub world: *mut World,
}

impl WorldHandle {
    /// SAFETY: valid only while inside the scope that created this userdata.
    #[allow(clippy::mut_from_ref)]
    fn world(&self) -> &mut World {
        unsafe { &mut *self.world }
    }
}

impl UserData for WorldHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("log", |_, _this, msg: String| {
            tracing::info!("[lua] {msg}");
            Ok(())
        });

        methods.add_method("spawn", |_, this, ()| {
            let id = this.world().spawn().id();
            Ok(EntityHandle(id))
        });

        methods.add_method("despawn", |_, this, id: UserDataRef<EntityHandle>| {
            this.world().despawn(id.0);
            Ok(())
        });

        methods.add_method(
            "set_name",
            |_, this, (id, name): (UserDataRef<EntityHandle>, String)| {
                this.world().set_name(id.0, &name);
                Ok(())
            },
        );

        methods.add_method(
            "add_tag",
            |_, this, (id, name): (UserDataRef<EntityHandle>, String)| {
                let _ = this.world().add_tag_by_name(id.0, &name);
                Ok(())
            },
        );

        methods.add_method(
            "remove_tag",
            |_, this, (id, name): (UserDataRef<EntityHandle>, String)| {
                this.world().remove_tag_by_name(id.0, &name);
                Ok(())
            },
        );

        methods.add_method("log_warn", |_, _this, msg: String| {
            crate::log_warn!("[lua] {msg}");
            Ok(())
        });
        methods.add_method("log_error", |_, _this, msg: String| {
            crate::log_error!("[lua] {msg}");
            Ok(())
        });
        methods.add_method("log", |_, _this, msg: String| {
            crate::log!("[lua] {msg}");
            Ok(())
        });
    }
}
