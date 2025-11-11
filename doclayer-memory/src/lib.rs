//! In-memory document storage backend for doclayer.
//!
//! This crate provides a thread-safe, in-memory implementation of the `StoreBackend` trait.
//! It uses async-aware read-write locks for concurrent access and is ideal for development,
//! testing, and small-scale deployments.
//!
//! # Features
//!
//! - **Thread-safe access** - Concurrent reads and writes using async-aware RwLock
//! - **Type-erased storage** - Stores documents as BSON for flexibility
//! - **Full query support** - Supports filtering, sorting, and pagination
//! - **Revision tracking** - Optional revision ID tracking for migrations
//!
//! # Quick Start
//!
//! ```ignore
//! use doclayer::{Document, DocumentStore, memory::InMemoryStore};
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
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let backend = InMemoryStore::builder().build().await.unwrap()
//!     let store = DocumentStore::new(backend);
//!     let user_collection = store.typed_collection::<User>();    
//! 
//!     let user = User {
//!         id: Uuid::new(),
//!         name: "Alice".to_string(),
//!     };
//!     
//!     user_collection.insert(vec![user.clone()]).await.unwrap();    
//! 
//!     Ok(())
//! }
//! ```

#[allow(unused_extern_crates)]
extern crate self as doclayer_memory;

pub mod store;
pub mod evaluator;

pub use store::{InMemoryStore, InMemoryStoreBuilder};
