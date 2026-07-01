use cgmath::Vector3;
use mlua::{LuaSerdeExt, MetaMethod, UserData, UserDataMethods, UserDataRef};
use serde_yaml::Value as YamlValue;

use super::component::{LuaComponentRegistry, ScriptComponents};
use super::query::LuaQuery;
use super::resource::LuaResources;
use crate::assets::asset_manager::AssetManager;
use crate::assets::loaders::material_loader::MaterialLoader;
use crate::audio::Audio;
use crate::audio::audio_player::AudioPlayer;
use crate::audio::layers::AudioLayer;
use crate::audio::sound::Sound;
use crate::ecs::components::get_component_registration;
use crate::ecs::resources::input_manager::{
    InputManager, KeyBind, ModifierKeys, MouseBind, RepeatConfig,
};
use crate::ecs::systems::{DeltaTime, EngineTimer};
use crate::ecs::{World, cell::EntityId};
use crate::packages::project_package::load_startup_worldspace;
use crate::physics::raycast::{Ray, build_collider_snapshot, raycast_colliders_raw};
use crate::scripting::lua::ui_api::invoke_with_ui;
use crate::ui::ui_context::EguiContext;

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
        methods.add_method(
            "input_vector_3d",
            |lua,
             this,
             (x_pos, x_neg, y_pos, y_neg, z_pos, z_neg): (
                String,
                String,
                String,
                String,
                String,
                String,
            )| {
                let v = this
                    .world()
                    .get_resource::<InputManager>()
                    .map(|im| im.input_vector_3d(&x_pos, &x_neg, &y_pos, &y_neg, &z_pos, &z_neg))
                    .unwrap_or_else(|_| cgmath::Vector3::new(0.0, 0.0, 0.0));
                let t = lua.create_table()?;
                t.set("x", v.x)?;
                t.set("y", v.y)?;
                t.set("z", v.z)?;
                Ok(t)
            },
        );

        methods.add_method("mouse_position", |lua, this, ()| {
            let pos = this
                .world()
                .get_resource::<InputManager>()
                .map(|im| im.mouse_position)
                .unwrap_or_default();
            let t = lua.create_table()?;
            t.set("x", pos.x)?;
            t.set("y", pos.y)?;
            Ok(t)
        });

        methods.add_method("mouse_delta", |lua, this, ()| {
            let delta = this
                .world()
                .get_resource::<InputManager>()
                .map(|im| im.mouse_delta)
                .unwrap_or((0.0, 0.0));
            let t = lua.create_table()?;
            t.set("x", delta.0)?;
            t.set("y", delta.1)?;
            Ok(t)
        });

        methods.add_method("scroll_delta", |lua, this, ()| {
            let delta = this
                .world()
                .get_resource::<InputManager>()
                .map(|im| im.scroll_delta)
                .unwrap_or((0.0, 0.0));
            let t = lua.create_table()?;
            t.set("x", delta.0)?;
            t.set("y", delta.1)?;
            Ok(t)
        });

        // Marks `name` as an active input context for this frame; binds with a
        // matching `context` only fire while their context is active.
        methods.add_method("set_input_context", |_, this, name: String| {
            if let Ok(im) = this.world().get_resource_mut::<InputManager>() {
                im.active_contexts.insert(name);
            }
            Ok(())
        });

        // Registers a new keybind. Fails (returns false) if `name` is already
        // registered use `rebind_key` to overwrite an existing bind.
        //
        // ```lua
        // world:register_keybind("Jump", "Space", "Press")
        // world:register_keybind("Save", "KeyS", "Press", { ctrl = true })
        // world:register_keybind("Sprint", "ShiftLeft", "Hold")
        // world:register_keybind("Interact", "KeyE", "Press", { context = "gameplay" })
        // ```
        methods.add_method(
            "register_keybind",
            |_, this, (name, key, action, opts): (String, String, String, Option<mlua::Table>)| {
                let bind = lua_to_keybind(&key, &action, opts.as_ref())?;
                let world = this.world();
                let Ok(im) = world.get_resource_mut::<InputManager>() else {
                    return Ok(false);
                };
                match im.register_keybind(name.clone(), bind) {
                    Ok(()) => Ok(true),
                    Err(e) => {
                        crate::log_error!("[lua] register_keybind('{name}') failed: {e}");
                        Ok(false)
                    }
                }
            },
        );

        // Registers a default keybind only takes effect if `name` has no bind
        // yet (e.g. nothing was loaded from the keybinds file). Use this for
        // bootstrapping defaults; never errors.
        methods.add_method(
            "register_default_keybind",
            |_, this, (name, key, action, opts): (String, String, String, Option<mlua::Table>)| {
                let bind = lua_to_keybind(&key, &action, opts.as_ref())?;
                if let Ok(im) = this.world().get_resource_mut::<InputManager>() {
                    im.register_default_keybind(name, bind);
                }
                Ok(())
            },
        );

        // Overwrites the bind for `name`, persisting the change use for
        // remapping from a settings/preferences UI.
        methods.add_method(
            "rebind_key",
            |_, this, (name, key, action, opts): (String, String, String, Option<mlua::Table>)| {
                let bind = lua_to_keybind(&key, &action, opts.as_ref())?;
                if let Ok(im) = this.world().get_resource_mut::<InputManager>() {
                    im.rebind_key(name, bind);
                }
                Ok(())
            },
        );

        // Mouse-button equivalents of the keybind methods above.
        methods.add_method(
            "register_mousebind",
            |_,
             this,
             (name, button, action, opts): (String, String, String, Option<mlua::Table>)| {
                let bind = lua_to_mousebind(&button, &action, opts.as_ref())?;
                let world = this.world();
                let Ok(im) = world.get_resource_mut::<InputManager>() else {
                    return Ok(false);
                };
                match im.register_mousebind(name.clone(), bind) {
                    Ok(()) => Ok(true),
                    Err(e) => {
                        crate::log_error!("[lua] register_mousebind('{name}') failed: {e}");
                        Ok(false)
                    }
                }
            },
        );

        methods.add_method(
            "register_default_mousebind",
            |_,
             this,
             (name, button, action, opts): (String, String, String, Option<mlua::Table>)| {
                let bind = lua_to_mousebind(&button, &action, opts.as_ref())?;
                if let Ok(im) = this.world().get_resource_mut::<InputManager>() {
                    im.register_default_mousebind(name, bind);
                }
                Ok(())
            },
        );

        methods.add_method(
            "rebind_mouse",
            |_,
             this,
             (name, button, action, opts): (String, String, String, Option<mlua::Table>)| {
                let bind = lua_to_mousebind(&button, &action, opts.as_ref())?;
                if let Ok(im) = this.world().get_resource_mut::<InputManager>() {
                    im.rebind_mouse(name, bind);
                }
                Ok(())
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
                Option<Vec<UserDataRef<EntityHandle>>>,
            )| {
                let ray = Ray::new(table_to_vec3(&origin), table_to_vec3(&direction));
                let world = this.world();
                let snapshots = build_collider_snapshot(world);
                let ignores: Vec<_> = ignore
                    .unwrap_or_default()
                    .chunks(1)
                    .filter_map(|c| c.first())
                    .map(|i| i.0)
                    .collect();
                let hit = raycast_colliders_raw(&ray, max_distance, &snapshots, Some(ignores));
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
                {
                    let mut registry = loader.registry.write();
                    if let Some(mat) = registry.materials.get_mut(&name) {
                        mat.color = rgba;
                        registry.version = registry.version.wrapping_add(1);
                    }
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
                .and_then(|loader| loader.registry.read().materials.get(&name).map(|m| m.color));
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

        // ---- worldspaces ----
        // Loads a worldspace/worldspace by name, despawning the current scene's
        // entities first. `retain_tags` is an optional list of tag names whose
        // entities survive the switch (e.g. {"Player"}); pass nothing to clear
        // everything. Returns true if the worldspace was found and loaded.
        methods.add_method(
            "load_worldspace",
            |_, this, (name, retain_tags): (String, Option<Vec<String>>)| {
                let tags = retain_tags.unwrap_or_default();
                let tag_refs: Vec<&str> = tags.iter().map(String::as_str).collect();
                match load_startup_worldspace(this.world(), &name, &tag_refs) {
                    Ok(()) => Ok(true),
                    Err(e) => {
                        crate::log_error!("[lua] load_worldspace('{name}') failed: {e}");
                        Ok(false)
                    }
                }
            },
        );

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

        methods.add_method(
            "audio_pause",
            |_, this, (id, index): (UserDataRef<EntityHandle>, usize)| {
                if let Some(player) = this.world().get_component_mut::<AudioPlayer>(id.0) {
                    player.pending_pause = Some(index);
                }
                Ok(())
            },
        );

        methods.add_method(
            "audio_resume",
            |_, this, (id, index): (UserDataRef<EntityHandle>, usize)| {
                if let Some(player) = this.world().get_component_mut::<AudioPlayer>(id.0) {
                    player.pending_resume = Some(index);
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
                    match Audio::new() {
                        Ok(audio) => {
                            world.insert_resource(audio);
                        }
                        Err(e) => {
                            crate::log_warn!(
                                "[lua] audio_play_oneshot: failed to create AudioManager: {e}"
                            );
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
                if let Ok(audio) = world.get_resource::<Audio>() {
                    if let Err(e) = audio.play_sound(AudioLayer::default(), data) {
                        crate::log_warn!("[lua] audio_play_oneshot: play failed: {e}");
                    }
                }
                Ok(())
            },
        );

        // Adds a Sound to an entity's AudioPlayer component and returns its 0-based index.
        // opts is an optional table with any of: layer, volume, pitch, loop, spatial,
        // max_distance, fade_in, fade_out, auto_play (all optional).
        //
        // Usage:
        //   local idx = world:audio_add_sound(entity, "audio/click.ogg")
        //   local idx = world:audio_add_sound(entity, "audio/theme.ogg", { layer="Music", loop=true, volume=-6 })
        //   world:audio_play(entity, idx)
        methods.add_method(
            "audio_add_sound",
            |_, this, (id, path, opts): (UserDataRef<EntityHandle>, String, Option<mlua::Table>)| {
                let mut sound = Sound::default();
                sound.path = path;

                if let Some(t) = &opts {
                    if let Ok(l) = t.get::<String>("layer")
                        && let Some(layer) = AudioLayer::from_str(&l)
                    {
                        sound.layer = layer;
                    }
                    if let Ok(v) = t.get::<f32>("volume") {
                        sound.volume = v;
                    }
                    if let Ok(p) = t.get::<f32>("pitch") {
                        sound.pitch = p;
                    }
                    if let Ok(b) = t.get::<bool>("loop") {
                        sound.looping = b;
                    }
                    if let Ok(b) = t.get::<bool>("spatial") {
                        sound.spatial = b;
                    }
                    if let Ok(d) = t.get::<f32>("max_distance") {
                        sound.max_distance = d;
                    }
                    if let Ok(f) = t.get::<f32>("fade_in") {
                        sound.fade_in = f;
                    }
                    if let Ok(f) = t.get::<f32>("fade_out") {
                        sound.fade_out = f;
                    }
                    if let Ok(b) = t.get::<bool>("auto_play") {
                        sound.auto_play = b;
                    }
                }

                if !sound.path.is_empty() {
                    if let Err(e) = sound.reload() {
                        crate::log_warn!("[lua] audio_add_sound: failed to load '{}': {e}", sound.path);
                    }
                }

                let world = this.world();
                if world.get_component::<AudioPlayer>(id.0).is_none() {
                    world.add_component(id.0, AudioPlayer::default());
                }

                let idx = if let Some(player) = world.get_component_mut::<AudioPlayer>(id.0) {
                    let idx = player.audio.len();
                    player.audio.push(sound);
                    idx
                } else {
                    return Ok(usize::MAX);
                };

                Ok(idx)
            },
        );

        // Removes the sound at `index` from an entity's AudioPlayer.
        methods.add_method(
            "audio_remove_sound",
            |_, this, (id, index): (UserDataRef<EntityHandle>, usize)| {
                if let Some(player) = this.world().get_component_mut::<AudioPlayer>(id.0) {
                    if index < player.audio.len() {
                        player.audio.remove(index);
                    }
                }
                Ok(())
            },
        );

        // Returns the number of sounds on an entity's AudioPlayer (or 0 if none).
        methods.add_method(
            "audio_sound_count",
            |_, this, id: UserDataRef<EntityHandle>| {
                Ok(this
                    .world()
                    .get_component::<AudioPlayer>(id.0)
                    .map(|p| p.audio.len())
                    .unwrap_or(0))
            },
        );

        // Plays a streaming music track (Music layer) without needing an entity.
        // opts table: volume (dB, default 0), loop (bool, default true), fade_in (secs, default 0).
        //
        // Usage:
        //   world:audio_play_music("audio/theme.ogg")
        //   world:audio_play_music("audio/theme.ogg", { loop=false, volume=-10, fade_in=2.0 })
        methods.add_method(
            "audio_play_music",
            |_, this, (path, opts): (String, Option<mlua::Table>)| {
                use crate::assets::audio::resolve_audio_path;
                use kira::sound::streaming::StreamingSoundData;
                use kira::{Decibels, PlaybackRate, Tween};

                let volume_db = opts
                    .as_ref()
                    .and_then(|t| t.get::<f32>("volume").ok())
                    .unwrap_or(0.0);
                let do_loop = opts
                    .as_ref()
                    .and_then(|t| t.get::<bool>("loop").ok())
                    .unwrap_or(true);
                let fade_in = opts
                    .as_ref()
                    .and_then(|t| t.get::<f32>("fade_in").ok())
                    .unwrap_or(0.0);
                let pitch = opts
                    .as_ref()
                    .and_then(|t| t.get::<f32>("pitch").ok())
                    .unwrap_or(1.0);

                let world = this.world();
                if !world.has_resource::<Audio>() {
                    match Audio::new() {
                        Ok(audio) => {
                            world.insert_resource(audio);
                        }
                        Err(e) => {
                            crate::log_warn!(
                                "[lua] audio_play_music: failed to create AudioManager: {e}"
                            );
                            return Ok(());
                        }
                    }
                }

                let resolved = match resolve_audio_path(&path) {
                    Some(p) => p,
                    None => {
                        crate::log_warn!("[lua] audio_play_music: file not found: {path}");
                        return Ok(());
                    }
                };

                let data = match StreamingSoundData::from_file(&resolved) {
                    Ok(d) => d,
                    Err(e) => {
                        crate::log_warn!("[lua] audio_play_music: failed to open '{path}': {e}");
                        return Ok(());
                    }
                };

                let fade_tween = (fade_in > 0.0).then(|| Tween {
                    duration: std::time::Duration::from_secs_f32(fade_in),
                    ..Default::default()
                });

                let mut data = data
                    .volume(Decibels(volume_db))
                    .playback_rate(PlaybackRate(pitch as f64))
                    .fade_in_tween(fade_tween);
                if do_loop {
                    data = data.loop_region(..);
                }

                if let Ok(audio) = world.get_resource::<Audio>() {
                    if let Err(e) = audio.play_sound(AudioLayer::Music, data) {
                        crate::log_warn!("[lua] audio_play_music: play failed: {e}");
                    }
                }
                Ok(())
            },
        );

        // Usage:
        //   world:audio_play_sfx("audio/click.ogg")
        //   world:audio_play_sfx("audio/footstep.ogg", { layer="FootStep", volume=-3, pitch=1.1 })
        methods.add_method(
            "audio_play_sfx",
            |_, this, (path, opts): (String, Option<mlua::Table>)| {
                use kira::{Decibels, PlaybackRate, Tween};

                let layer_str = opts.as_ref().and_then(|t| t.get::<String>("layer").ok());
                let layer = layer_str
                    .as_deref()
                    .and_then(AudioLayer::from_str)
                    .unwrap_or(AudioLayer::SoundEffect);
                let volume_db = opts
                    .as_ref()
                    .and_then(|t| t.get::<f32>("volume").ok())
                    .unwrap_or(0.0);
                let pitch = opts
                    .as_ref()
                    .and_then(|t| t.get::<f32>("pitch").ok())
                    .unwrap_or(1.0);
                let fade_in = opts
                    .as_ref()
                    .and_then(|t| t.get::<f32>("fade_in").ok())
                    .unwrap_or(0.0);
                let do_loop = opts
                    .as_ref()
                    .and_then(|t| t.get::<bool>("loop").ok())
                    .unwrap_or(false);

                let world = this.world();
                if !world.has_resource::<Audio>() {
                    match Audio::new() {
                        Ok(audio) => {
                            world.insert_resource(audio);
                        }
                        Err(e) => {
                            crate::log_warn!(
                                "[lua] audio_play_sfx: failed to create AudioManager: {e}"
                            );
                            return Ok(());
                        }
                    }
                }

                let sound = match Sound::from_path(&path) {
                    Ok(s) => s,
                    Err(e) => {
                        crate::log_warn!("[lua] audio_play_sfx: {e}");
                        return Ok(());
                    }
                };

                let Some(raw) = sound.data else {
                    return Ok(());
                };

                let fade_tween = (fade_in > 0.0).then(|| Tween {
                    duration: std::time::Duration::from_secs_f32(fade_in),
                    ..Default::default()
                });

                let mut data = raw
                    .volume(Decibels(volume_db))
                    .playback_rate(PlaybackRate(pitch as f64));
                if do_loop {
                    data = data.loop_region(..);
                }
                if let Some(tween) = fade_tween {
                    data = data.fade_in_tween(tween);
                }

                if let Ok(audio) = world.get_resource::<Audio>() {
                    if let Err(e) = audio.play_sound(layer, data) {
                        crate::log_warn!("[lua] audio_play_sfx: play failed: {e}");
                    }
                }
                Ok(())
            },
        );

        // ---- egui UI ----
        // Opens a floating egui window, calling `callback(ui)` with a UiHandle.
        // Only valid during update/fixed_update (between begin_ui and end_ui).
        //
        // Simple form:
        //   world:ui_window("My Window", function(ui) ... end)
        //
        // With options table:
        //   world:ui_window("HUD", {
        //       anchor       = "top_left",  -- "top_left","top_right","bottom_left","bottom_right","center"
        //       offset       = {10, 10},    -- pixels from the anchor point
        //       pos          = {100, 200},  -- fixed screen position (overrides anchor)
        //       size         = {300, 200},  -- default window size
        //       no_title_bar = true,        -- hide the title bar
        //       resizable    = false,       -- prevent resizing
        //   }, function(ui) ... end)
        methods.add_method(
            "ui_window",
            |lua, this, (title, second, third): (String, mlua::Value, Option<mlua::Function>)| {
                let ctx = match this.world().get_resource::<EguiContext>() {
                    Ok(c) => c.0.clone(),
                    Err(_) => return Ok(()),
                };

                let (maybe_opts, func) = match third {
                    Some(f) => {
                        let opts = if let mlua::Value::Table(t) = second {
                            Some(t)
                        } else {
                            None
                        };
                        (opts, f)
                    }
                    None => match second {
                        mlua::Value::Function(f) => (None, f),
                        _ => {
                            return Err(mlua::Error::runtime(
                                "ui_window: 2nd arg must be a function or an options table",
                            ));
                        }
                    },
                };

                let mut window = egui::Window::new(&title);
                if let Some(opts) = &maybe_opts {
                    if let Ok(pos) = opts.get::<mlua::Table>("pos") {
                        window = window.fixed_pos([
                            pos.get::<f32>(1).unwrap_or(0.0),
                            pos.get::<f32>(2).unwrap_or(0.0),
                        ]);
                    }
                    if let Ok(size) = opts.get::<mlua::Table>("size") {
                        window = window.default_size([
                            size.get::<f32>(1).unwrap_or(200.0),
                            size.get::<f32>(2).unwrap_or(100.0),
                        ]);
                    }
                    if opts.get::<bool>("no_title_bar").unwrap_or(false) {
                        window = window.title_bar(false);
                    }
                    if let Ok(v) = opts.get::<bool>("resizable") {
                        window = window.resizable(v);
                    }
                    if let Ok(anchor_str) = opts.get::<String>("anchor") {
                        let align = match anchor_str.as_str() {
                            "top_left" => Some(egui::Align2::LEFT_TOP),
                            "top_right" => Some(egui::Align2::RIGHT_TOP),
                            "bottom_left" => Some(egui::Align2::LEFT_BOTTOM),
                            "bottom_right" => Some(egui::Align2::RIGHT_BOTTOM),
                            "center" => Some(egui::Align2::CENTER_CENTER),
                            _ => None,
                        };
                        if let Some(a) = align {
                            let offset = opts
                                .get::<mlua::Table>("offset")
                                .map(|t| {
                                    egui::Vec2::new(
                                        t.get::<f32>(1).unwrap_or(0.0),
                                        t.get::<f32>(2).unwrap_or(0.0),
                                    )
                                })
                                .unwrap_or(egui::Vec2::ZERO);
                            window = window.anchor(a, offset);
                        }
                    }
                }

                window.show(&ctx, |ui| invoke_with_ui(lua, ui, &func));
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

/// Reads `ctrl`/`shift`/`alt` booleans out of an optional Lua opts table.
fn modifiers_from_opts(opts: Option<&mlua::Table>) -> ModifierKeys {
    let Some(opts) = opts else {
        return ModifierKeys::default();
    };
    ModifierKeys {
        ctrl: opts.get::<bool>("ctrl").unwrap_or(false),
        shift: opts.get::<bool>("shift").unwrap_or(false),
        alt: opts.get::<bool>("alt").unwrap_or(false),
    }
}

/// Reads an optional `context` string out of a Lua opts table.
fn context_from_opts(opts: Option<&mlua::Table>) -> Option<String> {
    opts.and_then(|o| o.get::<String>("context").ok())
}

/// Reads `repeat_delay`/`repeat_rate` out of a Lua opts table; only present if
/// at least one of the two fields was set.
fn repeat_from_opts(opts: Option<&mlua::Table>) -> Option<RepeatConfig> {
    let opts = opts?;
    let delay = opts.get::<f32>("repeat_delay").ok();
    let rate = opts.get::<f32>("repeat_rate").ok();
    if delay.is_none() && rate.is_none() {
        return None;
    }
    let default = RepeatConfig::default();
    Some(RepeatConfig::new(
        delay.unwrap_or(default.initial_delay_secs),
        rate.unwrap_or(default.rate_hz),
    ))
}

/// Builds a `KeyBind` from Lua-provided strings ("Space", "Press", ...) and an
/// optional opts table ({ ctrl, shift, alt, context, repeat_delay, repeat_rate }).
fn lua_to_keybind(key: &str, action: &str, opts: Option<&mlua::Table>) -> mlua::Result<KeyBind> {
    let key = InputManager::string_to_key(key).map_err(mlua::Error::external)?;
    let action = InputManager::string_to_action(action).map_err(mlua::Error::external)?;
    let mut bind = KeyBind::new(key, action);
    bind.modifiers = modifiers_from_opts(opts);
    bind.context = context_from_opts(opts);
    bind.repeat = repeat_from_opts(opts);
    Ok(bind)
}

/// Builds a `MouseBind` from Lua-provided strings ("Left", "Press", ...) and an
/// optional opts table ({ ctrl, shift, alt, context }).
fn lua_to_mousebind(
    button: &str,
    action: &str,
    opts: Option<&mlua::Table>,
) -> mlua::Result<MouseBind> {
    let button = InputManager::string_to_mouse_button(button).map_err(mlua::Error::external)?;
    let action = InputManager::string_to_action(action).map_err(mlua::Error::external)?;
    let mut bind = MouseBind::new(button, action);
    bind.modifiers = modifiers_from_opts(opts);
    bind.context = context_from_opts(opts);
    Ok(bind)
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
