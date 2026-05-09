use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{DeriveInput, ItemFn, LitInt, parse_macro_input, parse_quote};

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
        impl #impl_generics apostasy_core::objects::component::Component for #struct_name #type_generics
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
            apostasy_core::objects::component::ComponentRegistration {
                type_name: #struct_name_str,
                create: || Box::new(#struct_name::default()),
                deserialize: |component, value| {
                    if let Some(c) = component.as_any_mut().downcast_mut::<#struct_name>() {
                        c.deserialize(value)
                    } else {
                        Ok(())
                    }
                },
            }
        };
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
     impl #impl_generics apostasy_core::objects::resource::Resource for #struct_name #type_generics
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
     impl #impl_generics apostasy_core::objects::tag::Tag for #struct_name #type_generics
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
}

/// Parser for the attribute arguments
impl Parse for SystemArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(SystemArgs { priority: None });
        }

        let name: syn::Ident = input.parse()?;
        if name != "priority" {
            return Err(syn::Error::new_spanned(name, "expected `priority`"));
        }

        input.parse::<syn::Token![=]>()?;
        let priority_lit: LitInt = input.parse()?;
        let priority: u32 = priority_lit.base10_parse()?;

        Ok(SystemArgs {
            priority: Some(priority),
        })
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

    let expanded = quote! {
        #input_fn
        inventory::submit! {
            apostasy_core::objects::systems::StartSystem{
                name: stringify!(#fn_name),
                func: #fn_name,
                priority: #priority,
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

    let expanded = quote! {
        #input_fn
        inventory::submit! {
            apostasy_core::objects::systems::UpdateSystem{
                name: stringify!(#fn_name),
                func: #fn_name,
                priority: #priority,
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

    let expanded = quote! {
        #input_fn
        inventory::submit! {
            apostasy_core::objects::systems::FixedUpdateSystem{
                name: stringify!(#fn_name),
                func: #fn_name,
                priority: #priority,
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

    let expanded = quote! {
        #input_fn
        inventory::submit! {
            apostasy_core::objects::systems::LateUpdateSystem{
                name: stringify!(#fn_name),
                func: #fn_name,
                priority: #priority,
            }
        }
    };
    TokenStream::from(expanded)
}
