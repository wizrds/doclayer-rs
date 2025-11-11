//! Collection types for document store operations.
//!
//! This module provides collection abstractions that enable working with documents
//! in a specific collection. It offers both typed collections (with full type safety)
//! and dynamic collections (for working with dynamically dispatched backends).
//!
//! # Collection Types
//!
//! - [`Collection`] - Untyped collection with explicit BSON documents
//! - [`TypedCollection`] - Type-safe collection for a specific document type
//! - [`DynCollection`] - Dynamic dispatch version of untyped collection
//! - [`DynTypedCollection`] - Dynamic dispatch version of typed collection
//!
//! # Example
//!
//! ```ignore
//! use doclayer::document::Document;
//! use serde::{Serialize, Deserialize};
//! use bson::Uuid;
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
//! # async fn example(store: &doclayer::store::DocumentStore<impl doclayer::backend::StoreBackend>) -> doclayer::error::DocumentStoreResult<()> {
//! // Get a typed collection
//! let users = store.typed_collection::<User>();
//! let user = User { id: Uuid::new(), name: "Alice".to_string() };
//! users.insert(vec![user]).await?;
//! # Ok(()) }
//! ```

use bson::{Bson, Uuid};
use std::marker::PhantomData;

use crate::{
    backend::{DynStoreBackend, StoreBackend},
    document::{Document, DocumentExt},
    error::DocumentStoreResult,
    query::Query,
};

/// An untyped collection with a reference to a storage backend.
///
/// This struct provides access to a collection with explicit BSON document handling.
/// All documents are represented as BSON values, providing maximum flexibility
/// but without compile-time type safety.
///
/// # Type Parameters
///
/// * `'a` - Lifetime of the backend reference
/// * `B` - The storage backend type
#[derive(Debug)]
pub struct Collection<'a, B: StoreBackend> {
    name: String,
    backend: &'a B,
}

impl<'a, B: StoreBackend> Collection<'a, B> {
    /// Creates a new collection reference (internal use).
    pub(crate) fn new(name: String, backend: &'a B) -> Self {
        Self { name, backend }
    }

    /// Returns the name of this collection.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Inserts new documents into the collection, overwriting existing documents with the same IDs.
    ///
    /// # Arguments
    ///
    /// * `documents` - A vector of (ID, BSON document) pairs to insert
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if the operation fails.
    pub async fn insert(&self, documents: Vec<(Uuid, Bson)>) -> DocumentStoreResult<()> {
        Ok(self
            .backend
            .insert_documents(documents, &self.name())
            .await?)
    }

    /// Updates existing documents in the collection.
    ///
    /// # Arguments
    ///
    /// * `documents` - A vector of (ID, BSON document) pairs with updated content
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if the operation fails.
    pub async fn update(&self, documents: Vec<(Uuid, Bson)>) -> DocumentStoreResult<()> {
        Ok(self
            .backend
            .update_documents(documents, &self.name())
            .await?)
    }

    /// Deletes documents from the collection by their IDs.
    ///
    /// # Arguments
    ///
    /// * `ids` - A vector of document IDs to delete (must implement `Into<Uuid>`)
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if the operation fails.
    pub async fn delete<U>(&self, ids: Vec<U>) -> DocumentStoreResult<()>
    where
        U: Into<Uuid> + Send + Sync + 'static,
    {
        Ok(self
            .backend
            .delete_documents(
                ids.into_iter()
                    .map(Into::into)
                    .collect(),
                &self.name(),
            )
            .await?)
    }

    /// Retrieves documents from the collection by their IDs.
    ///
    /// # Arguments
    ///
    /// * `ids` - A vector of document IDs to retrieve (must implement `Into<Uuid>`)
    ///
    /// # Returns
    ///
    /// A vector of BSON documents found. If a document ID doesn't exist, it is omitted from results.
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if the operation fails.
    pub async fn get<U>(&self, ids: Vec<U>) -> DocumentStoreResult<Vec<Bson>>
    where
        U: Into<Uuid> + Send + Sync + 'static,
    {
        Ok(self
            .backend
            .get_documents(
                ids.into_iter()
                    .map(Into::into)
                    .collect(),
                &self.name(),
            )
            .await?)
    }

