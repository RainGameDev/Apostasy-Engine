use std::marker::PhantomData;

use crate::ecs::{World, components::Component, tags::Tag};
use crate::worldspaces::cell::ObjectId;

// QueryFetch
pub trait QueryFetch {
    type Item<'w>;
    fn collect_base_ids(world: &World) -> Vec<ObjectId>;
    /// # Safety: don't alias `&mut T` and `&T` (or two `&mut T`) for the same T on the same entity.
    unsafe fn fetch<'w>(world: *mut World, id: ObjectId) -> Option<Self::Item<'w>>;
}

impl<'a, T: Component + 'static> QueryFetch for &'a T {
    type Item<'w> = &'w T;
    fn collect_base_ids(world: &World) -> Vec<ObjectId> {
        world.get_entities_with_component::<T>()
    }
    unsafe fn fetch<'w>(world: *mut World, id: ObjectId) -> Option<Self::Item<'w>> {
        unsafe { (*world).get_component::<T>(id).map(|r| &*(r as *const T)) }
    }
}

impl<'a, T: Component + 'static> QueryFetch for &'a mut T {
    type Item<'w> = &'w mut T;
    fn collect_base_ids(world: &World) -> Vec<ObjectId> {
        world.get_entities_with_component::<T>()
    }
    unsafe fn fetch<'w>(world: *mut World, id: ObjectId) -> Option<Self::Item<'w>> {
        unsafe {
            (*world)
                .get_component_mut::<T>(id)
                .map(|r| &mut *(r as *mut T))
        }
    }
}

/// Optional read — entity is never skipped, closure receives `None` if the component is absent.
impl<'a, T: Component + 'static> QueryFetch for Option<&'a T> {
    type Item<'w> = Option<&'w T>;
    fn collect_base_ids(world: &World) -> Vec<ObjectId> {
        world.get_all_ids()
    }
    unsafe fn fetch<'w>(world: *mut World, id: ObjectId) -> Option<Self::Item<'w>> {
        unsafe { Some((*world).get_component::<T>(id).map(|r| &*(r as *const T))) }
    }
}

macro_rules! impl_query_fetch_tuple {
    ($head:ident, $($tail:ident),+) => {
        #[allow(non_snake_case)]
        impl<$head: QueryFetch, $($tail: QueryFetch,)+> QueryFetch for ($head, $($tail,)+) {
            type Item<'w> = ($head::Item<'w>, $($tail::Item<'w>,)+);
            fn collect_base_ids(world: &World) -> Vec<ObjectId> { $head::collect_base_ids(world) }
            unsafe fn fetch<'w>(world: *mut World, id: ObjectId) -> Option<Self::Item<'w>> {
                unsafe { Some(($head::fetch(world, id)?, $($tail::fetch(world, id)?,)+)) }
            }
        }
    };
}

impl_query_fetch_tuple!(Q1, Q2);
impl_query_fetch_tuple!(Q1, Q2, Q3);
impl_query_fetch_tuple!(Q1, Q2, Q3, Q4);
impl_query_fetch_tuple!(Q1, Q2, Q3, Q4, Q5);
impl_query_fetch_tuple!(Q1, Q2, Q3, Q4, Q5, Q6);
impl_query_fetch_tuple!(Q1, Q2, Q3, Q4, Q5, Q6, Q7);
impl_query_fetch_tuple!(Q1, Q2, Q3, Q4, Q5, Q6, Q7, Q8);

// QueryFilter
pub trait QueryFilter {
    fn matches(world: &World, id: ObjectId) -> bool;
}

impl QueryFilter for () {
    fn matches(_: &World, _: ObjectId) -> bool {
        true
    }
}

pub struct With<T>(PhantomData<T>);
pub struct Without<T>(PhantomData<T>);
pub struct WithTag<T>(PhantomData<T>);
pub struct WithoutTag<T>(PhantomData<T>);

impl<T: Component + 'static> QueryFilter for With<T> {
    fn matches(world: &World, id: ObjectId) -> bool {
        world.has_component::<T>(id)
    }
}

