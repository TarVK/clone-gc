use darling::FromField;
use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Field, Fields};

pub fn derive_graph_clone_impl_internal(input: DeriveInput) -> darling::Result<TokenStream> {
    let name = input.ident;
    let body = match input.data {
        Data::Struct(data) => graph_clone_struct(&data.fields)?,
        Data::Enum(data) => graph_clone_enum(&name, &data.variants)?,
        _ => panic!("Can only derive GraphClone for structs and enums"),
    };

    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics clone_gc::GraphClone for #name #ty_generics #where_clause {
            fn graph_clone(&self, m: &mut clone_gc::GraphCloneState) -> Self {
                #body
            }
        }
    };

    Ok(expanded.into())
}

fn graph_clone_struct(fields: &syn::Fields) -> darling::Result<proc_macro2::TokenStream> {
    let res = match fields {
        Fields::Named(fields) => {
            let clones = fields
                .named
                .iter()
                .map(|f| -> darling::Result<_> {
                    let name = &f.ident;
                    let init = get_graph_clone_code(f, quote! {self.#name})?;
                    Ok(quote! { #name: #init })
                })
                .collect::<Result<Vec<_>, _>>()?;
            quote! { Self { #(#clones),* } }
        }
        Fields::Unnamed(fields) => {
            let clones = fields
                .unnamed
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let idx = syn::Index::from(i);
                    get_graph_clone_code(f, quote! {self.#idx})
                })
                .collect::<Result<Vec<_>, _>>()?;
            quote! { Self ( #(#clones),* ) }
        }
        Fields::Unit => quote! { Self },
    };
    Ok(res)
}
fn graph_clone_enum(
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
                    let clones = fields
                        .named
                        .iter()
                        .zip(names.clone())
                        .map(|(f, name)| -> darling::Result<_> {
                            let init = get_graph_clone_code(f, quote! { #name })?;
                            Ok(quote! { #name: #init })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    quote! {
                        #name::#vname { #( #names ),* } => {
                            #name::#vname {
                                #(#clones),*
                            }
                        }
                    }
                }

                Fields::Unnamed(fields) => {
                    let bindings: Vec<_> = (0..fields.unnamed.len())
                        .map(|i| syn::Ident::new(&format!("f{i}"), proc_macro2::Span::call_site()))
                        .collect();
                    let clones = fields
                        .unnamed
                        .iter()
                        .zip(bindings.clone())
                        .map(|(f, id)| get_graph_clone_code(f, quote! {#id}))
                        .collect::<Result<Vec<_>, _>>()?;
                    quote! {
                        #name::#vname( #( #bindings ),* ) => {
                            #name::#vname (
                                #(#clones),*
                            )
                        }
                    }
                }

                Fields::Unit => {
                    quote! { #name::#vname => #name::#vname  }
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
#[darling(attributes(graphClone))]
struct FieldGraphcloneOptions {
    shallow: Option<bool>,
    skip: Option<bool>,
}

fn get_graph_clone_code(
    field: &Field,
    val: proc_macro2::TokenStream,
) -> darling::Result<proc_macro2::TokenStream> {
    let field_trace = FieldGraphcloneOptions::from_field(&field)?;
    if Some(true) == field_trace.skip {
        Ok(quote! { default::Default() })
    } else if Some(true) == field_trace.shallow {
        Ok(quote! { #val.clone() })
    } else {
        Ok(quote! { #val.graph_clone(m) })
    }
}