    /// Queries documents in the collection using a structured query.
    ///
    /// # Arguments
    ///
    /// * `query` - The [`Query`] specifying filters, sorting, limits, and offsets
    ///
    /// # Returns
    ///
    /// A vector of BSON documents matching the query criteria.
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if the operation fails.
    pub async fn query(&self, query: Query) -> DocumentStoreResult<Vec<Bson>> {
        Ok(self
            .backend
            .query_documents(query, &self.name())
            .await?)
    }
}

/// A dynamic (type-erased) collection with a reference to a backend trait object.
///
/// This struct provides access to a collection with explicit BSON document handling,
/// similar to [`Collection`], but uses dynamic dispatch via trait objects for backend operations.
/// This enables using different backend implementations at runtime without generic type parameters.
///
/// # Type Parameters
///
/// * `'a` - Lifetime of the backend trait object reference
#[derive(Debug)]
pub struct DynCollection<'a> {
    name: String,
    backend: &'a dyn DynStoreBackend,
}

impl<'a> DynCollection<'a> {
    /// Creates a new dynamic collection reference (internal use).
    pub(crate) fn new(name: String, backend: &'a dyn DynStoreBackend) -> Self {
        Self { name, backend }
    }

    /// Returns the name of this collection.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Inserts new documents into the collection, overwriting existing documents with the same IDs.
    ///
    /// # Arguments
    ///
    /// * `documents` - A vector of (ID, BSON document) pairs to insert
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if the operation fails.
    pub async fn insert(&self, documents: Vec<(Uuid, Bson)>) -> DocumentStoreResult<()> {
        Ok(self
            .backend
            .insert_documents(documents, &self.name())
            .await?)
    }

    /// Updates existing documents in the collection.
    ///
    /// # Arguments
    ///
    /// * `documents` - A vector of (ID, BSON document) pairs with updated content
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if the operation fails.
    pub async fn update(&self, documents: Vec<(Uuid, Bson)>) -> DocumentStoreResult<()> {
        Ok(self
            .backend
            .update_documents(documents, &self.name())
            .await?)
    }

    /// Deletes documents from the collection by their IDs.
    ///
    /// # Arguments
    ///
    /// * `ids` - A vector of document IDs to delete (must implement `Into<Uuid>`)
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if the operation fails.
    pub async fn delete<U>(&self, ids: Vec<U>) -> DocumentStoreResult<()>
    where
        U: Into<Uuid> + Send + Sync + 'static,
    {
        Ok(self
            .backend
            .delete_documents(
                ids.into_iter()
                    .map(Into::into)
                    .collect(),
                &self.name(),
            )
            .await?)
    }

    /// Retrieves documents from the collection by their IDs.
    ///
    /// # Arguments
    ///
    /// * `ids` - A vector of document IDs to retrieve (must implement `Into<Uuid>`)
    ///
    /// # Returns
    ///
    /// A vector of BSON documents found. If a document ID doesn't exist, it is omitted from results.
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if the operation fails.
    pub async fn get<U>(&self, ids: Vec<U>) -> DocumentStoreResult<Vec<Bson>>
    where
        U: Into<Uuid> + Send + Sync + 'static,
    {
        Ok(self
            .backend
            .get_documents(
                ids.into_iter()
                    .map(Into::into)
                    .collect(),
                &self.name(),
            )
            .await?)
    }

    /// Queries documents in the collection using a structured query.
    ///
    /// # Arguments
    ///
    /// * `query` - The [`Query`] specifying filters, sorting, limits, and offsets
    ///
    /// # Returns
    ///
    /// A vector of BSON documents matching the query criteria.
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if the operation fails.
    pub async fn query(&self, query: Query) -> DocumentStoreResult<Vec<Bson>> {
        Ok(self
            .backend
            .query_documents(query, &self.name())
            .await?)
    }
}

