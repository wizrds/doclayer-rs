//! A thin JSON document database abstraction layer that provides a unified interface for working with document stores.
//!
//! This crate is the core of the doclayer project and provides:
//!
//! - **Document traits** ([`document`]) - Core traits for defining and serializing documents
//! - **Store backend abstraction** ([`backend`]) - Traits for implementing different storage backends
//! - **Query and filtering API** ([`query`]) - Type-safe query construction and filtering
//! - **Collections interface** ([`collection`]) - High-level API for interacting with document collections
//! - **Document store** ([`store`]) - Main interface for working with typed or untyped documents
//! - **Error handling** ([`error`]) - Comprehensive error types and result types
//! - **Type utilities** ([`types`]) - Common types like pagination and page results
//! - **Schema migrations** ([`migrate`]) - Tools for versioning and migrating document schemas
//!
//! # Example
//!
//! ```ignore
//! use doclayer::{Document, DocumentStore};
//! use bson::Uuid;
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! pub struct User {
//!     pub id: Uuid,
//!     pub name: String,
//! }
//!
//! impl Document for User {
//!     fn id(&self) -> &Uuid {
//!         &self.id
//!     }
//!
//!     fn collection_name() -> &'static str {
//!         "users"
//!     }
//! }
//! ```

#[allow(unused_extern_crates)]
extern crate self as doclayer_core;

pub mod backend;
pub mod collection;
pub mod document;
pub mod error;
pub mod migrate;
pub mod query;
pub mod store;
pub mod page;
