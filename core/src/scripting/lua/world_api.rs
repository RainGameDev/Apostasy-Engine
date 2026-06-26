use mlua::{LuaSerdeExt, MetaMethod, UserData, UserDataMethods, UserDataRef};
use serde_yaml::Value as YamlValue;

use super::component::{LuaComponentRegistry, ScriptComponents};
use super::query::LuaQuery;
use super::resource::LuaResources;
use crate::ecs::components::get_component_registration;
use crate::ecs::resources::input_manager::InputManager;
use crate::ecs::systems::{DeltaTime, EngineTimer};
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

        methods.add_method("get_name", |_, this, id: UserDataRef<EntityHandle>| {
            Ok(this.world().get_name(id.0).map(str::to_string))
        });

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

        // ---- components ----
        methods.add_method(
            "add_component",
            |lua,
             this,
             (id, name, overrides): (UserDataRef<EntityHandle>, String, Option<mlua::Value>)| {
                let world = this.world();

                // Native component: insert a default, then overlay any passed fields.
                if let Some(reg) = get_component_registration(&name) {
                    let value: YamlValue = match overrides {
                        Some(v) => lua.from_value(v)?,
                        None => YamlValue::Mapping(Default::default()),
                    };
                    (reg.apply)(world, id.0, &value);
                    return Ok(());
                }

                // Script component: start from registered defaults, overlay fields.
                let default = world
                    .get_resource::<LuaComponentRegistry>()
                    .ok()
                    .and_then(|r| r.default_for(&name).cloned())
                    .unwrap_or(YamlValue::Mapping(Default::default()));
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
                let world = this.world();
                let value = match get_component_registration(&name) {
                    Some(reg) => (reg.read)(world, id.0),
                    None => world
                        .get_component::<ScriptComponents>(id.0)
                        .and_then(|sc| sc.get(&name).cloned()),
                };
                match value {
                    Some(v) => lua.to_value(&v),
                    None => Ok(mlua::Value::Nil),
                }
            },
        );

        methods.add_method(
            "set_component",
            |lua, this, (id, name, table): (UserDataRef<EntityHandle>, String, mlua::Value)| {
                let value: YamlValue = lua.from_value(table)?;
                let world = this.world();
                if let Some(reg) = get_component_registration(&name) {
                    (reg.apply)(world, id.0, &value);
                    return Ok(());
                }
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
                let world = this.world();
                if let Some(reg) = get_component_registration(&name) {
                    (reg.remove)(world, id.0);
                    return Ok(());
                }
                if let Some(sc) = world.get_component_mut::<ScriptComponents>(id.0) {
                    sc.remove(&name);
                }
                Ok(())
            },
        );

        methods.add_method(
            "has_component",
            |_, this, (id, name): (UserDataRef<EntityHandle>, String)| {
                let world = this.world();
                if let Some(reg) = get_component_registration(&name) {
                    return Ok((reg.contains)(world, id.0));
                }
                Ok(world
                    .get_component::<ScriptComponents>(id.0)
                    .is_some_and(|sc| sc.has(&name)))
            },
        );

        // ---- time ----
        methods.add_method("delta", |_, this, ()| {
            Ok(this
                .world()
                .get_resource::<DeltaTime>()
                .map(|d| d.0)
                .unwrap_or(0.0))
        });
        methods.add_method("time", |_, this, ()| {
            Ok(this
                .world()
                .get_resource::<EngineTimer>()
                .map(|t| t.0)
                .unwrap_or(0.0))
        });

        // ---- global script resources ----
        methods.add_method("remove_resource", |_, this, name: String| {
            if let Ok(r) = this.world().get_resource_mut::<LuaResources>() {
                r.remove(&name);
            }
            Ok(())
        });

        methods.add_method("has_resource", |_, this, name: String| {
            Ok(this
                .world()
                .get_resource::<LuaResources>()
                .map(|r| r.has(&name))
                .unwrap_or(false))
        });

        methods.add_method("get_resource", |lua, this, name: String| {
            let value = this
                .world()
                .get_resource::<LuaResources>()
                .ok()
                .and_then(|r| r.get(&name).cloned());
            match value {
                Some(v) => lua.to_value(&v),
                None => Ok(mlua::Value::Nil),
            }
        });
        methods.add_method(
            "set_resource",
            |lua, this, (name, table): (String, mlua::Value)| {
                let value: YamlValue = lua.from_value(table)?;
                if let Ok(r) = this.world().get_resource_mut::<LuaResources>() {
                    r.set(&name, value);
                }
                Ok(())
            },
        );

        // ---- input ----
        methods.add_method("is_keybind_active", |_, this, name: String| {
            Ok(this
                .world()
                .get_resource::<InputManager>()
                .map(|im| im.is_keybind_active(&name))
                .unwrap_or(false))
        });
        methods.add_method("is_mousebind_active", |_, this, name: String| {
            Ok(this
                .world()
                .get_resource::<InputManager>()
                .map(|im| im.is_mousebind_active(&name))
                .unwrap_or(false))
        });
        methods.add_method(
            "input_vector_2d",
            |lua, this, (left, right, up, down): (String, String, String, String)| {
                let v = this
                    .world()
                    .get_resource::<InputManager>()
                    .map(|im| im.input_vector_2d(&left, &right, &up, &down))
                    .unwrap_or_else(|_| cgmath::Vector2::new(0.0, 0.0));
                let t = lua.create_table()?;
                t.set("x", v.x)?;
                t.set("y", v.y)?;
                Ok(t)
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
            ids_to_table(lua, this.world().get_entities_with_tag_by_name(&name))
        });

        // ---- hierarchy ----
        methods.add_method(
            "set_parent",
            |_, this, (child, parent): (UserDataRef<EntityHandle>, UserDataRef<EntityHandle>)| {
                let _ = this.world().set_parent(child.0, Some(parent.0));
                Ok(())
            },
        );

        methods.add_method("detach", |_, this, id: UserDataRef<EntityHandle>| {
            let _ = this.world().detach(id.0);
            Ok(())
        });

        methods.add_method("get_parent", |_, this, id: UserDataRef<EntityHandle>| {
            Ok(this.world().get_parent_id(id.0).map(EntityHandle))
        });

        methods.add_method(
            "get_children",
            |lua, this, id: UserDataRef<EntityHandle>| {
                ids_to_table(lua, this.world().get_children_ids(id.0))
            },
        );

        methods.add_method(
            "get_ancestors",
            |lua, this, id: UserDataRef<EntityHandle>| {
                ids_to_table(lua, this.world().get_ancestors(id.0))
            },
        );

        methods.add_method(
            "get_descendants",
            |lua, this, id: UserDataRef<EntityHandle>| {
                ids_to_table(lua, this.world().get_descendants(id.0))
            },
        );

        methods.add_method("get_root_entities", |lua, this, ()| {
            ids_to_table(lua, this.world().get_root_ids())
        });

        methods.add_method("get_all_entities", |lua, this, ()| {
            ids_to_table(lua, this.world().get_all_ids())
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

/// Builds a 1-indexed Lua array of `EntityHandle`s from a list of ids.
fn ids_to_table(lua: &mlua::Lua, ids: Vec<EntityId>) -> mlua::Result<mlua::Table> {
    let t = lua.create_table()?;
    for (i, id) in ids.into_iter().enumerate() {
        t.set(i + 1, EntityHandle(id))?;
    }
    Ok(t)
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
