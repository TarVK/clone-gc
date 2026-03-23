use darling::FromField;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{Data, DeriveInput, Field, Fields};

pub fn derive_trace_impl_internal(input: DeriveInput) -> darling::Result<TokenStream> {
    let name = input.ident;
    let body = match input.data {
        Data::Struct(data) => trace_struct(&data.fields)?,
        Data::Enum(data) => trace_enum(&name, &data.variants)?,
        _ => panic!("Can only derive Trace for structs and enums"),
    };

    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics clone_gc::Trace for #name #ty_generics #where_clause {
            fn trace(&self, tracer: &mut clone_gc::GCTracer) {
                #body
            }
        }
    };

    Ok(expanded.into())
}

fn trace_struct(fields: &syn::Fields) -> darling::Result<proc_macro2::TokenStream> {
    let res = match fields {
        Fields::Named(fields) => {
            let tracers = fields
                .named
                .iter()
                .map(|f| {
                    let name = &f.ident;
                    get_trace_code(f, quote! {self.#name})
                })
                .collect::<Result<Vec<_>, _>>()?;
            quote! { #(#tracers)* }
        }
        Fields::Unnamed(fields) => {
            let tracers = fields
                .unnamed
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let idx = syn::Index::from(i);
                    get_trace_code(f, quote! {self.#idx})
                })
                .collect::<Result<Vec<_>, _>>()?;
            quote! { #(#tracers)* }
        }
        Fields::Unit => quote! {},
    };
    Ok(res)
}
fn trace_enum(
    name: &syn::Ident,
    variants: &syn::punctuated::Punctuated<syn::Variant, syn::token::Comma>,
) -> darling::Result<proc_macro2::TokenStream> {
    let arms = variants
        .iter()
        .map(|variant| -> darling::Result<_> {
            let vname = &variant.ident;

            let arm = match &variant.fields {
                Fields::Named(fields) => {
                    let names: Vec<_> = fields
                        .named
                        .iter()
                        .map(|f| f.ident.as_ref().unwrap())
                        .collect();
                    let tracers = fields
                        .named
                        .iter()
                        .zip(names.clone())
                        .map(|(f, name)| get_trace_code(f, quote! {#name}))
                        .collect::<Result<Vec<_>, _>>()?;
                    quote! {
                        #name::#vname { #( #names ),* } => { #(#tracers)* }
                    }
                }

                Fields::Unnamed(fields) => {
                    let bindings: Vec<_> = (0..fields.unnamed.len())
                        .map(|i| syn::Ident::new(&format!("f{i}"), proc_macro2::Span::call_site()))
                        .collect();
                    let tracers = fields
                        .unnamed
                        .iter()
                        .zip(bindings.clone())
                        .map(|(f, id)| get_trace_code(f, quote! {#id}))
                        .collect::<Result<Vec<_>, _>>()?;
                    quote! {
                        #name::#vname( #( #bindings ),* ) => { #(#tracers)* }
                    }
                }

                Fields::Unit => {
                    quote! { #name::#vname => {} }
                }
            };
            Ok(arm)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let res = quote! {
        match self {
            #(#arms),*
        }
    };
    Ok(res)
}

#[derive(FromField)]
#[darling(attributes(trace))]
struct FieldTraceOptions {
    skip: Option<bool>,
}

fn get_trace_code(
    field: &Field,
    val: proc_macro2::TokenStream,
) -> darling::Result<proc_macro2::TokenStream> {
    let field_trace = FieldTraceOptions::from_field(&field)?;
    if Some(true) == field_trace.skip {
        Ok(quote! {})
    } else {
        Ok(quote! { #val.trace(tracer); })
    }
}
