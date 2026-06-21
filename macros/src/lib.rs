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

    let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();

    let output = quote! {
        impl #impl_generics apostasy_core::ecs::tags::Tag for #struct_name #type_generics
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

            fn type_name_static() -> &'static str {
                std::any::type_name::<Self>()
            }
        }
    };
    output.into()
}

#[proc_macro_derive(TagWithInventory)]
pub fn tag_with_inventory_derive(input: TokenStream) -> TokenStream {
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
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
            fn type_name(&self) -> &'static str {
                std::any::type_name::<Self>()
            }

            fn type_name_static() -> &'static str {
                std::any::type_name::<Self>()
            }
        }
    };
    output.into()
}

// ========== ========== Systems ========== ==========

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

/// Registers a start system, Start systems run once at the start of the game
/// NOTE: systems with a higher priority run first
/// NOTE: priority is non negative
#[proc_macro_attribute]
pub fn start(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as SystemArgs);
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;

    let priority = args.priority.unwrap_or(0);
    let mode = args
        .mode
        .unwrap_or(quote! { apostasy_core::EngineMode::Game });

    let expanded = quote! {
        #input_fn
        inventory::submit! {
            apostasy_core::ecs::systems::StartSystem{
                name: stringify!(#fn_name),
                func: #fn_name,
                priority: #priority,
                mode: #mode,
            }
        }
    };
    TokenStream::from(expanded)
}

/// Registers a preupdate system, pre render systems run before each frame
/// NOTE: systems with a higher priority run first
/// NOTE: priority is non negative
#[proc_macro_attribute]
pub fn prerender(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as SystemArgs);
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;

    let priority = args.priority.unwrap_or(0);
    let mode = args
        .mode
        .unwrap_or(quote! { apostasy_core::EngineMode::Game });

    let expanded = quote! {
        #input_fn
        inventory::submit! {
            apostasy_core::ecs::systems::PreRenderSystem{
                name: stringify!(#fn_name),
                func: #fn_name,
                priority: #priority,
                mode: #mode,
            }
        }
    };
    TokenStream::from(expanded)
}

/// Registers an update system, Update systems run each frame
/// NOTE: systems with a higher priority run first
/// NOTE: priority is non negative
#[proc_macro_attribute]
pub fn update(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as SystemArgs);
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;

    let priority = args.priority.unwrap_or(0);
    let mode = args
        .mode
        .unwrap_or(quote! { apostasy_core::EngineMode::Game });

    let expanded = quote! {
        #input_fn
        inventory::submit! {
            apostasy_core::ecs::systems::UpdateSystem{
                name: stringify!(#fn_name),
                func: #fn_name,
                priority: #priority,
                mode: #mode,
            }
        }
    };
    TokenStream::from(expanded)
}

/// Registers a fixed update system, Fixed update systems run x amount of times a second
/// NOTE: systems with a higher priority run first
/// NOTE: priority is non negative
#[proc_macro_attribute]
pub fn fixed_update(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as SystemArgs);
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;

    let priority = args.priority.unwrap_or(0);
    let mode = args
        .mode
        .unwrap_or(quote! { apostasy_core::EngineMode::Game });

    let expanded = quote! {
        #input_fn
        inventory::submit! {
            apostasy_core::ecs::systems::FixedUpdateSystem{
                name: stringify!(#fn_name),
                func: #fn_name,
                priority: #priority,
                mode: #mode,
            }
        }
    };
    TokenStream::from(expanded)
}

/// Registers a late update system, Late update systems run at the end of a frame
/// NOTE: systems with a higher priority run first
/// NOTE: priority is non negative
#[proc_macro_attribute]
pub fn late_update(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as SystemArgs);
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;

    let priority = args.priority.unwrap_or(0);
    let mode = args
        .mode
        .unwrap_or(quote! { apostasy_core::EngineMode::Game });

    let expanded = quote! {
        #input_fn
        inventory::submit! {
            apostasy_core::ecs::systems::LateUpdateSystem{
                name: stringify!(#fn_name),
                func: #fn_name,
                priority: #priority,
                mode: #mode,
            }
        }
    };
    TokenStream::from(expanded)
}
