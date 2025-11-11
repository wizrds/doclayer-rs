//! Convenient re-exports of commonly used types from doclayer.
//!
//! Import this prelude module to quickly access the most frequently used types
//! and traits without needing to import from multiple sub-modules:
//!
//! ```ignore
//! use doclayer::prelude::*;
//! ```
//!
//! This provides access to:
//! - Document traits and implementations
//! - Store backends and builders
//! - Query construction and filtering
//! - Collection interfaces
//! - Error types and migration tools

pub use doclayer_core::{
    collection::{Collection, DynCollection},
    store::{DocumentStore, DynDocumentStore, DynDocumentStoreRef, AsDynDocumentStore, IntoDynDocumentStore, AsStaticDocumentStore, IntoStaticDocumentStore},
    document::{Document, DocumentExt},
    backend::{StoreBackend, DynStoreBackend, StoreBackendBuilder},
    query::{Query, QueryVisitor, Expr, Sort, SortDirection, FieldOp, QueryBuilder, Filter},
    migrate::{Migration, MigrationDirection, MigrationRef, MigrateOp, MigrationRunner, Migrations, Migrator},
    error::{DocumentStoreError, DocumentStoreResult},
};
