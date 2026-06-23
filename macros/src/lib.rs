use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{DeriveInput, ItemFn, LitInt, LitStr, Token, parse_macro_input, parse_quote};

#[proc_macro_derive(Component, attributes(component_deserialize))]
pub fn component_derive(input: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(input as DeriveInput);
    ast.generics
        .make_where_clause()
        .predicates
        .push(parse_quote! { Self: Clone + Send + Sync + 'static });

    let struct_name = &ast.ident;
    let struct_name_str = struct_name.to_string();
    let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();

    let output = quote! {
        impl #impl_generics apostasy_core::ecs::components::Component for #struct_name #type_generics
            #where_clause
        {
            fn name() -> &'static str where Self: Sized {
                std::any::type_name::<#struct_name>()
            }
            fn as_any(&self) -> &dyn std::any::Any { self }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
            fn type_name(&self) -> &'static str {
                std::any::type_name::<Self>()
            }
        }
        inventory::submit! {
            apostasy_core::ecs::components::ComponentRegistration {
                type_name: #struct_name_str,
                create: || Box::new(#struct_name::default()),
                deserialize: |component, value| {
                    if let Some(c) = component.as_any_mut().downcast_mut::<#struct_name>() {
                        c.deserialize(value)
                    } else {
                        Ok(())
                    }
                },
                add_to_world: |world, id, component| {
                    if let Some(c) = component.as_any().downcast_ref::<#struct_name>() {
                        world.add_component(id, c.clone());
                    }
                },
            }
        }
        inventory::submit! {
            apostasy_core::ecs::components::InspectEntry {
                type_id: || std::any::TypeId::of::<#struct_name>(),
                inspect_fn: |any, ui: &mut apostasy_core::egui::Ui| {
                    if let Some(c) = any.downcast_mut::<#struct_name>() {
                        apostasy_core::ecs::components::Inspect::inspect(c, ui);
                    }
                },
            }
        }
    };
    output.into()
}

#[proc_macro_derive(Inspect)]
pub fn inspect_derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let struct_name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = ast.generics.split_for_impl();
    let output = quote! {
        impl #impl_generics apostasy_core::ecs::components::Inspect for #struct_name #type_generics #where_clause {}
    };
    output.into()
}

#[proc_macro_derive(Resource)]
pub fn resource_derive(input: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(input as DeriveInput);
    ast.generics
        .make_where_clause()
        .predicates
        .push(parse_quote! { Self: Clone + Send + Sync + 'static });

    let struct_name = &ast.ident;

    let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();

    let output = quote! {
        impl #impl_generics apostasy_core::ecs::resources::Resource for #struct_name #type_generics
            #where_clause

        {
            fn name() -> &'static str where Self: Sized {
                std::any::type_name::<#struct_name>()
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
            fn type_name(&self) -> &'static str {
                std::any::type_name::<Self>()
            }
        }
    };
    output.into()
}

#[proc_macro_derive(Tag)]
pub fn tag_derive(input: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(input as DeriveInput);
    ast.generics
        .make_where_clause()
        .predicates
        .push(parse_quote! { Self: Clone + Send + Sync + 'static });

    let struct_name = &ast.ident;
    let struct_name_str = struct_name.to_string();

    let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();

    let output = quote! {
        impl #impl_generics apostasy_core::ecs::tags::Tag for #struct_name #type_generics
            #where_clause
        {
            fn name() -> &'static str where Self: Sized {
                std::any::type_name::<#struct_name>()
            }
            fn as_any(&self) -> &dyn std::any::Any { self }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
            fn type_name(&self) -> &'static str {
                std::any::type_name::<Self>()
            }
            fn type_name_static() -> &'static str {
                std::any::type_name::<Self>()
            }
        }
        inventory::submit! {
            apostasy_core::ecs::tags::TagRegistration {
                type_name: #struct_name_str,
                singleton: false,
                hidden: false,
                type_id: std::any::TypeId::of::<#struct_name>,
                create: || Box::new(#struct_name),
                add_to_world: |world, id| world.add_tag::<#struct_name>(id),
            }
        }
    };
    output.into()
}

// ========== ========== Systems ========== ==========

fn is_query_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(p) = ty {
        if let Some(seg) = p.path.segments.last() {
            return seg.ident == "Query";
        }
    }
    false
}

