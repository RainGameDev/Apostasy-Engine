use std::{
    any::{Any, type_name},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};

use anyhow::Result;
use mlua::{Lua, Table};

use crate::ecs::resources::Resource;

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
}

impl LuaRuntime {
    pub fn new() -> Result<Self> {
        Ok(Self {
            lua: Lua::new(),
            scripts: Arc::new(Mutex::new(Vec::new())),
        })
    }

    fn make_env(&self) -> Result<Table> {
        let env = self.lua.create_table().unwrap();
        let meta = self.lua.create_table().unwrap();
        meta.set("__index", self.lua.globals()).unwrap();
        env.set_metatable(Some(meta));
        Ok(env)
    }

    /// Loads a `.lua` file, runs its top-level code in a fresh sandbox, and stores it.
    pub fn load_script(&self, path: &Path) -> Result<()> {
        let source = std::fs::read_to_string(path)?;
        let env = self.make_env()?;
        self.lua
            .load(&source)
            .set_name(path.to_string_lossy().to_string())
            .set_environment(env.clone())
            .exec()
            .unwrap();
        let last_modified = path.metadata().ok().and_then(|m| m.modified().ok());
        self.scripts.lock().unwrap().push(LoadedScript {
            path: path.to_path_buf(),
            env,
            last_modified,
        });
        tracing::info!("[lua] loaded {:?}", path);
        Ok(())
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
            .collect::<Result<Vec<_>, _>>()
        else {
            continue;
        };
        for entry in entries {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "lua") {
                paths.push(path.to_path_buf());
            }
        }
    }
    paths
}
