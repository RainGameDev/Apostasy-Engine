use mlua::{LuaSerdeExt, MetaMethod, UserData, UserDataMethods, UserDataRef};
use serde_yaml::Value as YamlValue;

use super::component::{LuaComponentRegistry, ScriptComponents};
use super::query::LuaQuery;
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

        methods.add_method(
            "add_component",
            |lua,
             this,
             (id, name, overrides): (UserDataRef<EntityHandle>, String, Option<mlua::Value>)| {
                let world = this.world();

                // Start from the registered defaults (if any), else an empty table.
                let default = world
                    .get_resource::<LuaComponentRegistry>()
                    .ok()
                    .and_then(|r| r.default_for(&name).cloned())
                    .unwrap_or(YamlValue::Mapping(Default::default()));

                // Overlay any fields the caller passed.
                let value = match overrides {
                    Some(v) => merge_yaml(&default, &lua.from_value(v)?),
                    None => default,
                };

                if !world.has_component::<ScriptComponents>(id.0) {
                    world.add_component(id.0, ScriptComponents::default());
                }
                if let Some(sc) = world.get_component_mut::<ScriptComponents>(id.0) {
                    sc.set(&name, value);
                }
                Ok(())
            },
        );

        methods.add_method(
            "get_component",
            |lua, this, (id, name): (UserDataRef<EntityHandle>, String)| {
                let value = this
                    .world()
                    .get_component::<ScriptComponents>(id.0)
                    .and_then(|sc| sc.get(&name).cloned());
                match value {
                    Some(v) => lua.to_value(&v),
                    None => Ok(mlua::Value::Nil),
                }
            },
        );

        methods.add_method(
            "set_component",
            |lua,
             this,
             (id, name, table): (UserDataRef<EntityHandle>, String, mlua::Value)| {
                let value: YamlValue = lua.from_value(table)?;
                let world = this.world();
                if !world.has_component::<ScriptComponents>(id.0) {
                    world.add_component(id.0, ScriptComponents::default());
                }
                if let Some(sc) = world.get_component_mut::<ScriptComponents>(id.0) {
                    sc.set(&name, value);
                }
                Ok(())
            },
        );

        methods.add_method(
            "remove_component",
            |_, this, (id, name): (UserDataRef<EntityHandle>, String)| {
                if let Some(sc) = this.world().get_component_mut::<ScriptComponents>(id.0) {
                    sc.remove(&name);
                }
                Ok(())
            },
        );

        methods.add_method(
            "has_component",
            |_, this, (id, name): (UserDataRef<EntityHandle>, String)| {
                Ok(this
                    .world()
                    .get_component::<ScriptComponents>(id.0)
                    .is_some_and(|sc| sc.has(&name)))
            },
        );

        methods.add_method("query", |_, this, names: mlua::Variadic<String>| {
            Ok(LuaQuery::new(this.world, names.into_iter().collect()))
        });

        methods.add_method(
            "has_tag",
            |_, this, (id, name): (UserDataRef<EntityHandle>, String)| {
                Ok(this.world().has_tag_by_name(id.0, &name))
            },
        );

        methods.add_method("get_entity_with_tag", |_, this, name: String| {
            Ok(this
                .world()
                .get_entities_with_tag_by_name(&name)
                .first()
                .copied()
                .map(EntityHandle))
        });

        methods.add_method("get_entities_with_tag", |lua, this, name: String| {
            let ids = this.world().get_entities_with_tag_by_name(&name);
            let t = lua.create_table()?;
            for (i, id) in ids.into_iter().enumerate() {
                t.set(i + 1, EntityHandle(id))?;
            }
            Ok(t)
        });

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

/// Overlays `over`'s fields onto a clone of `base` (shallow). Lets
/// `add_component(id, "Health", { current = 50 })` keep the registered `max`.
fn merge_yaml(base: &YamlValue, over: &YamlValue) -> YamlValue {
    match (base, over) {
        (YamlValue::Mapping(b), YamlValue::Mapping(o)) => {
            let mut m = b.clone();
            for (k, v) in o {
                m.insert(k.clone(), v.clone());
            }
            YamlValue::Mapping(m)
        }
        _ => over.clone(),
    }
}
