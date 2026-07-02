use hail::memory_usage::MemoryUsage;
use hail::{FunctionInfo, FunctionKind, Module, ModuleError, ValueType};
use hail_macro::hail;

use crate::ecs::World;
use crate::ecs::components::get_component_registration;
use crate::ecs::components::transform::Transform;
use crate::physics::velocity::Velocity;
use crate::scripting::hail::components::{HailTransform, HailVelocity};
use crate::scripting::hail::runtime::WorldHandle;
use crate::scripting::hail::world::EntityHandle;

/// Composable entity query, registered as `Query`. Builder methods return
/// modified copies (hail chains break on in-place mutation), `run()` evaluates.
#[derive(Clone)]
pub struct QueryBuilder {
    world: *mut World,
    with: Vec<String>,
    without: Vec<String>,
    with_tags: Vec<String>,
    without_tags: Vec<String>,
}

impl MemoryUsage for QueryBuilder {
    fn memory_usage(&self) -> usize {
        std::mem::size_of::<u64>()
            + self
                .with
                .iter()
                .chain(&self.without)
                .chain(&self.with_tags)
                .chain(&self.without_tags)
                .map(MemoryUsage::memory_usage)
                .sum::<usize>()
    }
}

/// `world.query` — the root of a query chain.
#[hail(property, "query")]
pub fn query(world: &WorldHandle) -> QueryBuilder {
    QueryBuilder {
        world: world.world,
        with: Vec::new(),
        without: Vec::new(),
        with_tags: Vec::new(),
        without_tags: Vec::new(),
    }
}

#[hail(method, "with")]
pub fn with(query: &QueryBuilder, component: String) -> QueryBuilder {
    let mut q = query.clone();
    q.with.push(component);
    q
}

#[hail(method, "without")]
pub fn without(query: &QueryBuilder, component: String) -> QueryBuilder {
    let mut q = query.clone();
    q.without.push(component);
    q
}

#[hail(method, "with_tag")]
pub fn with_tag(query: &QueryBuilder, tag: String) -> QueryBuilder {
    let mut q = query.clone();
    q.with_tags.push(tag);
    q
}

#[hail(method, "without_tag")]
pub fn without_tag(query: &QueryBuilder, tag: String) -> QueryBuilder {
    let mut q = query.clone();
    q.without_tags.push(tag);
    q
}

/// One query result with its component snapshots, registered as `Row`.
/// Missing snapshots surface as a runtime error on property access.
#[derive(Clone, Copy)]
pub struct QueryRow {
    pub entity: EntityHandle,
    pub transform: Option<HailTransform>,
    pub velocity: Option<HailVelocity>,
}

impl MemoryUsage for QueryRow {}

#[hail(property, "entity")]
pub fn row_entity(row: &QueryRow) -> EntityHandle {
    row.entity
}

// row.transform / row.velocity are registered by hand instead of via the
// macro so a missing component raises a spanned script error, not a panic.
pub fn register_row_properties(module: &mut Module) -> Result<(), ModuleError> {
    module.register_method(
        ValueType::of::<QueryRow>(),
        FunctionInfo {
            name: "transform".to_string(),
            param_types: vec![ValueType::of::<QueryRow>()],
            param_names: vec!["row".to_string()],
            doc_comments: vec!["The entity's Transform snapshot.".to_string()],
            return_type: Some(ValueType::of::<HailTransform>()),
            kind: FunctionKind::Property(|_executor, callee| {
                let row = callee
                    .as_any()
                    .downcast_ref::<QueryRow>()
                    .expect("callee should be a QueryRow");
                match row.transform {
                    Some(t) => Ok(Some(Box::new(t))),
                    None => Err(hail::ExecutorErrorKind::Fail(
                        "row has no Transform (the entity lost it mid-loop?)".to_string(),
                    )),
                }
            }),
        },
    )?;

    module.register_method(
        ValueType::of::<QueryRow>(),
        FunctionInfo {
            name: "velocity".to_string(),
            param_types: vec![ValueType::of::<QueryRow>()],
            param_names: vec!["row".to_string()],
            doc_comments: vec!["The entity's Velocity snapshot.".to_string()],
            return_type: Some(ValueType::of::<HailVelocity>()),
            kind: FunctionKind::Property(|_executor, callee| {
                let row = callee
                    .as_any()
                    .downcast_ref::<QueryRow>()
                    .expect("callee should be a QueryRow");
                match row.velocity {
                    Some(v) => Ok(Some(Box::new(v))),
                    None => Err(hail::ExecutorErrorKind::Fail(
                        "row has no Velocity (the entity lost it mid-loop?)".to_string(),
                    )),
                }
            }),
        },
    )?;

    Ok(())
}

/// Like `run`, but each result carries snapshots of the components the
/// entity has — the closest hail gets to Lua's `for_each(id, t, v)`.
#[hail(method, "fetch")]
pub fn fetch(query: &QueryBuilder) -> Vec<QueryRow> {
    // SAFETY: see run_query.
    let world = unsafe { &mut *query.world };
    run_query(query)
        .into_iter()
        .map(|entity| QueryRow {
            entity,
            transform: world
                .get_component::<Transform>(entity.0)
                .map(HailTransform::from),
            velocity: world
                .get_component::<Velocity>(entity.0)
                .map(HailVelocity::from),
        })
        .collect()
}

// Named run_query on the Rust side: the macro's generated code globs in
// `hail::*`, and a fn literally named `run` collides with `hail::run`.
#[hail(method, "run")]
pub fn run_query(query: &QueryBuilder) -> Vec<EntityHandle> {
    // SAFETY: same lifetime argument as WorldHandle — the builder only exists
    // inside the run_* call that produced its world pointer.
    let world = unsafe { &mut *query.world };

    // Resolve component names up front; an unknown `with` can never match.
    let mut with_regs = Vec::with_capacity(query.with.len());
    for name in &query.with {
        match get_component_registration(name) {
            Some(reg) => with_regs.push(reg),
            None => {
                crate::log_error!("[hail] query: unknown component '{name}'");
                return Vec::new();
            }
        }
    }
    let mut without_regs = Vec::with_capacity(query.without.len());
    for name in &query.without {
        match get_component_registration(name) {
            Some(reg) => without_regs.push(reg),
            None => {
                crate::log_error!("[hail] query: unknown component '{name}' in without");
            }
        }
    }

    let mut out = Vec::new();
    'entities: for id in world.get_all_ids() {
        for reg in &with_regs {
            if !(reg.contains)(world, id) {
                continue 'entities;
            }
        }
        for reg in &without_regs {
            if (reg.contains)(world, id) {
                continue 'entities;
            }
        }
        for tag in &query.with_tags {
            if !world.has_tag_by_name(id, tag) {
                continue 'entities;
            }
        }
        for tag in &query.without_tags {
            if world.has_tag_by_name(id, tag) {
                continue 'entities;
            }
        }
        out.push(EntityHandle(id));
    }
    out
}
