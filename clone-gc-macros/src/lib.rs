mod clone;
mod trace;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

use crate::{clone::derive_graph_clone_impl_internal, trace::derive_trace_impl_internal};

#[proc_macro_derive(Trace, attributes(trace))]
pub fn derive_trace_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match derive_trace_impl_internal(input) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.write_errors().into(),
    }
}

#[proc_macro_derive(GraphClone, attributes(graphClone))]
pub fn derive_graph_clone_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match derive_graph_clone_impl_internal(input) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.write_errors().into(),
    }
}