impl<T: Component + 'static> QueryFilter for Without<T> {
    fn matches(world: &World, id: ObjectId) -> bool {
        !world.has_component::<T>(id)
    }
}

impl<T: Tag + 'static> QueryFilter for WithTag<T> {
    fn matches(world: &World, id: ObjectId) -> bool {
        world.has_tag::<T>(id)
    }
}

impl<T: Tag + 'static> QueryFilter for WithoutTag<T> {
    fn matches(world: &World, id: ObjectId) -> bool {
        !world.has_tag::<T>(id)
    }
}

macro_rules! impl_query_filter_tuple {
    ($head:ident, $($tail:ident),+) => {
        #[allow(non_snake_case)]
        impl<$head: QueryFilter, $($tail: QueryFilter,)+> QueryFilter for ($head, $($tail,)+) {
            fn matches(world: &World, id: ObjectId) -> bool {
                $head::matches(world, id) $(&& $tail::matches(world, id))+
            }
        }
    };
}

impl_query_filter_tuple!(F1, F2);
impl_query_filter_tuple!(F1, F2, F3);
impl_query_filter_tuple!(F1, F2, F3, F4);

// Query<F, Fil> — injectable, holds a raw pointer so it can coexist
// with &mut World in the same function without a borrow conflict.
pub struct Query<F: QueryFetch, Fil: QueryFilter = ()> {
    world: *mut World,
    ids: Vec<ObjectId>,
    _marker: PhantomData<(F, Fil)>,
}

impl<F: QueryFetch, Fil: QueryFilter> Query<F, Fil> {
    /// Build the query, snapshotting matching entity IDs immediately.
    ///
    /// # Safety
    /// `world` must remain valid (and not have entities added/removed) for the
    /// lifetime of this `Query`.
    pub unsafe fn new(world: *mut World) -> Self {
        let ids = {
            let w = unsafe { &*world };
            F::collect_base_ids(w)
                .into_iter()
                .filter(|&id| Fil::matches(w, id))
                .collect()
        };
        Self {
            world,
            ids,
            _marker: PhantomData,
        }
    }

    pub fn for_each(&self, mut f: impl FnMut(ObjectId, F::Item<'_>)) {
        for &id in &self.ids {
            if let Some(item) = unsafe { F::fetch(self.world, id) } {
                f(id, item);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
    pub fn len(&self) -> usize {
        self.ids.len()
    }
}

// QueryBuilder — manual builder with runtime filter chaining
pub struct QueryBuilder<'w, F: QueryFetch> {
    world: &'w mut World,
    filters: Vec<Box<dyn Fn(&World, ObjectId) -> bool>>,
    _fetch: PhantomData<F>,
}

impl<'w, F: QueryFetch> QueryBuilder<'w, F> {
    pub(crate) fn new(world: &'w mut World) -> Self {
        Self {
            world,
            filters: Vec::new(),
            _fetch: PhantomData,
        }
    }

    pub fn with<C: Component + 'static>(mut self) -> Self {
        self.filters
            .push(Box::new(|w, id| w.has_component::<C>(id)));
        self
    }

    pub fn without<C: Component + 'static>(mut self) -> Self {
        self.filters
            .push(Box::new(|w, id| !w.has_component::<C>(id)));
        self
    }

    pub fn with_tag<T: Tag + 'static>(mut self) -> Self {
        self.filters.push(Box::new(|w, id| w.has_tag::<T>(id)));
        self
    }

    pub fn without_tag<T: Tag + 'static>(mut self) -> Self {
        self.filters.push(Box::new(|w, id| !w.has_tag::<T>(id)));
        self
    }

    pub fn for_each(self, mut f: impl FnMut(ObjectId, F::Item<'_>)) {
        let world_ptr: *mut World = self.world;
        let ids: Vec<ObjectId> = {
            let world_ref: &World = unsafe { &*world_ptr };
            F::collect_base_ids(world_ref)
                .into_iter()
                .filter(|&id| self.filters.iter().all(|flt| flt(world_ref, id)))
                .collect()
        };
        for id in ids {
            if let Some(item) = unsafe { F::fetch(world_ptr, id) } {
                f(id, item);
            }
        }
    }
}
