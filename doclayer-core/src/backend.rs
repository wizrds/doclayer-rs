//! Storage backend abstraction for the document store.
//!
//! This module defines the core traits that abstract over different storage implementations,
//! allowing the document store to work with various backends (in-memory, persistent, distributed, etc.).
//!
//! # Overview
//!
//! The [`StoreBackend`] trait provides a unified async interface for all storage operations
//! including document insertion, retrieval, deletion, querying, and collection management.
//! Implementations are required to be thread-safe (`Send + Sync`) and support concurrent access.
//!
//! # Traits
//!
//! - [`StoreBackend`]: The core trait for storage backends
//! - [`DynStoreBackend`]: A trait for dynamic dispatch over backend implementations
//! - [`StoreBackendBuilder`]: Factory trait for creating backend instances
//!
//! # Examples
//!
//! ```ignore
//! use doclayer::backend::StoreBackend;
//! use bson::{Uuid, Bson, doc};
//!
//! // Use a concrete backend implementation
//! let backend = MyBackendImpl::new();
//!
//! // Insert a document into a collection
//! let uuid = Uuid::new();
//! let doc = Bson::Document(doc! { "name": "Alice", "age": 30 });
//! backend.insert_documents(vec![(uuid, doc)], "users").await?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use async_trait::async_trait;
use bson::{Bson, Uuid};
use std::{any::Any, fmt::Debug};

use crate::{error::DocumentStoreResult, query::Query};

/// Abstract interface for document storage backends.
///
/// Implementers of this trait provide concrete storage strategies for documents,
/// supporting everything from simple in-memory stores to complex distributed systems.
/// The trait defines essential operations for document lifecycle management and collection
/// administration.
///
/// # Thread Safety
///
/// All implementations must be thread-safe and support concurrent access from multiple
/// async tasks. The exact concurrency model (e.g., lock-free, mutex-based, read-write locks)
/// is implementation-specific but should be documented by the implementer.
///
/// # Async Runtime
///
/// All methods are async and can be awaited. They support cancellation-safe semantics
/// typical of Rust async functions.
///
/// # Error Handling
///
/// Operations return [`DocumentStoreResult<T>`](crate::error::DocumentStoreResult),
/// which is a specialized `Result` type. Implementers should document which error
/// variants may be returned by each operation.
#[async_trait]
pub trait StoreBackend: Send + Sync + Debug {
    /// Inserts new documents into a collection, overwriting any existing documents with the same IDs.
    ///
    /// This method batches the insertion of multiple documents into a single collection.
    /// If a document with the same ID already exists, it is replaced entirely.
    ///
    /// # Arguments
    ///
    /// * `documents` - A vector of (UUID, BSON document) pairs to insert
    /// * `collection` - The name of the collection to insert into. Created automatically if it doesn't exist.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or a [`DocumentStoreError`](crate::error::DocumentStoreError) on failure.
    async fn insert_documents(
        &self,
        documents: Vec<(Uuid, Bson)>,
        collection: &str,
    ) -> DocumentStoreResult<()>;

    /// Updates existing documents in a collection, replacing them entirely.
    ///
    /// This method updates multiple documents in a single collection. If a document with the
    /// specified ID does not exist, this may be treated as an error depending on the backend
    /// implementation. Check the specific backend documentation for its behavior.
    ///
    /// # Arguments
    ///
    /// * `documents` - A vector of (UUID, BSON document) pairs with updated content
    /// * `collection` - The name of the collection containing the documents
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or a [`DocumentStoreError`](crate::error::DocumentStoreError) on failure.
    async fn update_documents(
        &self,
        documents: Vec<(Uuid, Bson)>,
        collection: &str,
    ) -> DocumentStoreResult<()>;

    /// Deletes documents from a collection by their IDs.
    ///
    /// This method removes the specified documents from the collection. If a document with
    /// a given ID doesn't exist, it is silently skipped (idempotent operation).
    ///
    /// # Arguments
    ///
    /// * `ids` - A vector of document UUIDs to delete
    /// * `collection` - The name of the collection to delete from
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or a [`DocumentStoreError`](crate::error::DocumentStoreError) on failure.
    async fn delete_documents(&self, ids: Vec<Uuid>, collection: &str) -> DocumentStoreResult<()>;