#[derive(Debug)]
pub struct TypedCollection<'a, B: StoreBackend, D: Document> {
    name: String,
    backend: &'a B,
    _marker: PhantomData<D>,
}

impl<'a, B: StoreBackend, D: Document> TypedCollection<'a, B, D> {
    pub(crate) fn new(name: String, backend: &'a B) -> Self {
        Self { name, backend, _marker: PhantomData }
    }

    /// Returns the name of this collection.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Converts this typed collection to a different document type.
    ///
    /// This method allows switching between different document types for the same collection.
    pub fn with_type<T: Document>(&self) -> TypedCollection<'a, B, T> {
        TypedCollection {
            name: self.name.clone(),
            backend: self.backend,
            _marker: PhantomData,
        }
    }

    /// Inserts new documents into the collection, overwriting existing documents with the same IDs.
    ///
    /// # Arguments
    ///
    /// * `documents` - A vector of documents to insert
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if serialization or insertion fails.
    pub async fn insert(&self, documents: Vec<D>) -> DocumentStoreResult<()> {
        Ok(self
            .backend
            .insert_documents(
                documents
                    .into_iter()
                    .map(|d| {
                        d.to_bson()
                            .map(move |b| (d.id().clone(), b))
                    })
                    .collect::<Result<Vec<(Uuid, Bson)>, _>>()?,
                &self.name(),
            )
            .await?)
    }

    /// Updates existing documents in the collection.
    ///
    /// # Arguments
    ///
    /// * `documents` - A vector of documents with updated content
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if serialization or update fails.
    pub async fn update(&self, documents: Vec<D>) -> DocumentStoreResult<()> {
        Ok(self
            .backend
            .update_documents(
                documents
                    .into_iter()
                    .map(|d| {
                        d.to_bson()
                            .map(move |b| (d.id().clone(), b))
                    })
                    .collect::<Result<Vec<(Uuid, Bson)>, _>>()?,
                &self.name(),
            )
            .await?)
    }

    /// Deletes documents from the collection by their IDs.
    ///
    /// # Arguments
    ///
    /// * `ids` - A vector of document IDs to delete (must implement `Into<Uuid>`)
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if the operation fails.
    pub async fn delete<U>(&self, ids: Vec<U>) -> DocumentStoreResult<()>
    where
        U: Into<Uuid> + Send + Sync + 'static,
    {
        Ok(self
            .backend
            .delete_documents(
                ids.into_iter()
                    .map(Into::into)
                    .collect(),
                &self.name(),
            )
            .await?)
    }

    /// Retrieves documents from the collection by their IDs.
    ///
    /// # Arguments
    ///
    /// * `ids` - A vector of document IDs to retrieve (must implement `Into<Uuid>`)
    ///
    /// # Returns
    ///
    /// A vector of documents found. If a document ID doesn't exist, it is omitted from results.
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if deserialization or retrieval fails.
    pub async fn get<U>(&self, ids: Vec<U>) -> DocumentStoreResult<Vec<D>>
    where
        U: Into<Uuid> + Send + Sync + 'static,
    {
        Ok(self
            .backend
            .get_documents(
                ids.into_iter()
                    .map(Into::into)
                    .collect(),
                &self.name(),
            )
            .await?
            .into_iter()
            .map(|doc| D::from_bson(doc))
            .collect::<Result<Vec<D>, _>>()?)
    }

    /// Queries documents in the collection using a structured query.
    ///
    /// # Arguments
    ///
    /// * `query` - The [`Query`] specifying filters, sorting, limits, and offsets
    ///
    /// # Returns
    ///
    /// A vector of documents matching the query criteria.
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if deserialization or query fails.
    pub async fn query(&self, query: Query) -> DocumentStoreResult<Vec<D>> {
        Ok(self
            .backend
            .query_documents(query, &self.name())
            .await?
            .into_iter()
            .map(|doc| D::from_bson(doc))
            .collect::<Result<Vec<D>, _>>()?)
    }
}

