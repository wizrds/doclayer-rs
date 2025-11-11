//! Main doclayer crate providing a unified interface for document storage.
//!
//! This crate is the primary entry point for users of the doclayer framework.
//! It re-exports the core types and functionality from various sub-crates and provides
//! convenient access to different storage backends.
//!
//! # Features
//!
//! - **Type-safe document storage** - Define your data structures with Serde and store them safely
//! - **Multiple backends** - Support for in-memory and MongoDB storage with extensible trait system
//! - **Flexible querying** - Powerful, composable query API for filtering and sorting
//! - **Schema migrations** - Versioned migrations for evolving your data models
//!
//! # Quick Start
//!
//! ```ignore
//! use doclayer::{prelude::*, memory::InMemoryStore};
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
//!     fn id(&self) -> &Uuid { &self.id }
//!     fn collection_name() -> &'static str { "users" }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create an in-memory store backend
//!     let store = DocumentStore::new(InMemoryStore::builder().build().await.unwrap());
//!     
//!     // Get a typed collection for User documents
//!     let user_collection = store.typed_collection::<User>();
//!     
//!     // Use the store to insert and query documents
//!     let user = User {
//!         id: Uuid::new(),
//!         name: "Alice".to_string(),
//!     };
//! 
//!     // Insert the user document
//!     user_collection.insert(vec![user.clone()]).await.unwrap();
//! 
//!     // Query for the user document
//!     let results = user_collection
//!         .query(
//!             Query::builder()
//!                 .filter(Field::new("name").eq("Alice"))
//!                 .build(),
//!         )
//!         .await
//!         .unwrap();
//! 
//!     println!("Queried users: {:?}", results);
//!     
//!     // Shutdown the store
//!     store.shutdown().await.unwrap();
//! }
//! ```
//!
//! # Dynamic Dispatch
//! 
//! The `doclayer` crate also supports dynamic dispatch for scenarios where backend types
//! are not known at compile time. You can convert a typed `DocumentStore` into a
//! dynamically dispatched store using the `into_dyn` method. This allows for runtime
//! selection of backends and flexible handling of documents without static type information.
//! 
//! ```ignore
//! use doclayer::{prelude::*, memory::InMemoryStore};
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
//!     fn id(&self) -> &Uuid { &self.id }
//!     fn collection_name() -> &'static str { "users" }
//! }
//! 
//! #[tokio::main]
//! async fn main() {
//!     // Create an in-memory store backend
//!     let store = DocumentStore::new(InMemoryStore::builder().build().await.unwrap());
//! 
//!     // Convert to a dynamically dispatched store
//!     let dyn_store = store.into_dyn();
//! 
//!     // Use the dynamic store to get a collection
//!     let user_collection = dyn_store.typed_collection::<User>();
//! 
//!     // Insert and query documents as before
//!     let user = User {
//!         id: Uuid::new(),
//!         name: "Bob".to_string(),
//!     };
//! 
//!     user_collection.insert(vec![user.clone()]).await.unwrap();
//! 
//!     let results = user_collection
//!         .query(
//!             Query::builder()
//!                 .filter(Field::new("name").eq("Bob"))
//!                 .build(),
//!         )
//!         .await
//!         .unwrap();
//! 
//!     println!("Queried users: {:?}", results);
//!     
//!     // Shutdown the store
//!     // The dynamic store must use the `shutdown_boxed` method
//!     dyn_store.shutdown_boxed().await.unwrap();
//! }
//! ```
//! 
//! # Migrations
//! 
//! The doclayer framework includes support for schema migrations to help evolve your data models
//! over time. You can define migrations that modify document structures and apply them
//! to your document store.
//! 
//! ```ignore
//! use doclayer::{prelude::*, memory::InMemoryStore, migrate::{Migration, MigrationRef, MigrateOp, Migrations}};
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
//!     fn id(&self) -> &Uuid { &self.id }
//!     fn collection_name() -> &'static str { "users" }
//! }
//! 
//! struct AddAgeToUserMigration;
//! 
//! #[async_trait::async_trait]
//! impl Migration for AddAgeToUserMigration {
//!     fn id(&self) -> &'static str { "add_age_to_user" }
//!     fn previous_id(&self) -> Option<&'static str> { None }
//! 
//!     async fn up(&self, op: &MigrateOp<'_>) -> DocumentStoreResult<()> {
//!         op.add_field(User::collection_name(), "age", 0).await?;
//! 
//!         Ok(())
//!     }
//!     
//!     async fn down(&self, op: &MigrateOp<'_>) -> DocumentStoreResult<()> {
//!         op.remove_field(User::collection_name(), "age").await?;
//! 
//!         Ok(())
//!     }
//! }
//! 
//! struct MyMigrations;
//! 
//! impl Migrations for MyMigrations {
//!     fn migrations() -> Vec<MigrationRef> {
//!         vec![
//!             Box::new(AddAgeToUserMigration),
//!         ]
//!     }
//! }
//! 
//! #[tokio::main]
//! async fn main() {
//!     let store = DocumentStore::new(InMemoryStore::builder().build().await.unwrap());
//!
//!     // Upgrade to head
//!     store
//!         .upgrade::<MyMigrations>()
//!         .await
//!         .unwrap();
//! 
//!     // Downgrade to specific version
//!     store
//!         .downgrade_to::<MyMigrations>("add_age_to_user")
//!         .await
//!         .unwrap();
//! }
//! ```
//!
//! # Backends
//!
//! - [`memory`] - Fast in-memory storage for development and testing
//! - [`mongodb`] - Persistent MongoDB backend (requires `mongodb` feature)

pub mod prelude;

pub use doclayer_core::{collection, document, store, backend, query, migrate, error};

// Re-export BSON types for convenience
pub use bson;

/// In-memory storage backend implementations.
pub mod memory {
    pub use doclayer_memory::{InMemoryStore, InMemoryStoreBuilder};
}

/// MongoDB storage backend implementations.
///
/// This module is only available when the `mongodb` feature is enabled.
#[cfg(feature = "mongodb")]
pub mod mongodb {
    pub use doclayer_mongodb::{MongoDbStore, MongoDbStoreBuilder};
}