    /// Retrieves documents from a collection by their IDs.
    ///
    /// This method fetches multiple documents in a single operation. Documents are returned
    /// in the order they exist in the store (order not guaranteed to match request order).
    /// If a document ID doesn't exist, it is simply omitted from the results.
    ///
    /// # Arguments
    ///
    /// * `ids` - A vector of document UUIDs to retrieve
    /// * `collection` - The name of the collection to query
    ///
    /// # Returns
    ///
    /// Returns a vector of BSON documents found, or a [`DocumentStoreError`](crate::error::DocumentStoreError) on failure.
    async fn get_documents(
        &self,
        ids: Vec<Uuid>,
        collection: &str,
    ) -> DocumentStoreResult<Vec<Bson>>;

    /// Queries documents in a collection using a structured query.
    ///
    /// This method applies filter expressions, sorting, pagination, and other query operations
    /// to select and return matching documents from the collection.
    ///
    /// # Arguments
    ///
    /// * `query` - The [`Query`] object specifying filters, sorts, limits, and offsets
    /// * `collection` - The name of the collection to query
    ///
    /// # Returns
    ///
    /// Returns a vector of matching BSON documents, or a [`DocumentStoreError`](crate::error::DocumentStoreError) on failure.
    ///
    /// # See Also
    ///
    /// - [`Query`] for constructing queries
    /// - [`crate::query::Filter`] for building filter expressions
    async fn query_documents(
        &self,
        query: Query,
        collection: &str,
    ) -> DocumentStoreResult<Vec<Bson>>;

    /// Retrieves the current revision/version ID of the store.
    ///
    /// Some backends track the overall revision of the store (useful for change detection,
    /// caching invalidation, or optimistic concurrency control). This returns the current
    /// revision ID if supported by the backend, or `None` if not.
    ///
    /// # Returns
    ///
    /// Returns `Ok(Some(id))` with the current revision, `Ok(None)` if not supported,
    /// or a [`DocumentStoreError`](crate::error::DocumentStoreError) on failure.
    async fn current_revision_id(&self) -> DocumentStoreResult<Option<String>>;

    /// Sets or updates the revision/version ID of the store.
    ///
    /// Allows backends that track revision information to update it. This is typically
    /// used during migrations or when synchronizing with external systems.
    ///
    /// # Arguments
    ///
    /// * `revision_id` - The new revision ID string
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or a [`DocumentStoreError`](crate::error::DocumentStoreError) on failure.
    async fn set_revision_id(&self, revision_id: &str) -> DocumentStoreResult<()>;

    /// Creates a new collection with the specified name.
    ///
    /// Creates an empty collection. If the collection already exists, this may be treated
    /// as an error depending on the backend implementation.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the collection to create
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or a [`DocumentStoreError`](crate::error::DocumentStoreError) on failure.
    async fn create_collection(&self, name: &str) -> DocumentStoreResult<()>;

    /// Drops (deletes) a collection and all its documents.
    ///
    /// This is a destructive operation that permanently removes the collection and all
    /// documents it contains. If the collection doesn't exist, this may be treated as
    /// an error or silently ignored depending on the backend.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the collection to drop
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or a [`DocumentStoreError`](crate::error::DocumentStoreError) on failure.
    ///
    /// # Warning
    ///
    /// This operation is irreversible. Ensure you have backups if the data is important.
    async fn drop_collection(&self, name: &str) -> DocumentStoreResult<()>;

    /// Lists the names of all collections in the store.
    ///
    /// # Returns
    ///
    /// Returns a vector of collection names, or a [`DocumentStoreError`](crate::error::DocumentStoreError) on failure.
    async fn list_collections(&self) -> DocumentStoreResult<Vec<String>>;

    /// Adds a new field to all documents in a collection with a default value.
    ///
    /// This is a schema migration operation that adds a field to every document.
    /// For backends that don't support schema, this may be a no-op or an error.
    ///
    /// # Arguments
    ///
    /// * `collection` - The name of the collection
    /// * `field` - The name of the field to add
    /// * `default` - The default BSON value for existing documents
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or a [`DocumentStoreError`](crate::error::DocumentStoreError) on failure.
    async fn add_field(
        &self,
        collection: &str,
        field: &str,
        default: Bson,
    ) -> DocumentStoreResult<()>;

