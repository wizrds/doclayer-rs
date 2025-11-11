//! Main document store interface for interacting with document backends.
//!
//! This module provides the primary API for working with document stores. It exposes two main store types:
//!
//! - [`DocumentStore`] - Typed store for working with a specific backend implementation
//! - [`DynDocumentStore`] - Dynamic dispatch store for runtime backend selection
//! - [`DynDocumentStoreRef`] - Reference-based store for temporary use
//!
//! Additionally, it provides conversion traits for flexible store type handling.
//!
//! # Example
//!
//! ```ignore
//! use doclayer::store::DocumentStore;
//! use doclayer::document::Document;
//!
//! let store = DocumentStore::new(backend);
//! let collection = store.typed_collection::<MyDocument>();
//! ```

use bson::Bson;

use crate::{
    backend::{DynStoreBackend, StoreBackend},
    collection::{Collection, DynCollection, DynTypedCollection, TypedCollection},
    document::Document,
    error::DocumentStoreResult,
};

/// A strongly-typed document store bound to a specific backend implementation.
///
/// This struct provides access to a document store with compile-time knowledge of the backend type.
/// It enables type-safe operations and full backend optimization.
///
/// # Type Parameters
///
/// * `B` - The backend implementation type
///
/// # Example
///
/// ```ignore
/// let store = DocumentStore::new(my_backend);
/// let users = store.typed_collection::<User>();
/// ```
#[derive(Debug)]
pub struct DocumentStore<B: StoreBackend> {
    backend: B,
}

impl<B: StoreBackend> DocumentStore<B> {
    /// Creates a new document store with the given backend.
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Gets a typed collection for the specified document type.
    ///
    /// The collection name is determined by the document type's `collection_name()` method.
    pub fn typed_collection<'a, D: Document>(&'a self) -> TypedCollection<'a, B, D> {
        TypedCollection::new(D::collection_name().to_string(), &self.backend)
    }

    /// Gets an untyped collection with the given name.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the collection
    pub fn collection<'a>(&'a self, name: &str) -> Collection<'a, B> {
        Collection::new(name.to_string(), &self.backend)
    }

    /// Creates a new collection with the given name.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the collection to create
    ///
    /// # Errors
    ///
    /// Returns an error if the collection already exists or creation fails.
    pub async fn create_collection(&self, name: &str) -> DocumentStoreResult<()> {
        self.backend
            .create_collection(name)
            .await
    }

    /// Drops (deletes) a collection with the given name.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the collection to drop
    ///
    /// # Errors
    ///
    /// Returns an error if the collection does not exist or deletion fails.
    pub async fn drop_collection(&self, name: &str) -> DocumentStoreResult<()> {
        self.backend.drop_collection(name).await
    }

    /// Lists all collections in the store.
    ///
    /// # Returns
    ///
    /// A vector of collection names.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn list_collections(&self) -> DocumentStoreResult<Vec<String>> {
        self.backend.list_collections().await
    }

    /// Adds a new field to all documents in a collection with a default value.
    ///
    /// # Arguments
    ///
    /// * `collection` - The name of the collection
    /// * `field` - The name of the field to add
    /// * `default` - The default value for the field on existing documents
    ///
    /// # Errors
    ///
    /// Returns an error if the field already exists or the operation fails.
    pub async fn add_field(
        &self,
        collection: &str,
        field: &str,
        default: impl Into<Bson>,
    ) -> DocumentStoreResult<()> {
        self.backend
            .add_field(collection, field, default.into())
            .await
    }

    /// Removes a field from all documents in a collection.
    ///
    /// # Arguments
    ///
    /// * `collection` - The name of the collection
    /// * `field` - The name of the field to drop
    ///
    /// # Errors
    ///
    /// Returns an error if the field does not exist or the operation fails.
    pub async fn drop_field(&self, collection: &str, field: &str) -> DocumentStoreResult<()> {
        self.backend
            .drop_field(collection, field)
            .await
    }

    /// Renames a field in all documents in a collection.
    ///
    /// # Arguments
    ///
    /// * `collection` - The name of the collection
    /// * `field` - The current field name
    /// * `new` - The new field name
    ///
    /// # Errors
    ///
    /// Returns an error if the field does not exist, the new name already exists, or the operation fails.
    pub async fn rename_field(
        &self,
        collection: &str,
        field: &str,
        new: &str,
    ) -> DocumentStoreResult<()> {
        self.backend
            .rename_field(collection, field, new)
            .await
    }

    /// Adds an index to a field in a collection.
    ///
    /// # Arguments
    ///
    /// * `collection` - The name of the collection
    /// * `field` - The field to index
    /// * `unique` - Whether the index should enforce uniqueness
    ///
    /// # Errors
    ///
    /// Returns an error if the index already exists or the operation fails.
    pub async fn add_index(
        &self,
        collection: &str,
        field: &str,
        unique: bool,
    ) -> DocumentStoreResult<()> {
        self.backend
            .add_index(collection, field, unique)
            .await
    }

    /// Removes an index from a field in a collection.
    ///
    /// # Arguments
    ///
    /// * `collection` - The name of the collection
    /// * `field` - The indexed field
    ///
    /// # Errors
    ///
    /// Returns an error if the index does not exist or the operation fails.
    pub async fn drop_index(&self, collection: &str, field: &str) -> DocumentStoreResult<()> {
        self.backend
            .drop_index(collection, field)
            .await
    }

    /// Shuts down the store and releases backend resources.
    ///
    /// This consumes the store and should be called when no longer needed.
    ///
    /// # Errors
    ///
    /// Returns an error if the shutdown operation fails.
    pub async fn shutdown(self) -> DocumentStoreResult<()> {
        self.backend.shutdown().await?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct DynDocumentStore {
    backend: Box<dyn DynStoreBackend>,
}

impl DynDocumentStore {
    /// Creates a new dynamic document store with the given backend trait object.
    pub fn new(backend: Box<dyn DynStoreBackend>) -> Self {
        Self { backend }
    }

    /// Gets a typed collection for the specified document type.
    pub fn typed_collection<'a, D: Document>(&'a self) -> DynTypedCollection<'a, D> {
        DynTypedCollection::new(D::collection_name().to_string(), &*self.backend)
    }

    /// Gets an untyped collection with the given name.
    pub fn collection<'a>(&'a self, name: &str) -> DynCollection<'a> {
        DynCollection::new(name.to_string(), &*self.backend)
    }

    /// Creates a new collection with the given name.
    pub async fn create_collection(&self, name: &str) -> DocumentStoreResult<()> {
        self.backend
            .create_collection(name)
            .await
    }

    /// Drops (deletes) a collection with the given name.
    pub async fn drop_collection(&self, name: &str) -> DocumentStoreResult<()> {
        self.backend.drop_collection(name).await
    }

    /// Lists all collections in the store.
    pub async fn list_collections(&self) -> DocumentStoreResult<Vec<String>> {
        self.backend.list_collections().await
    }

    /// Adds a new field to all documents in a collection with a default value.
    pub async fn add_field(
        &self,
        collection: &str,
        field: &str,
        default: impl Into<Bson>,
    ) -> DocumentStoreResult<()> {
        self.backend
            .add_field(collection, field, default.into())
            .await
    }

    /// Removes a field from all documents in a collection.
    pub async fn drop_field(&self, collection: &str, field: &str) -> DocumentStoreResult<()> {
        self.backend
            .drop_field(collection, field)
            .await
    }

    /// Renames a field in all documents in a collection.
    pub async fn rename_field(
        &self,
        collection: &str,
        field: &str,
        new: &str,
    ) -> DocumentStoreResult<()> {
        self.backend
            .rename_field(collection, field, new)
            .await
    }

    /// Adds an index to a field in a collection.
    pub async fn add_index(
        &self,
        collection: &str,
        field: &str,
        unique: bool,
    ) -> DocumentStoreResult<()> {
        self.backend
            .add_index(collection, field, unique)
            .await
    }

    /// Removes an index from a field in a collection.
    pub async fn drop_index(&self, collection: &str, field: &str) -> DocumentStoreResult<()> {
        self.backend
            .drop_index(collection, field)
            .await
    }

    /// Shuts down the store and releases backend resources.
    pub async fn shutdown(self) -> DocumentStoreResult<()> {
        self.backend.shutdown_boxed().await
    }
}

