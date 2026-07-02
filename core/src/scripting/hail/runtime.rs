use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};

use apostasy_macros::Resource;

use crate::{ecs::World, scripting::hail::resolver::FileImportResolver};

/// Container of the World used for hail.
#[derive(Clone, Copy)]
pub struct WorldHandle {
    pub world: *mut World,
}

// Required by hail's ProgramValuem default impl fits a fixed-size handle.
impl hail::memory_usage::MemoryUsage for WorldHandle {}

impl WorldHandle {
    /// SAFETY: valid only while inside the run_* call that created this handle.
    #[allow(clippy::mut_from_ref)]
    pub(crate) fn world(&self) -> &mut World {
        unsafe { &mut *self.world }
    }
}

pub struct LoadedHailScript {
    pub path: PathBuf,
    pub program: Arc<hail::Program>,
    pub last_modified: Option<SystemTime>,
}

#[derive(Resource, Clone)]
pub struct HailRuntime {
    module: Arc<hail::Module>,
    resolver: Arc<FileImportResolver>,
    scripts: Arc<Mutex<Vec<LoadedHailScript>>>,
}

impl HailRuntime {
    pub fn new() -> Self {
        Self {
            module: Arc::new(build_engine_module().expect("hail engine module should build")),
            resolver: Arc::new(FileImportResolver::new()),
            scripts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Compiles one .hail source file.
    /// Returns None on any scan/parse/construct error.
    fn compile(&self, path: &Path) -> Option<Arc<hail::Program>> {
        let path_str = path.to_string_lossy();
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                crate::log_error!("[hail] read {:?} failed: {e}", path);
                return None;
            }
        };

        let mut scanner = hail::Scanner::new(source.clone());
        scanner.scan();
        // Owned: the program keeps the tokens for runtime error spans.
        let Some(tokens) = scanner.get().map(<[hail::Token]>::to_vec) else {
            return None;
        };
        if !scanner.errors().is_empty() {
            crate::log_error!(
                "{}",
                hail::format_diagnostics(&source, Some(&path_str), scanner.errors(), &tokens)
            );
            return None;
        }

        let mut parser = hail::Parser::new(&tokens);
        parser.parse();
        if !parser.errors().is_empty() {
            crate::log_error!(
                "{}",
                hail::format_diagnostics(&source, Some(&path_str), parser.errors(), &tokens)
            );
            return None;
        }
        let stmts = parser.get()?;

        let constructor = hail::Constructor::new(
            self.module.clone(),
            self.resolver.clone(), // coerces to Arc<dyn ImportResolver>
            Some(path_str.to_string()),
        );
        let construct = match constructor.generate(stmts) {
            Ok(c) => c,
            Err(err) => {
                crate::log_error!(
                    "{}",
                    hail::format_diagnostic(&source, Some(&path_str), &err, &tokens)
                );
                return None;
            }
        };

        let instructor = hail::Instructor::new(self.module.clone(), self.resolver.clone(), None);
        Some(Arc::new(instructor.generate(construct, source, tokens)))
    }

    pub fn load_script(&self, path: &Path) -> Option<Arc<hail::Program>> {
        let program = self.compile(path)?;
        let last_modified = path.metadata().ok().and_then(|m| m.modified().ok());
        self.scripts.lock().unwrap().push(LoadedHailScript {
            path: path.to_path_buf(),
            program: program.clone(),
            last_modified,
        });
        crate::log!("[hail] loaded {:?}", path);
        Some(program)
    }

    pub fn run_event(&self, world: &mut World, event: &str) {
        let handle = WorldHandle { world };
        let scripts = self.scripts.lock().unwrap();
        for s in scripts.iter() {
            let result = hail::Executor::default().run_function_with_any_return(
                &s.program,
                event,
                vec![hail::ValueType::of::<WorldHandle>()],
                vec![Box::new(handle)],
                None,
            );
            match result {
                Ok(_) => {}
                // ExecutorError is now a struct; the variants live on `.kind`.
                Err(e) => match e.kind {
                    // Script doesn't subscribe to this event.
                    hail::ExecutorErrorKind::MissingFunction(_) => {}
                    _ => {
                        crate::log_error!("[hail] {event} error in {:?}: {e:?}", s.path);
                    }
                },
            }
        }
    }
}