#[derive(Debug)]
pub struct DynTypedCollection<'a, D: Document> {
    name: String,
    backend: &'a dyn DynStoreBackend,
    _marker: PhantomData<D>,
}

impl<'a, D: Document> DynTypedCollection<'a, D> {
    pub(crate) fn new(name: String, backend: &'a dyn DynStoreBackend) -> Self {
        Self { name, backend, _marker: PhantomData }
    }

    /// Returns the name of this collection.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Converts this typed collection to a different document type.
    ///
    /// This method allows switching between different document types for the same collection.
    pub fn with_type<T: Document>(&self) -> DynTypedCollection<'a, T> {
        DynTypedCollection {
            name: self.name.clone(),
            backend: self.backend,
            _marker: PhantomData,
        }
    }

    /// Inserts new documents into the collection, overwriting existing documents with the same IDs.
    ///
    /// # Arguments
    ///
    /// * `documents` - A vector of documents to insert
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if serialization or insertion fails.
    pub async fn insert(&self, documents: Vec<D>) -> DocumentStoreResult<()> {
        Ok(self
            .backend
            .insert_documents(
                documents
                    .into_iter()
                    .map(|d| {
                        d.to_bson()
                            .map(move |b| (d.id().clone(), b))
                    })
                    .collect::<Result<Vec<(Uuid, Bson)>, _>>()?,
                &self.name(),
            )
            .await?)
    }

    /// Updates existing documents in the collection.
    ///
    /// # Arguments
    ///
    /// * `documents` - A vector of documents with updated content
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if serialization or update fails.
    pub async fn update(&self, documents: Vec<D>) -> DocumentStoreResult<()> {
        Ok(self
            .backend
            .update_documents(
                documents
                    .into_iter()
                    .map(|d| {
                        d.to_bson()
                            .map(move |b| (d.id().clone(), b))
                    })
                    .collect::<Result<Vec<(Uuid, Bson)>, _>>()?,
                &self.name(),
            )
            .await?)
    }

    /// Deletes documents from the collection by their IDs.
    ///
    /// # Arguments
    ///
    /// * `ids` - A vector of document IDs to delete (must implement `Into<Uuid>`)
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if the operation fails.
    pub async fn delete<U>(&self, ids: Vec<U>) -> DocumentStoreResult<()>
    where
        U: Into<Uuid> + Send + Sync + 'static,
    {
        Ok(self
            .backend
            .delete_documents(
                ids.into_iter()
                    .map(Into::into)
                    .collect(),
                &self.name(),
            )
            .await?)
    }

    /// Retrieves documents from the collection by their IDs.
    ///
    /// # Arguments
    ///
    /// * `ids` - A vector of document IDs to retrieve (must implement `Into<Uuid>`)
    ///
    /// # Returns
    ///
    /// A vector of documents found. If a document ID doesn't exist, it is omitted from results.
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if deserialization or retrieval fails.
    pub async fn get<U>(&self, ids: Vec<U>) -> DocumentStoreResult<Vec<D>>
    where
        U: Into<Uuid> + Send + Sync + 'static,
    {
        Ok(self
            .backend
            .get_documents(
                ids.into_iter()
                    .map(Into::into)
                    .collect(),
                &self.name(),
            )
            .await?
            .into_iter()
            .map(|doc| D::from_bson(doc))
            .collect::<Result<Vec<D>, _>>()?)
    }

    /// Queries documents in the collection using a structured query.
    ///
    /// # Arguments
    ///
    /// * `query` - The [`Query`] specifying filters, sorting, limits, and offsets
    ///
    /// # Returns
    ///
    /// A vector of documents matching the query criteria.
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`](crate::error::DocumentStoreError) if deserialization or query fails.
    pub async fn query(&self, query: Query) -> DocumentStoreResult<Vec<D>> {
        Ok(self
            .backend
            .query_documents(query, &self.name())
            .await?
            .into_iter()
            .map(|doc| D::from_bson(doc))
            .collect::<Result<Vec<D>, _>>()?)
    }
}