#[derive(Debug)]
pub struct DynDocumentStoreRef<'a> {
    backend: &'a dyn DynStoreBackend,
}

impl<'a> DynDocumentStoreRef<'a> {
    /// Creates a reference to a dynamic document store.
    pub fn new(backend: &'a dyn DynStoreBackend) -> Self {
        Self { backend }
    }

    /// Gets a typed collection for the specified document type.
    pub fn typed_collection<D: Document>(&'a self) -> DynTypedCollection<'a, D> {
        DynTypedCollection::new(D::collection_name().to_string(), self.backend)
    }

    /// Gets an untyped collection with the given name.
    pub fn collection(&'a self, name: &str) -> DynCollection<'a> {
        DynCollection::new(name.to_string(), self.backend)
    }

    /// Gets the current revision ID of the store.
    ///
    /// # Returns
    ///
    /// Returns `Some(id)` if a revision ID is set, or `None` otherwise.
    pub async fn current_revision_id(&self) -> DocumentStoreResult<Option<String>> {
        self.backend.current_revision_id().await
    }

    /// Sets the revision ID for the store.
    ///
    /// # Arguments
    ///
    /// * `revision_id` - The revision ID to set
    pub async fn set_revision_id(&self, revision_id: &str) -> DocumentStoreResult<()> {
        self.backend
            .set_revision_id(revision_id)
            .await
    }

    /// Creates a new collection with the given name.
    pub async fn create_collection(&self, name: &str) -> DocumentStoreResult<()> {
        self.backend
            .create_collection(name)
            .await
    }

    /// Drops (deletes) a collection with the given name.
    pub async fn drop_collection(&self, name: &str) -> DocumentStoreResult<()> {
        self.backend.drop_collection(name).await
    }

    /// Lists all collections in the store.
    pub async fn list_collections(&self) -> DocumentStoreResult<Vec<String>> {
        self.backend.list_collections().await
    }

    /// Adds a new field to all documents in a collection with a default value.
    pub async fn add_field(
        &self,
        collection: &str,
        field: &str,
        default: impl Into<Bson>,
    ) -> DocumentStoreResult<()> {
        self.backend
            .add_field(collection, field, default.into())
            .await
    }

    /// Removes a field from all documents in a collection.
    pub async fn drop_field(&self, collection: &str, field: &str) -> DocumentStoreResult<()> {
        self.backend
            .drop_field(collection, field)
            .await
    }

    /// Renames a field in all documents in a collection.
    pub async fn rename_field(
        &self,
        collection: &str,
        field: &str,
        new: &str,
    ) -> DocumentStoreResult<()> {
        self.backend
            .rename_field(collection, field, new)
            .await
    }

    /// Adds an index to a field in a collection.
    pub async fn add_index(
        &self,
        collection: &str,
        field: &str,
        unique: bool,
    ) -> DocumentStoreResult<()> {
        self.backend
            .add_index(collection, field, unique)
            .await
    }

    /// Removes an index from a field in a collection.
    pub async fn drop_index(&self, collection: &str, field: &str) -> DocumentStoreResult<()> {
        self.backend
            .drop_index(collection, field)
            .await
    }
}

/// Conversion trait for converting a document store to a dynamic reference.
///
/// This trait allows converting any store type to a [`DynDocumentStoreRef`] for runtime polymorphism.
pub trait AsDynDocumentStore {
    /// Converts this store to a dynamic reference.
    fn as_dyn<'a>(&'a self) -> DynDocumentStoreRef<'a>;
}

/// Conversion trait for converting a document store into a dynamic owned store.
///
/// This trait allows converting any store type to a [`DynDocumentStore`] for runtime polymorphism.
pub trait IntoDynDocumentStore {
    /// Converts this store into a dynamic owned store.
    fn into_dyn(self) -> DynDocumentStore;
}

impl<B: StoreBackend + 'static> AsDynDocumentStore for DocumentStore<B> {
    fn as_dyn<'a>(&'a self) -> DynDocumentStoreRef<'a> {
        DynDocumentStoreRef::new(&self.backend)
    }
}

