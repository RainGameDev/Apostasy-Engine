use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::Result;
use apostasy_macros::Resource;
use parking_lot::Mutex;
use wasmtime::{Config, Engine, Linker, Module, Store};

use crate::objects::world::World;

use super::host::{HostState, register_host_fns};

struct ScriptModule {
    path: PathBuf,
    module: Module,
    last_modified: Option<SystemTime>,
}

struct ScriptRuntimeInner {
    engine: Engine,
    linker: Linker<HostState>,
    modules: Vec<ScriptModule>,
}

/// Holds the wasmtime engine, host-function linker, and all loaded script modules.
#[derive(Clone, Resource)]
pub struct ScriptRuntime {
    inner: Arc<Mutex<ScriptRuntimeInner>>,
}

impl ScriptRuntime {
    /// Creates a new runtime and registers all engine host functions.
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config)?;
        let mut linker: Linker<HostState> = Linker::new(&engine);
        register_host_fns(&mut linker)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(ScriptRuntimeInner {
                engine,
                linker,
                modules: Vec::new(),
            })),
        })
    }

    /// Loads a .wasm file from `path` and registers it.
    pub fn load_module(&self, path: &Path) -> Result<()> {
        let mut inner = self.inner.lock();
        let module = Module::from_file(&inner.engine, path)?;
        let last_modified = path.metadata().ok().and_then(|m| m.modified().ok());
        inner.modules.push(ScriptModule {
            path: path.to_path_buf(),
            module,
            last_modified,
        });
        tracing::info!("[scripting] loaded {:?}", path);
        Ok(())
    }

    /// Calls `on_start` in every loaded module.
    pub fn run_start(&self, world: *mut World) {
        let inner = self.inner.lock();
        for sm in &inner.modules {
            call_export(&inner.linker, &inner.engine, &sm.module, "on_start", world);
        }
    }

    /// Reloads any modules whose .wasm file has changed, then calls `on_update` in each.
    pub fn run_update(&self, world: *mut World) {
        let mut inner = self.inner.lock();

        for i in 0..inner.modules.len() {
            let current = inner.modules[i].path.metadata().ok().and_then(|m| m.modified().ok());
            if current != inner.modules[i].last_modified {
                let path = inner.modules[i].path.clone();
                match Module::from_file(&inner.engine, &path) {
                    Ok(new_module) => {
                        inner.modules[i].module = new_module;
                        inner.modules[i].last_modified = current;
                        tracing::info!("[scripting] hot-reloaded {:?}", path);
                    }
                    Err(e) => {
                        tracing::error!("[scripting] reload failed for {:?}: {e}", path);
                    }
                }
            }
        }

        for i in 0..inner.modules.len() {
            call_export(&inner.linker, &inner.engine, &inner.modules[i].module, "on_update", world);
        }
    }
}

/// Calls a named export in `module` if it exists. Traps are caught and logged.
fn call_export(
    linker: &Linker<HostState>,
    engine: &Engine,
    module: &Module,
    export: &str,
    world: *mut World,
) {
    let host_state = HostState::new(world);
    let mut store = Store::new(engine, host_state);

    // Limit script execution to avoid runaway loops hanging the frame.
    let _ = store.set_fuel(1_000_000);

    let instance = match linker.instantiate(&mut store, module) {
        Ok(i) => i,
        Err(e) => {
            tracing::error!("[scripting] instantiate failed: {e}");
            return;
        }
    };

    let func = match instance.get_typed_func::<(), ()>(&mut store, export) {
        Ok(f) => f,
        Err(_) => return, // export doesn't exist in this module; that's fine
    };

    if let Err(trap) = func.call(&mut store, ()) {
        tracing::error!("[scripting] trap in '{export}': {trap}");
    }
}

/// Returns paths of all .wasm files under `res/scripts/` using the engine's dual-path strategy.
pub fn discover_script_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let candidates = [
        PathBuf::from("res/scripts"),
        PathBuf::from(format!("{}/res/scripts", env!("CARGO_MANIFEST_DIR"))),
    ];
    for dir in &candidates {
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = walkdir::WalkDir::new(dir).into_iter().collect::<Result<Vec<_>, _>>() else {
            continue;
        };
        for entry in entries {
            let path = entry.path();
            if path.components().any(|c| c.as_os_str() == "target") {
                continue;
            }
            if path.extension().map_or(false, |e| e == "wasm") {
                paths.push(path.to_path_buf());
            }
        }
    }
    paths
}
