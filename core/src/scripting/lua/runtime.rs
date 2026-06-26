use std::{
    any::{Any, type_name},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};

use mlua::{Function, Lua, LuaSerdeExt, Result, Table};
use serde_yaml::Value;

use crate::{
    ecs::{World, resources::Resource},
    scripting::lua::{
        component::LuaComponentRegistry, resource::LuaResources, world_api::WorldHandle,
    },
};

/// One loaded script, it has a source, it's sandbox environment, and the file time for hot-reloading.
pub(crate) struct LoadedScript {
    pub(crate) path: PathBuf,
    pub(crate) env: Table,
    pub(crate) last_modified: Option<SystemTime>,
}

/// Owns the Lua state end every loaded script.
/// Manually impliments `Resource` because `Lua` is not `Send`/`Sync`.
#[derive(Clone)]
pub struct LuaRuntime {
    pub(crate) lua: Lua,
    pub(crate) scripts: Arc<Mutex<Vec<LoadedScript>>>,
    pub(crate) pending: Arc<Mutex<Vec<(String, Value)>>>,
    /// Global resources declared by `register_resource(...)`, drained into
    /// the `LuaResources` resource via [`apply_registrations`].
    pub(crate) pending_resources: Arc<Mutex<Vec<(String, Value)>>>,
}

impl LuaRuntime {
    pub fn new() -> Result<Self> {
        let lua = Lua::new();

        // Math ergonomics (vec2/vec3/vec4/quat, math helpers) into globals, so
        // every sandboxed script env inherits them through its `__index`.
        lua.load(include_str!("prelude.lua"))
            .set_name("prelude")
            .exec()?;

        let pending: Arc<Mutex<Vec<(String, Value)>>> = Arc::new(Mutex::new(Vec::new()));

        let buffer = pending.clone();
        let register =
            lua.create_function(move |lua, (name, defaults): (String, mlua::Value)| {
                let value: Value = lua.from_value(defaults)?;
                buffer.lock().unwrap().push((name, value));
                Ok(())
            })?;
        lua.globals().set("register_component", register)?;

        // Global `register_resource(name, defaults)` — seeds global script state.
        let pending_resources: Arc<Mutex<Vec<(String, Value)>>> = Arc::new(Mutex::new(Vec::new()));
        let res_buffer = pending_resources.clone();
        let register_res =
            lua.create_function(move |lua, (name, defaults): (String, mlua::Value)| {
                let value: Value = lua.from_value(defaults)?;
                res_buffer.lock().unwrap().push((name, value));
                Ok(())
            })?;
        lua.globals().set("register_resource", register_res)?;

        Ok(Self {
            lua,
            scripts: Arc::new(Mutex::new(Vec::new())),
            pending,
            pending_resources,
        })
    }

    /// Drains buffered `register_component`/`register_resource` declarations into
    /// the world's `LuaComponentRegistry` and `LuaResources`.
    pub fn apply_registrations(&self, world: &mut World) {
        // Component schemas: always (re)register.
        {
            let mut pending = self.pending.lock().unwrap();
            if !pending.is_empty()
                && let Ok(reg) = world.get_resource_mut::<LuaComponentRegistry>()
            {
                for (name, value) in pending.drain(..) {
                    reg.register(&name, value);
                }
            }
        }
        {
            let mut pending = self.pending_resources.lock().unwrap();
            if !pending.is_empty()
                && let Ok(res) = world.get_resource_mut::<LuaResources>()
            {
                for (name, value) in pending.drain(..) {
                    if !res.has(&name) {
                        res.set(&name, value);
                    }
                }
            }
        }
    }

    fn make_env(&self) -> Result<Table> {
        let env = self.lua.create_table()?;
        let meta = self.lua.create_table()?;
        meta.set("__index", self.lua.globals())?;
        env.set_metatable(Some(meta));
        Ok(env)
    }

    /// Reads, sandboxes, and executes a script's top-level code, returning its env.
    fn exec_script(&self, path: &Path) -> Result<Table> {
        let source = std::fs::read_to_string(path)?;
        let env = self.make_env()?;
        self.lua
            .load(&source)
            .set_name(path.to_string_lossy().to_string())
            .set_environment(env.clone())
            .exec()?;
        Ok(env)
    }