    /// Removes a field from all documents in a collection.
    ///
    /// This is a schema migration operation. If a document doesn't have the field,
    /// it is left unchanged.
    ///
    /// # Arguments
    ///
    /// * `collection` - The name of the collection
    /// * `field` - The name of the field to remove
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or a [`DocumentStoreError`](crate::error::DocumentStoreError) on failure.
    async fn drop_field(&self, collection: &str, field: &str) -> DocumentStoreResult<()>;

    /// Renames a field in all documents of a collection.
    ///
    /// This is a schema migration operation that renames a field across all documents.
    ///
    /// # Arguments
    ///
    /// * `collection` - The name of the collection
    /// * `field` - The current name of the field
    /// * `new` - The new name for the field
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or a [`DocumentStoreError`](crate::error::DocumentStoreError) on failure.
    async fn rename_field(
        &self,
        collection: &str,
        field: &str,
        new: &str,
    ) -> DocumentStoreResult<()>;

    /// Creates an index on a field in a collection.
    ///
    /// Indexes improve query performance on frequently queried fields. Uniqueness constraints
    /// can be enforced by setting `unique` to `true`.
    ///
    /// # Arguments
    ///
    /// * `collection` - The name of the collection
    /// * `field` - The name of the field to index
    /// * `unique` - Whether this index should enforce uniqueness constraints
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or a [`DocumentStoreError`](crate::error::DocumentStoreError) on failure.
    ///
    /// # Note
    ///
    /// If `unique` is true and existing documents violate the uniqueness constraint,
    /// the backend may return an error.
    async fn add_index(
        &self,
        collection: &str,
        field: &str,
        unique: bool,
    ) -> DocumentStoreResult<()>;

    /// Removes an index from a collection.
    ///
    /// # Arguments
    ///
    /// * `collection` - The name of the collection
    /// * `field` - The name of the indexed field
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or a [`DocumentStoreError`](crate::error::DocumentStoreError) on failure.
    async fn drop_index(&self, collection: &str, field: &str) -> DocumentStoreResult<()>;

    /// Cleanly shuts down the backend, releasing all resources.
    ///
    /// This method is called when the backend is being dropped. Implementers should
    /// use this to close connections, flush caches, and perform other cleanup operations.
    ///
    /// The default implementation is a no-op, but backends with persistent storage or
    /// external connections should override this.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or a [`DocumentStoreError`](crate::error::DocumentStoreError) on failure.
    async fn shutdown(self) -> DocumentStoreResult<()>
    where
        Self: Sized,
    {
        Ok(())
    }
}

#[async_trait]
impl<B> StoreBackend for &B
where
    B: StoreBackend,
{
    async fn insert_documents(
        &self,
        documents: Vec<(Uuid, Bson)>,
        collection: &str,
    ) -> DocumentStoreResult<()> {
        (*self)
            .insert_documents(documents, collection)
            .await
    }

    async fn update_documents(
        &self,
        documents: Vec<(Uuid, Bson)>,
        collection: &str,
    ) -> DocumentStoreResult<()> {
        (*self)
            .update_documents(documents, collection)
            .await
    }

    async fn delete_documents(&self, ids: Vec<Uuid>, collection: &str) -> DocumentStoreResult<()> {
        (*self)
            .delete_documents(ids, collection)
            .await
    }

    async fn get_documents(
        &self,
        ids: Vec<Uuid>,
        collection: &str,
    ) -> DocumentStoreResult<Vec<Bson>> {
        (*self)
            .get_documents(ids, collection)
            .await
    }

    async fn query_documents(
        &self,
        query: Query,
        collection: &str,
    ) -> DocumentStoreResult<Vec<Bson>> {
        (*self)
            .query_documents(query, collection)
            .await
    }

    async fn current_revision_id(&self) -> DocumentStoreResult<Option<String>> {
        (*self).current_revision_id().await
    }

    async fn set_revision_id(&self, revision_id: &str) -> DocumentStoreResult<()> {
        (*self)
            .set_revision_id(revision_id)
            .await
    }

    async fn create_collection(&self, name: &str) -> DocumentStoreResult<()> {
        (*self).create_collection(name).await
    }

    async fn drop_collection(&self, name: &str) -> DocumentStoreResult<()> {
        (*self).drop_collection(name).await
    }

    async fn list_collections(&self) -> DocumentStoreResult<Vec<String>> {
        (*self).list_collections().await
    }

    async fn add_field(
        &self,
        collection: &str,
        field: &str,
        default: Bson,
    ) -> DocumentStoreResult<()> {
        (*self)
            .add_field(collection, field, default)
            .await
    }

    async fn drop_field(&self, collection: &str, field: &str) -> DocumentStoreResult<()> {
        (*self)
            .drop_field(collection, field)
            .await
    }

    async fn rename_field(
        &self,
        collection: &str,
        field: &str,
        new: &str,
    ) -> DocumentStoreResult<()> {
        (*self)
            .rename_field(collection, field, new)
            .await
    }

    async fn add_index(
        &self,
        collection: &str,
        field: &str,
        unique: bool,
    ) -> DocumentStoreResult<()> {
        (*self)
            .add_index(collection, field, unique)
            .await
    }

    async fn drop_index(&self, collection: &str, field: &str) -> DocumentStoreResult<()> {
        (*self)
            .drop_index(collection, field)
            .await
    }
}

