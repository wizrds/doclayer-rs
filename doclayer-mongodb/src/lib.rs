//! MongoDB backend implementation for doclayer.
//!
//! This crate provides a MongoDB-based implementation of the `StoreBackend` trait,
//! enabling persistent document storage with full query support using MongoDB's querying capabilities.
//! 
//! To use this backend, include the `mongodb` feature in your `Cargo.toml`:
//! 
//! ```toml
//! [dependencies]
//! doclayer = { version = "x.y.z", features = ["mongodb"] }
//! ```
//!
//! # Features
//!
//! - **Persistent storage** - Data is persisted to MongoDB Atlas or self-hosted MongoDB
//! - **Full query support** - Leverages MongoDB's query engine for filtering and sorting
//! - **Async/await** - Fully asynchronous API built on MongoDB's async driver
//! - **Indexing** - Support for creating and dropping MongoDB indexes
//! - **Schema migrations** - Compatible with the doclayer migration framework
//!
//! # Connection
//!
//! To use this backend, you need a MongoDB connection string. This can be provided
//! through the builder pattern.
//!
//! # Example
//!
//! ```ignore
//! use doclayer::{backend::StoreBackendBuilder, mongodb::MongoDbStore};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let store = MongoDbStore::builder("mongodb://localhost:27017", "my_database")
//!         .build()
//!         .await?;
//!         
//!     Ok(())
//! }
//! ```

#[allow(unused_extern_crates)]
extern crate self as doclayer_mongodb;

pub mod store;
pub mod query;
pub mod sanitizer;

pub use store::{MongoDbStore, MongoDbStoreBuilder};