/// Gets all hail files under `res/scripts/`.
pub fn discover_hail_scripts() -> Vec<PathBuf> {
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
            if path.extension().map_or(false, |e| e == "hail") {
                paths.push(path.to_path_buf());
            }
        }
    }
    paths
}

pub fn build_engine_module() -> Result<hail::Module, hail::ModuleError> {
    use super::{components, logging, query, resources, world};

    let mut module = hail_std::std()?;

    // Types first: functions below reference them by ValueType.
    module.register_type_named::<WorldHandle>("World")?;
    module.register_type_named::<world::EntityHandle>("Entity")?;
    module.register_type_named::<components::Vec2Value>("Vec2")?;
    module.register_type_named::<components::Vec3Value>("Vec3")?;
    module.register_type_named::<components::HailTransform>("Transform")?;
    module.register_type_named::<components::HailVelocity>("Velocity")?;
    module.register_type_named::<query::QueryBuilder>("Query")?;
    module.register_type_named::<query::QueryRow>("Row")?;

    logging::hail_register_log(&mut module)?;

    // Entities and tags.
    world::hail_register_spawn(&mut module)?;
    world::hail_register_despawn(&mut module)?;
    world::hail_register_set_name(&mut module)?;
    world::hail_register_entity_equal(&mut module)?;
    world::hail_register_entity_not_equal(&mut module)?;
    world::hail_register_add_tag(&mut module)?;
    world::hail_register_remove_tag(&mut module)?;
    world::hail_register_has_tag(&mut module)?;
    world::hail_register_entities_with_tag(&mut module)?;

    // Engine resources.
    resources::hail_register_delta_time(&mut module)?;
    resources::hail_register_time(&mut module)?;
    resources::hail_register_window_size(&mut module)?;
    resources::hail_register_quit(&mut module)?;

    // Input.
    world::hail_register_is_keybind_active(&mut module)?;
    world::hail_register_input_vector_2d(&mut module)?;
    world::hail_register_mouse_delta(&mut module)?;

    // Queries.
    query::hail_register_query(&mut module)?;
    query::hail_register_with(&mut module)?;
    query::hail_register_without(&mut module)?;
    query::hail_register_with_tag(&mut module)?;
    query::hail_register_without_tag(&mut module)?;
    query::hail_register_run_query(&mut module)?;
    query::hail_register_fetch(&mut module)?;
    query::hail_register_row_entity(&mut module)?;
    query::register_row_properties(&mut module)?;

    // Vec2.
    components::hail_register_vec2(&mut module)?;
    components::hail_register_vec2_x(&mut module)?;
    components::hail_register_vec2_y(&mut module)?;
    components::hail_register_vec2_set_x(&mut module)?;
    components::hail_register_vec2_set_y(&mut module)?;

    // Vec3.
    components::hail_register_vec3(&mut module)?;
    components::hail_register_vec3_x(&mut module)?;
    components::hail_register_vec3_y(&mut module)?;
    components::hail_register_vec3_z(&mut module)?;
    components::hail_register_vec3_set_x(&mut module)?;
    components::hail_register_vec3_set_y(&mut module)?;
    components::hail_register_vec3_set_z(&mut module)?;

    // Transform snapshot proxy.
    components::hail_register_transform_position(&mut module)?;
    components::hail_register_transform_euler_angles(&mut module)?;
    components::hail_register_transform_scale(&mut module)?;
    components::hail_register_transform_set_position(&mut module)?;
    components::hail_register_transform_set_euler_angles(&mut module)?;
    components::hail_register_transform_set_scale(&mut module)?;
    components::hail_register_transform_rotate(&mut module)?;
    components::hail_register_get_transform(&mut module)?;
    components::hail_register_set_transform(&mut module)?;

    // Velocity snapshot proxy.
    components::hail_register_velocity_linear(&mut module)?;
    components::hail_register_velocity_set_linear(&mut module)?;
    components::hail_register_velocity_is_grounded(&mut module)?;
    components::hail_register_get_velocity(&mut module)?;
    components::hail_register_set_velocity(&mut module)?;

    // Generic by-name component access and queries.
    components::hail_register_add_component(&mut module)?;
    components::hail_register_remove_component(&mut module)?;
    components::hail_register_has_component(&mut module)?;
    components::hail_register_entities_with(&mut module)?;

    Ok(module)
}