/// Shared system registration logic.
///
/// `is_fixed` = true for `fixed_update`, whose base signature is
/// `fn(&mut World, f32) -> Result<()>` instead of `fn(&mut World) -> Result<()>`.
fn emit_system(
    input_fn: ItemFn,
    fn_name: &syn::Ident,
    priority: u32,
    mode: TokenStream2,
    system_struct: TokenStream2,
    is_fixed: bool,
) -> TokenStream2 {
    // Collect Query<...> params by name and type.
    let query_params: Vec<(syn::Ident, Box<syn::Type>)> = input_fn
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let syn::FnArg::Typed(pt) = arg {
                if is_query_type(&pt.ty) {
                    if let syn::Pat::Ident(pi) = &*pt.pat {
                        return Some((pi.ident.clone(), pt.ty.clone()));
                    }
                }
            }
            None
        })
        .collect();

    if query_params.is_empty() {
        // No queries — emit as-is.
        return quote! {
            #input_fn
            inventory::submit! {
                #system_struct {
                    name: stringify!(#fn_name),
                    func: #fn_name,
                    priority: #priority,
                    mode: #mode,
                }
            }
        };
    }

    // Build query-construction statements: `let __q0 = unsafe { Query::<T>::new(__w as *mut _) };`
    let query_inits: Vec<TokenStream2> = query_params
        .iter()
        .enumerate()
        .map(|(i, (_, ty))| {
            let var = syn::Ident::new(&format!("__q{i}"), proc_macro2::Span::call_site());
            quote! {
                let #var = unsafe { <#ty>::new(__w as *mut apostasy_core::ecs::World) };
            }
        })
        .collect();

    let query_vars: Vec<syn::Ident> = (0..query_params.len())
        .map(|i| syn::Ident::new(&format!("__q{i}"), proc_macro2::Span::call_site()))
        .collect();

    let orig_inputs = &input_fn.sig.inputs;
    let orig_output = &input_fn.sig.output;
    let orig_body = &input_fn.block;
    let orig_attrs = &input_fn.attrs;
    let vis = &input_fn.vis;

    // Wrapper signature and call-site args differ for fixed_update.
    let (wrapper_sig, call_args) = if is_fixed {
        (
            quote! { fn #fn_name(__w: &mut apostasy_core::ecs::World, __dt: f32) -> anyhow::Result<()> },
            quote! { __w, __dt, #(#query_vars),* },
        )
    } else {
        (
            quote! { fn #fn_name(__w: &mut apostasy_core::ecs::World) -> anyhow::Result<()> },
            quote! { __w, #(#query_vars),* },
        )
    };

    quote! {
        #(#orig_attrs)*
        #vis #wrapper_sig {
            #(#query_inits)*
            (|#orig_inputs| #orig_output #orig_body)(#call_args)
        }
        inventory::submit! {
            #system_struct {
                name: stringify!(#fn_name),
                func: #fn_name,
                priority: #priority,
                mode: #mode,
            }
        }
    }
}

struct SystemArgs {
    priority: Option<u32>,
    mode: Option<TokenStream2>,
}

/// Parser for the attribute arguments
impl Parse for SystemArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut priority = None;
        let mut mode = None;

        while !input.is_empty() {
            let name: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            if name == "priority" {
                let priority_lit: LitInt = input.parse()?;
                priority = Some(priority_lit.base10_parse()?);
            } else if name == "mode" {
                let mode_lit: LitStr = input.parse()?;
                mode = Some(match mode_lit.value().as_str() {
                    "game" => quote! { apostasy_core::EngineMode::Game },
                    "editor" => quote! { apostasy_core::EngineMode::Editor },
                    "all" => quote! { apostasy_core::EngineMode::All },
                    _ => {
                        return Err(syn::Error::new_spanned(
                            mode_lit,
                            "expected `game`, `editor`, or `all`",
                        ));
                    }
                });
            } else {
                return Err(syn::Error::new_spanned(
                    name,
                    "expected `priority` or `mode`",
                ));
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(SystemArgs { priority, mode })
    }
}

/// Registers a one-shot startup system, run once before the first frame.
///
/// # Signature
/// ```rust
/// fn my_system(world: &mut World) -> Result<()>
/// // or with queries:
/// fn my_system(world: &mut World, query: Query<(&ComponentA, &ComponentB)>) -> Result<()>
/// ```
///
/// # Options
/// - `mode = "game" | "editor" | "all"` — default: `"game"`
/// - `priority = <u32>` — higher runs first; default: `0`
///
/// # Example
/// ```rust
/// #[start]
/// fn setup(world: &mut World) -> Result<()> {
///     world.spawn();
///     Ok(())
/// }
///
/// #[start(mode = "editor", priority = 10)]
/// fn editor_setup(world: &mut World, query: Query<&Transform>) -> Result<()> {
///     query.for_each(|id, transform| {
///         println!("entity {:?} at {:?}", id, transform.local_position);
///     });
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn start(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as SystemArgs);
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = input_fn.sig.ident.clone();
    let priority = args.priority.unwrap_or(0);
    let mode = args
        .mode
        .unwrap_or(quote! { apostasy_core::EngineMode::Game });
    emit_system(
        input_fn,
        &fn_name,
        priority,
        mode,
        quote! { apostasy_core::ecs::systems::StartSystem },
        false,
    )
    .into()
}

/// Registers a pre-render system, run at the start of every frame before the scene is drawn.
///
/// Use this to update GPU-side state (push constants, material overrides) that must be
/// ready before the render pass begins.
///
/// # Signature
/// ```rust
/// fn my_system(world: &mut World) -> Result<()>
/// // or with queries:
/// fn my_system(world: &mut World, query: Query<(&ComponentA, &ComponentB)>) -> Result<()>
/// ```
///
/// # Options
/// - `mode = "game" | "editor" | "all"` — default: `"game"`
/// - `priority = <u32>` — higher runs first; default: `0`
///
/// # Example
/// ```rust
/// #[prerender(mode = "all")]
/// fn sync_push_constants(
///     world: &mut World,
///     query: Query<(&Transform, &Light)>,
/// ) -> Result<()> {
///     query.for_each(|_id, (transform, light)| {
///         // upload per-light data before the draw calls
///     });
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn prerender(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as SystemArgs);
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = input_fn.sig.ident.clone();
    let priority = args.priority.unwrap_or(0);
    let mode = args
        .mode
        .unwrap_or(quote! { apostasy_core::EngineMode::Game });
    emit_system(
        input_fn,
        &fn_name,
        priority,
        mode,
        quote! { apostasy_core::ecs::systems::PreRenderSystem },
        false,
    )
    .into()
}

/// Registers an update system, run every frame after rendering.
///
/// # Signature
/// ```rust
/// fn my_system(world: &mut World) -> Result<()>
/// // or with queries:
/// fn my_system(world: &mut World, query: Query<(&mut ComponentA, &ComponentB)>) -> Result<()>
/// // multiple queries:
/// fn my_system(world: &mut World, q1: Query<&ComponentA>, q2: Query<&ComponentB>) -> Result<()>
/// ```
///
/// # Options
/// - `mode = "game" | "editor" | "all"` — default: `"game"`
/// - `priority = <u32>` — higher runs first; default: `0`
///
/// # Example
/// ```rust
/// #[update]
/// fn move_players(
///     world: &mut World,
///     query: Query<(&mut Transform, &Velocity), WithTag<Player>>,
/// ) -> Result<()> {
///     let dt = world.get_resource::<DeltaTime>()?.0;
///     query.for_each(|_id, (transform, vel)| {
///         transform.local_position += vel.linear * dt;
///     });
///     Ok(())
/// }
///
/// // Filter with With/Without:
/// #[update]
/// fn update_active_cameras(
///     world: &mut World,
///     query: Query<&mut Camera, (With<Transform>, Without<Frozen>)>,
/// ) -> Result<()> {
///     query.for_each(|id, camera| { /* ... */ });
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn update(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as SystemArgs);
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = input_fn.sig.ident.clone();
    let priority = args.priority.unwrap_or(0);
    let mode = args
        .mode
        .unwrap_or(quote! { apostasy_core::EngineMode::Game });
    emit_system(
        input_fn,
        &fn_name,
        priority,
        mode,
        quote! { apostasy_core::ecs::systems::UpdateSystem },
        false,
    )
    .into()
}