#[async_trait]
impl<B> StoreBackend for &mut B
where
    B: StoreBackend,
{
    async fn insert_documents(
        &self,
        documents: Vec<(Uuid, Bson)>,
        collection: &str,
    ) -> DocumentStoreResult<()> {
        (**self)
            .insert_documents(documents, collection)
            .await
    }

    async fn update_documents(
        &self,
        documents: Vec<(Uuid, Bson)>,
        collection: &str,
    ) -> DocumentStoreResult<()> {
        (**self)
            .update_documents(documents, collection)
            .await
    }

    async fn delete_documents(&self, ids: Vec<Uuid>, collection: &str) -> DocumentStoreResult<()> {
        (**self)
            .delete_documents(ids, collection)
            .await
    }

    async fn get_documents(
        &self,
        ids: Vec<Uuid>,
        collection: &str,
    ) -> DocumentStoreResult<Vec<Bson>> {
        (**self)
            .get_documents(ids, collection)
            .await
    }

    async fn query_documents(
        &self,
        query: Query,
        collection: &str,
    ) -> DocumentStoreResult<Vec<Bson>> {
        (**self)
            .query_documents(query, collection)
            .await
    }

    async fn current_revision_id(&self) -> DocumentStoreResult<Option<String>> {
        (**self).current_revision_id().await
    }

    async fn set_revision_id(&self, revision_id: &str) -> DocumentStoreResult<()> {
        (**self)
            .set_revision_id(revision_id)
            .await
    }

    async fn create_collection(&self, name: &str) -> DocumentStoreResult<()> {
        (**self).create_collection(name).await
    }

    async fn drop_collection(&self, name: &str) -> DocumentStoreResult<()> {
        (**self).drop_collection(name).await
    }

    async fn list_collections(&self) -> DocumentStoreResult<Vec<String>> {
        (**self).list_collections().await
    }

    async fn add_field(
        &self,
        collection: &str,
        field: &str,
        default: Bson,
    ) -> DocumentStoreResult<()> {
        (**self)
            .add_field(collection, field, default)
            .await
    }

    async fn drop_field(&self, collection: &str, field: &str) -> DocumentStoreResult<()> {
        (**self)
            .drop_field(collection, field)
            .await
    }

    async fn rename_field(
        &self,
        collection: &str,
        field: &str,
        new: &str,
    ) -> DocumentStoreResult<()> {
        (**self)
            .rename_field(collection, field, new)
            .await
    }

    async fn add_index(
        &self,
        collection: &str,
        field: &str,
        unique: bool,
    ) -> DocumentStoreResult<()> {
        (**self)
            .add_index(collection, field, unique)
            .await
    }

    async fn drop_index(&self, collection: &str, field: &str) -> DocumentStoreResult<()> {
        (**self)
            .drop_index(collection, field)
            .await
    }
}

