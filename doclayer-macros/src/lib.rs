//! Procedural macros for the doclayer project.

#[allow(unused_extern_crates)]
extern crate self as doclayer_macros;

mod projection;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

/// Derives [`doclayer_core::query::Projection`] for a struct.
///
/// For named structs, field names are used as projection paths. Annotate a field with
/// `#[project]` to recurse into that type's `Projection` impl, prefixing paths with the
/// field name using dot notation.
///
/// For newtype structs, add `#[projection(fields = ["a", "b.c"])]` to specify paths manually.
#[proc_macro_derive(Projection, attributes(project, projection))]
pub fn derive_projection(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    projection::ProjectionDerive::new(&input)
        .expand()
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