/// Registers a fixed-timestep system, run at 20 Hz regardless of frame rate.
///
/// Receives a `delta: f32` parameter (the fixed timestep, ~0.05 s) in addition to `world`.
///
/// # Signature
/// ```rust
/// fn my_system(world: &mut World, delta: f32) -> Result<()>
/// // or with queries:
/// fn my_system(world: &mut World, delta: f32, query: Query<&mut ComponentA>) -> Result<()>
/// ```
///
/// # Options
/// - `mode = "game" | "editor" | "all"` — default: `"game"`
/// - `priority = <u32>` — higher runs first; default: `0`
///
/// # Example
/// ```rust
/// #[fixed_update]
/// fn physics_step(
///     world: &mut World,
///     delta: f32,
///     query: Query<(&mut Transform, &mut Velocity)>,
/// ) -> Result<()> {
///     query.for_each(|_id, (transform, vel)| {
///         transform.local_position += vel.linear * delta;
///     });
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn fixed_update(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as SystemArgs);
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = input_fn.sig.ident.clone();
    let priority = args.priority.unwrap_or(0);
    let mode = args
        .mode
        .unwrap_or(quote! { apostasy_core::EngineMode::Game });
    emit_system(
        input_fn,
        &fn_name,
        priority,
        mode,
        quote! { apostasy_core::ecs::systems::FixedUpdateSystem },
        true,
    )
    .into()
}

