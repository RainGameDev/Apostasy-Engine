use mlua::{AnyUserData, Function, LuaSerdeExt, MultiValue, UserData, UserDataMethods, Value};
use serde_yaml::Value as YamlValue;

use super::component::ScriptComponents;
use super::world_api::EntityHandle;
use crate::ecs::World;
use crate::ecs::cell::EntityId;

/// A runtime filter applied on top of the fetched component set.
enum Filter {
    WithTag(String),
    WithoutTag(String),
    WithComponent(String),
    WithoutComponent(String),
}

impl Filter {
    fn matches(&self, world: &World, id: EntityId) -> bool {
        let has_comp = |n: &str| {
            world
                .get_component::<ScriptComponents>(id)
                .is_some_and(|sc| sc.has(n))
        };
        match self {
            Filter::WithTag(n) => world.has_tag_by_name(id, n),
            Filter::WithoutTag(n) => !world.has_tag_by_name(id, n),
            Filter::WithComponent(n) => has_comp(n),
            Filter::WithoutComponent(n) => !has_comp(n),
        }
    }
}

/// Lua-facing query builder. Holds a raw World pointer valid for the duration of
/// the originating script call. Chained filters mutate it; `for_each` runs it.
pub struct LuaQuery {
    world: *mut World,
    fetch: Vec<String>,
    filters: Vec<Filter>,
}

impl LuaQuery {
    pub fn new(world: *mut World, fetch: Vec<String>) -> Self {
        Self {
            world,
            fetch,
            filters: Vec::new(),
        }
    }
}

impl UserData for LuaQuery {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // Filters use `add_function` (not `add_method`) so we receive the userdata
        // itself and can return it for `:with_tag(...):without(...)` chaining.
        methods.add_function("with_tag", |_, (ud, name): (AnyUserData, String)| {
            ud.borrow_mut::<LuaQuery>()?.filters.push(Filter::WithTag(name));
            Ok(ud)
        });
        methods.add_function("without_tag", |_, (ud, name): (AnyUserData, String)| {
            ud.borrow_mut::<LuaQuery>()?
                .filters
                .push(Filter::WithoutTag(name));
            Ok(ud)
        });
        methods.add_function("with", |_, (ud, name): (AnyUserData, String)| {
            ud.borrow_mut::<LuaQuery>()?
                .filters
                .push(Filter::WithComponent(name));
            Ok(ud)
        });
        methods.add_function("without", |_, (ud, name): (AnyUserData, String)| {
            ud.borrow_mut::<LuaQuery>()?
                .filters
                .push(Filter::WithoutComponent(name));
            Ok(ud)
        });

        methods.add_method("for_each", |lua, this, callback: Function| {
            // Snapshot matching ids first so the callback can freely mutate the world.
            let ids: Vec<EntityId> = {
                let world = unsafe { &*this.world };
                world
                    .get_all_ids()
                    .into_iter()
                    .filter(|&id| {
                        this.fetch.iter().all(|n| {
                            world
                                .get_component::<ScriptComponents>(id)
                                .is_some_and(|sc| sc.has(n))
                        }) && this.filters.iter().all(|f| f.matches(world, id))
                    })
                    .collect()
            };

            for id in ids {
                // Gather component copies in a short borrow that ends before the callback.
                let comps: Vec<YamlValue> = {
                    let world = unsafe { &*this.world };
                    this.fetch
                        .iter()
                        .map(|n| {
                            world
                                .get_component::<ScriptComponents>(id)
                                .and_then(|sc| sc.get(n).cloned())
                                .unwrap_or(YamlValue::Null)
                        })
                        .collect()
                };

                // Build args: (id, comp1, comp2, ...)
                let mut args: Vec<Value> = Vec::with_capacity(comps.len() + 1);
                args.push(Value::UserData(lua.create_userdata(EntityHandle(id))?));
                for c in &comps {
                    args.push(lua.to_value(c)?);
                }
                callback.call::<()>(MultiValue::from_iter(args))?;
            }
            Ok(())
        });
    }
}
