//! Procedural macros for the doclayer project.
//!
//! This crate provides compile-time code generation for the doclayer framework,
//! enabling ergonomic derive macros and other compile-time utilities.

#[allow(unused_extern_crates)]
extern crate self as doclayer_macros;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};