#[async_trait]
pub trait DynStoreBackend: Send + Sync + Debug {
    async fn insert_documents(
        &self,
        documents: Vec<(Uuid, Bson)>,
        collection: &str,
    ) -> DocumentStoreResult<()>;
    async fn update_documents(
        &self,
        documents: Vec<(Uuid, Bson)>,
        collection: &str,
    ) -> DocumentStoreResult<()>;
    async fn delete_documents(&self, ids: Vec<Uuid>, collection: &str) -> DocumentStoreResult<()>;
    async fn get_documents(
        &self,
        ids: Vec<Uuid>,
        collection: &str,
    ) -> DocumentStoreResult<Vec<Bson>>;
    async fn query_documents(
        &self,
        query: Query,
        collection: &str,
    ) -> DocumentStoreResult<Vec<Bson>>;
    async fn current_revision_id(&self) -> DocumentStoreResult<Option<String>>;
    async fn set_revision_id(&self, revision_id: &str) -> DocumentStoreResult<()>;
    async fn create_collection(&self, name: &str) -> DocumentStoreResult<()>;
    async fn drop_collection(&self, name: &str) -> DocumentStoreResult<()>;
    async fn list_collections(&self) -> DocumentStoreResult<Vec<String>>;
    async fn add_field(
        &self,
        collection: &str,
        field: &str,
        default: Bson,
    ) -> DocumentStoreResult<()>;
    async fn drop_field(&self, collection: &str, field: &str) -> DocumentStoreResult<()>;
    async fn rename_field(
        &self,
        collection: &str,
        field: &str,
        new: &str,
    ) -> DocumentStoreResult<()>;
    async fn add_index(
        &self,
        collection: &str,
        field: &str,
        unique: bool,
    ) -> DocumentStoreResult<()>;
    async fn drop_index(&self, collection: &str, field: &str) -> DocumentStoreResult<()>;
    async fn shutdown_boxed(self: Box<Self>) -> DocumentStoreResult<()>;

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

#[async_trait]
impl<B: StoreBackend + Send + Sync + 'static> DynStoreBackend for B {
    async fn insert_documents(
        &self,
        documents: Vec<(Uuid, Bson)>,
        collection: &str,
    ) -> DocumentStoreResult<()> {
        self.insert_documents(documents, collection)
            .await
    }

    async fn update_documents(
        &self,
        documents: Vec<(Uuid, Bson)>,
        collection: &str,
    ) -> DocumentStoreResult<()> {
        self.update_documents(documents, collection)
            .await
    }

    async fn delete_documents(&self, ids: Vec<Uuid>, collection: &str) -> DocumentStoreResult<()> {
        self.delete_documents(ids, collection)
            .await
    }

    async fn get_documents(
        &self,
        ids: Vec<Uuid>,
        collection: &str,
    ) -> DocumentStoreResult<Vec<Bson>> {
        self.get_documents(ids, collection)
            .await
    }

    async fn query_documents(
        &self,
        query: Query,
        collection: &str,
    ) -> DocumentStoreResult<Vec<Bson>> {
        self.query_documents(query, collection)
            .await
    }

    async fn current_revision_id(&self) -> DocumentStoreResult<Option<String>> {
        self.current_revision_id().await
    }

    async fn set_revision_id(&self, revision_id: &str) -> DocumentStoreResult<()> {
        self.set_revision_id(revision_id).await
    }

    async fn create_collection(&self, name: &str) -> DocumentStoreResult<()> {
        self.create_collection(name).await
    }

    async fn drop_collection(&self, name: &str) -> DocumentStoreResult<()> {
        self.drop_collection(name).await
    }

    async fn list_collections(&self) -> DocumentStoreResult<Vec<String>> {
        self.list_collections().await
    }

    async fn add_field(
        &self,
        collection: &str,
        field: &str,
        default: Bson,
    ) -> DocumentStoreResult<()> {
        self.add_field(collection, field, default)
            .await
    }

    async fn drop_field(&self, collection: &str, field: &str) -> DocumentStoreResult<()> {
        self.drop_field(collection, field).await
    }

    async fn rename_field(
        &self,
        collection: &str,
        field: &str,
        new: &str,
    ) -> DocumentStoreResult<()> {
        self.rename_field(collection, field, new)
            .await
    }

    async fn add_index(
        &self,
        collection: &str,
        field: &str,
        unique: bool,
    ) -> DocumentStoreResult<()> {
        self.add_index(collection, field, unique)
            .await
    }

    async fn drop_index(&self, collection: &str, field: &str) -> DocumentStoreResult<()> {
        self.drop_index(collection, field).await
    }

    async fn shutdown_boxed(self: Box<Self>) -> DocumentStoreResult<()> {
        self.shutdown().await
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

#[async_trait]
pub trait StoreBackendBuilder {
    type Backend: StoreBackend;

    async fn build(self) -> DocumentStoreResult<Self::Backend>;
}