impl<B: StoreBackend + 'static> AsDynDocumentStore for &'_ DocumentStore<B> {
    fn as_dyn<'a>(&'a self) -> DynDocumentStoreRef<'a> {
        DynDocumentStoreRef::new(&self.backend)
    }
}

impl AsDynDocumentStore for DynDocumentStore {
    fn as_dyn<'a>(&'a self) -> DynDocumentStoreRef<'a> {
        DynDocumentStoreRef::new(&*self.backend)
    }
}

impl<'a> AsDynDocumentStore for DynDocumentStoreRef<'a> {
    fn as_dyn<'b>(&'b self) -> DynDocumentStoreRef<'b> {
        DynDocumentStoreRef::new(self.backend)
    }
}

impl<B: StoreBackend + 'static> IntoDynDocumentStore for DocumentStore<B> {
    fn into_dyn(self) -> DynDocumentStore {
        DynDocumentStore::new(Box::new(self.backend))
    }
}

impl IntoDynDocumentStore for DynDocumentStore {
    fn into_dyn(self) -> DynDocumentStore {
        self
    }
}

pub trait AsStaticDocumentStore {
    fn as_static<'a, B>(&'a self) -> Option<DocumentStore<&'a B>>
    where
        B: StoreBackend + 'static;
}

pub trait IntoStaticDocumentStore {
    fn into_static<B>(self) -> Option<DocumentStore<B>>
    where
        B: StoreBackend + 'static;
}

impl AsStaticDocumentStore for DynDocumentStore {
    fn as_static<'a, B>(&'a self) -> Option<DocumentStore<&'a B>>
    where
        B: StoreBackend + 'static,
    {
        self.backend
            .as_any()
            .downcast_ref::<B>()
            .map(|b| DocumentStore::new(b))
    }
}

impl<'a> AsStaticDocumentStore for DynDocumentStoreRef<'a> {
    fn as_static<'b, B>(&'b self) -> Option<DocumentStore<&'b B>>
    where
        B: StoreBackend + 'static,
    {
        self.backend
            .as_any()
            .downcast_ref::<B>()
            .map(|b| DocumentStore::new(b))
    }
}

impl IntoStaticDocumentStore for DynDocumentStore {
    fn into_static<B>(self) -> Option<DocumentStore<B>>
    where
        B: StoreBackend + 'static,
    {
        self.backend
            .into_any()
            .downcast::<B>()
            .ok()
            .map(|b| DocumentStore::new(*b))
    }
}