    /// Loads a `.lua` file, runs its top-level code in a fresh sandbox, and stores it.
    pub fn load_script(&self, path: &Path) -> Result<()> {
        let env = self.exec_script(path)?;
        let last_modified = path.metadata().ok().and_then(|m| m.modified().ok());
        self.scripts.lock().unwrap().push(LoadedScript {
            path: path.to_path_buf(),
            env,
            last_modified,
        });
        crate::log!("[lua] loaded {:?}", path);
        Ok(())
    }

    /// Re-executes any script whose file changed on disk since it was last loaded.
    /// A fresh sandbox replaces the old one, so redefined `start`/`update` take over.
    pub fn hot_reload(&self) {
        let mut scripts = self.scripts.lock().unwrap();
        for s in scripts.iter_mut() {
            let current = s.path.metadata().ok().and_then(|m| m.modified().ok());
            if current == s.last_modified {
                continue;
            }
            match self.exec_script(&s.path) {
                Ok(env) => {
                    s.env = env;
                    s.last_modified = current;
                    crate::log!("[lua] reloaded {:?}", s.path);
                }
                Err(e) => {
                    crate::log_error!("[lua] reload failed for {:?}: {e}", s.path);
                }
            }
        }
    }

    /// Calls a top-level function named `event` (e.g. "start"/"update") in every
    /// loaded script, passing a scoped World handle. Errors are logged, not fatal.
    pub fn run_event(&self, world: &mut World, event: &str) {
        let world_ptr: *mut World = world;
        let scripts = self.scripts.lock().unwrap();

        let result = self.lua.scope(|scope| {
            let world_ud = scope.create_userdata(WorldHandle { world: world_ptr })?;
            for s in scripts.iter() {
                if let Ok(func) = s.env.get::<Function>(event) {
                    if let Err(e) = func.call::<()>(&world_ud) {
                        crate::log_error!("[lua] {event} error in {:?}: {e}", s.path);
                    }
                }
            }
            Ok(())
        });

        if let Err(e) = result {
            crate::log_error!("[lua] scope failed for {event}: {e}");
        }
    }

    /// Like [`run_event`], but also passes the fixed timestep as a second argument,
    /// for `fixed_update(world, delta)`.
    pub fn run_fixed_event(&self, world: &mut World, event: &str, delta: f32) {
        let world_ptr: *mut World = world;
        let scripts = self.scripts.lock().unwrap();

        let result = self.lua.scope(|scope| {
            let world_ud = scope.create_userdata(WorldHandle { world: world_ptr })?;
            for s in scripts.iter() {
                if let Ok(func) = s.env.get::<Function>(event) {
                    if let Err(e) = func.call::<()>((&world_ud, delta)) {
                        crate::log_error!("[lua] {event} error in {:?}: {e}", s.path);
                    }
                }
            }
            Ok(())
        });

        if let Err(e) = result {
            crate::log_error!("[lua] scope failed for {event}: {e}");
        }
    }
}

impl Resource for LuaRuntime {
    fn name() -> &'static str {
        std::any::type_name::<Self>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn type_name(&self) -> &'static str {
        type_name::<Self>()
    }
}

/// Gets all lua files under `res/scripts/`.
pub fn discover_lua_scripts() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let candidates = [
        PathBuf::from("res/scripts"),
        PathBuf::from(format!("{}/res/scripts", env!("CARGO_MANIFEST_DIR"))),
    ];
    for dir in &candidates {
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = walkdir::WalkDir::new(dir)
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()
        else {
            continue;
        };
        for entry in entries {
            let path = entry.path();
            // Skip `types/` — it holds editor-only API definition stubs, not scripts.
            if path.components().any(|c| c.as_os_str() == "types") {
                continue;
            }
            if path.extension().map_or(false, |e| e == "lua") {
                paths.push(path.to_path_buf());
            }
        }
    }
    paths
}