/// Registers a late-update system, run at the very end of every frame.
///
/// Runs after `update` and `fixed_update`. Use for camera tracking, cleanup, or anything
/// that must see the final state of the frame.
///
/// # Signature
/// ```rust
/// fn my_system(world: &mut World) -> Result<()>
/// // or with queries:
/// fn my_system(world: &mut World, query: Query<(&ComponentA, &ComponentB)>) -> Result<()>
/// ```
///
/// # Options
/// - `mode = "game" | "editor" | "all"` — default: `"game"`
/// - `priority = <u32>` — higher runs first; default: `0`
///
/// # Example
/// ```rust
/// #[late_update(mode = "all")]
/// fn follow_camera(
///     world: &mut World,
///     query: Query<&mut Transform, WithTag<ActiveCamera>>,
/// ) -> Result<()> {
///     query.for_each(|_id, transform| {
///         // camera smoothing, lag, etc.
///     });
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn late_update(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as SystemArgs);
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = input_fn.sig.ident.clone();
    let priority = args.priority.unwrap_or(0);
    let mode = args
        .mode
        .unwrap_or(quote! { apostasy_core::EngineMode::Game });
    emit_system(
        input_fn,
        &fn_name,
        priority,
        mode,
        quote! { apostasy_core::ecs::systems::LateUpdateSystem },
        false,
    )
    .into()
}
