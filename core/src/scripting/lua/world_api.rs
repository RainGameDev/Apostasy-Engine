use cgmath::Vector3;
use kira::{AudioManager, AudioManagerSettings, DefaultBackend};
use mlua::{LuaSerdeExt, MetaMethod, UserData, UserDataMethods, UserDataRef};
use serde_yaml::Value as YamlValue;

use super::component::{LuaComponentRegistry, ScriptComponents};
use super::query::LuaQuery;
use super::resource::LuaResources;
use crate::assets::asset_manager::AssetManager;
use crate::assets::loaders::material_loader::MaterialLoader;
use crate::audio::Audio;
use crate::audio::audio_player::AudioPlayer;
use crate::audio::sound::Sound;
use crate::ecs::components::get_component_registration;
use crate::ecs::resources::input_manager::InputManager;
use crate::ecs::systems::{DeltaTime, EngineTimer};
use crate::ecs::{World, cell::EntityId};
use crate::physics::raycast::{Ray, build_collider_snapshot, raycast_colliders_raw};

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

        // Spawns into the cell containing `position` (a vec3 or `{x, y, z}` table).
        methods.add_method("spawn_at_position", |_, this, position: mlua::Table| {
            let id = this
                .world()
                .spawn_at_position(table_to_vec3(&position))
                .id();
            Ok(EntityHandle(id))
        });

        // Spawns into a specific 128-unit cell, addressed by integer cell coords.
        methods.add_method("spawn_in_cell", |_, this, (x, y, z): (i32, i32, i32)| {
            let id = this.world().spawn_in_cell(Vector3::new(x, y, z)).id();
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

        // Casts a ray against all colliders, returning the nearest hit within
        // `max_distance` as `{ entity, point, normal, distance, face }`, or nil.
        // `origin`/`direction` are vec3s (or `{x, y, z}` tables); `ignore` is an
        // optional entity to skip (e.g. the caster itself).
        methods.add_method(
            "raycast",
            |lua,
             this,
             (origin, direction, max_distance, ignore): (
                mlua::Table,
                mlua::Table,
                f32,
                Option<UserDataRef<EntityHandle>>,
            )| {
                let ray = Ray::new(table_to_vec3(&origin), table_to_vec3(&direction));
                let world = this.world();
                let snapshots = build_collider_snapshot(world);
                let hit =
                    raycast_colliders_raw(&ray, max_distance, &snapshots, ignore.map(|i| i.0));
                match hit {
                    Some(h) => {
                        let t = lua.create_table()?;
                        t.set("entity", EntityHandle(h.entity_id))?;
                        t.set("point", vec3_to_table(lua, h.point)?)?;
                        t.set("normal", vec3_to_table(lua, h.normal)?)?;
                        t.set("distance", h.distance)?;
                        t.set("face", h.face)?;
                        Ok(mlua::Value::Table(t))
                    }
                    None => Ok(mlua::Value::Nil),
                }
            },
        );

        // ---- materials ----
        // Sets a loaded material's RGBA color (`{r, g, b, a}` map or `[r, g, b, a]`
        // sequence). The render loop reads material colors every frame, so this
        // re-tints every entity using the material immediately.
        methods.add_method(
            "set_material_color",
            |_, this, (name, color): (String, mlua::Table)| {
                let rgba = table_to_rgba(&color);
                if let Ok(am) = this.world().get_resource::<AssetManager>()
                    && let Some(loader) = am.get_loader::<MaterialLoader>()
                    && let Some(mat) = loader.registry.write().unwrap().materials.get_mut(&name)
                {
                    mat.color = rgba;
                }
                Ok(())
            },
        );

        // Returns a material's current color as an `[r, g, b, a]` sequence, or nil
        // if no material with that name is loaded.
        methods.add_method("get_material_color", |lua, this, name: String| {
            let color = this
                .world()
                .get_resource::<AssetManager>()
                .ok()
                .and_then(|am| am.get_loader::<MaterialLoader>())
                .and_then(|loader| {
                    loader
                        .registry
                        .read()
                        .unwrap()
                        .materials
                        .get(&name)
                        .map(|m| m.color)
                });
            match color {
                Some(c) => {
                    let t = lua.create_table()?;
                    for (i, v) in c.iter().enumerate() {
                        t.set(i + 1, *v)?;
                    }
                    Ok(mlua::Value::Table(t))
                }
                None => Ok(mlua::Value::Nil),
            }
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

        // ---- audio ----
        // Sets pending_play on an entity's AudioPlayer component.
        // `index` is 0-based. The existing audio_player_pending system processes
        // it next frame.
        methods.add_method(
            "audio_play",
            |_, this, (id, index): (UserDataRef<EntityHandle>, usize)| {
                if let Some(player) = this.world().get_component_mut::<AudioPlayer>(id.0) {
                    player.pending_play = Some(index);
                }
                Ok(())
            },
        );

        // Sets pending_stop on an entity's AudioPlayer component.
        methods.add_method(
            "audio_stop",
            |_, this, (id, index): (UserDataRef<EntityHandle>, usize)| {
                if let Some(player) = this.world().get_component_mut::<AudioPlayer>(id.0) {
                    player.pending_stop = Some(index);
                }
                Ok(())
            },
        );

        // Plays a sound file directly without requiring an entity.
        // `path` is resolved the same way as Sound::from_path.
        // `volume_db` is optional (defaults to 0.0).
        methods.add_method(
            "audio_play_oneshot",
            |_, this, (path, volume_db): (String, Option<f32>)| {
                let world = this.world();
                if !world.has_resource::<Audio>() {
                    match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()) {
                        Ok(mgr) => {
                            world.insert_resource(Audio {
                                manager: std::sync::Arc::new(std::sync::Mutex::new(mgr)),
                            });
                        }
                        Err(e) => {
                            crate::log_warn!("[lua] audio_play_oneshot: failed to create AudioManager: {e}");
                            return Ok(());
                        }
                    }
                }
                let sound = match Sound::from_path(&path) {
                    Ok(s) => s,
                    Err(e) => {
                        crate::log_warn!("[lua] audio_play_oneshot: {e}");
                        return Ok(());
                    }
                };
                let data = match sound.data {
                    Some(d) => d.volume(kira::Decibels(volume_db.unwrap_or(0.0))),
                    None => return Ok(()),
                };
                if let Ok(audio) = world.get_resource::<Audio>()
                    && let Ok(mut mgr) = audio.manager.lock()
                {
                    if let Err(e) = mgr.play(data) {
                        crate::log_warn!("[lua] audio_play_oneshot: play failed: {e}");
                    }
                }
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

/// Reads a Lua value as a `Vector3<f32>`. Accepts both the `{x, y, z}` keyed
/// form and the `[x, y, z]` sequence form (a `vec3` from the prelude satisfies
/// both via its metatable), defaulting any missing component to 0.
fn table_to_vec3(t: &mlua::Table) -> Vector3<f32> {
    let component = |key: &str, idx: i64| {
        t.get::<f32>(key)
            .ok()
            .or_else(|| t.get::<f32>(idx).ok())
            .unwrap_or(0.0)
    };
    Vector3::new(component("x", 1), component("y", 2), component("z", 3))
}

/// converts a table to rgba values.
fn table_to_rgba(t: &mlua::Table) -> [f32; 4] {
    let channel = |key: &str, idx: i64| {
        t.get::<f32>(key)
            .ok()
            .or_else(|| t.get::<f32>(idx).ok())
            .unwrap_or(1.0)
    };
    [
        channel("r", 1),
        channel("g", 2),
        channel("b", 3),
        channel("a", 4),
    ]
}

/// Converts a vec3 to a lua table.
fn vec3_to_table(lua: &mlua::Lua, v: Vector3<f32>) -> mlua::Result<mlua::Table> {
    let t = lua.create_table()?;
    t.set(1, v.x)?;
    t.set(2, v.y)?;
    t.set(3, v.z)?;
    Ok(t)
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
