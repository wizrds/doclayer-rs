//! In-memory storage implementation for document stores.
//!
//! This module provides a simple but powerful in-memory backend that stores
//! documents as BSON values in HashMaps with async-safe read-write locks.

use std::{collections::HashMap, sync::Arc, cmp::Ordering};
use async_trait::async_trait;
use mea::rwlock::RwLock;
use bson::{Uuid, Bson};

use doclayer_core::{
    query::{Query, SortDirection},
    error::{DocumentStoreError, DocumentStoreResult},
    backend::{StoreBackend, StoreBackendBuilder},
};

use crate::evaluator::{DocumentEvaluator, Comparable};

type CollectionMap = HashMap<String, Bson>;
type StoreMap = HashMap<String, CollectionMap>;


/// Thread-safe in-memory document storage backend.
///
/// This struct implements the [`StoreBackend`] trait to provide a fully functional
/// document store that operates entirely in memory using async-aware read-write locks.
/// All documents are stored as BSON values indexed by their UUID.
///
/// # Thread Safety
///
/// `InMemoryStore` is cloneable and uses an `Arc`-wrapped internal state, allowing
/// it to be safely shared across async tasks. Multiple clones of the same instance
/// share the same underlying data.
///
/// # Performance
///
/// Queries scan all documents in a collection (no indexing). For small to medium
/// datasets (< 100k documents), this is typically acceptable. For larger datasets,
/// consider using a persistent backend like MongoDB.
///
/// # Example
///
/// ```ignore
/// use doclayer_memory::InMemoryStore;
/// use doclayer::backend::StoreBackend;
/// use bson::{Uuid, Bson, doc};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let store = InMemoryStore::new();
///     
///     // Insert documents
///     let id = Uuid::new();
///     let doc = Bson::Document(doc! { "name": "Alice", "age": 30 });
///     store.insert_documents(vec![(id, doc)], "users").await?;
///     
///     // Retrieve documents
///     let docs = store.get_documents(vec![id], "users").await?;
///     assert_eq!(docs.len(), 1);
///     
///     Ok(())
/// }
/// ```
#[derive(Default, Clone, Debug)]
pub struct InMemoryStore {
    /// The main storage map: collection_name -> (document_id -> document)
    store: Arc<RwLock<StoreMap>>,
    /// Optional current revision ID for tracking schema versions
    current_revision: Arc<RwLock<Option<String>>>,
}

impl InMemoryStore {
    /// Creates a new empty in-memory document store.
    ///
    /// The returned store is ready for use and contains no collections or documents.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use doclayer_memory::InMemoryStore;
    ///
    /// let store = InMemoryStore::new();
    /// assert!(store.list_collections().await.unwrap().is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(StoreMap::new())),
            current_revision: Arc::new(RwLock::new(None)),
        }
    }

    /// Creates a builder for constructing an `InMemoryStore` with custom options.
    ///
    /// Currently, the builder simply creates a default store, but it can be extended
    /// in future versions to support configuration options.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use doclayer_memory::InMemoryStore;
    ///
    /// let store = InMemoryStore::builder().build().await.unwrap();
    /// ```
    pub fn builder() -> InMemoryStoreBuilder {
        InMemoryStoreBuilder::default()
    }
}


#[async_trait]
impl StoreBackend for InMemoryStore {
    async fn insert_documents(&self, documents: Vec<(Uuid, Bson)>, collection: &str) -> DocumentStoreResult<()> {
        let mut store = self.store.write().await;
        let collection_map = store
            .entry(collection.to_string())
            .or_default();

        for (id, doc) in documents {
            let key = id.to_string();

            if collection_map.contains_key(&key) {
                return Err(DocumentStoreError::DocumentAlreadyExists(key, collection.to_string()));
            }

            collection_map.insert(key, doc);
        }

        Ok(())
    }

    async fn update_documents(&self, documents: Vec<(Uuid, Bson)>, collection: &str) -> DocumentStoreResult<()> {
        let mut store = self.store.write().await;
        let collection_map = match store.get_mut(collection) {
            Some(col) => col,
            None => return Err(DocumentStoreError::CollectionNotFound(collection.to_string())),
        };

        for (id, doc) in documents {
            let key = id.to_string();

            if !collection_map.contains_key(&key) {
                return Err(DocumentStoreError::DocumentNotFound(key, collection.to_string()));
            }

            collection_map.insert(key, doc);
        }

        Ok(())
    }

    async fn delete_documents(&self, ids: Vec<Uuid>, collection: &str) -> DocumentStoreResult<()> {
        let mut store = self.store.write().await;
        let collection_map = match store.get_mut(collection) {
            Some(col) => col,
            None => return Err(DocumentStoreError::CollectionNotFound(collection.to_string())),
        };

        for id in ids {
            let key = id.to_string();

            if collection_map.remove(&key).is_none() {
                return Err(DocumentStoreError::DocumentNotFound(key, collection.to_string()));
            }
        }

        Ok(())
    }

    async fn get_documents(&self, ids: Vec<Uuid>, collection: &str) -> DocumentStoreResult<Vec<Bson>> {
        let store = self.store.read().await;
        let collection_map = match store.get(collection) {
            Some(col) => col,
            None => return Ok(vec![]),
        };

        let mut documents = Vec::with_capacity(ids.len());

        for id in ids {
            let key = id.to_string();

            if let Some(doc) = collection_map.get(&key) {
                documents.push(doc.clone());
            }
        }

        Ok(documents)
    }

    async fn query_documents(&self, query: Query, collection: &str) -> DocumentStoreResult<Vec<Bson>> {
        let store = self.store.read().await;
        let collection_map = match store.get(collection) {
            Some(col) => col,
            None => return Ok(vec![]),
        };

        // Apply filter expressions if present
        let filtered_docs = match &query.filter {
            Some(filter) => DocumentEvaluator::filter_documents(
                collection_map.values(),
                filter,
            )?,
            None => collection_map
                .values()
                .cloned()
                .collect::<Vec<_>>(),
        };

        // Apply sorting if specified
        if let Some(sort) = &query.sort {
            let mut sorted_docs = filtered_docs;

            sorted_docs.sort_by(|a, b| {
                // Extract the field value and compare using Comparable wrapper
                let left = a
                    .as_document()
                    .unwrap()
                    .get(&sort.field)
                    .map(Comparable::from)
                    .unwrap_or(Comparable::Null);
                let right = b
                    .as_document()
                    .unwrap()
                    .get(&sort.field)
                    .map(Comparable::from)
                    .unwrap_or(Comparable::Null);

                match sort.direction {
                    SortDirection::Asc => left.partial_cmp(&right).unwrap_or(Ordering::Equal),
                    SortDirection::Desc => right.partial_cmp(&left).unwrap_or(Ordering::Equal),
                }
            });

            // Apply offset and limit
            return Ok(
                sorted_docs
                    .into_iter()
                    .skip(query.offset.unwrap_or(0))
                    .take(query.limit.unwrap_or(usize::MAX))
                    .collect()
            );
        }

        // Apply offset and limit without sorting
        Ok(
            filtered_docs
                .into_iter()
                .skip(query.offset.unwrap_or(0))
                .take(query.limit.unwrap_or(usize::MAX))
                .collect()
        )
    }

    async fn current_revision_id(&self) -> DocumentStoreResult<Option<String>> {
        Ok(
            self.current_revision
                .read()
                .await
                .clone()
        )
    }

    async fn set_revision_id(&self, revision_id: &str) -> DocumentStoreResult<()> {
        let mut guard = self.current_revision.write().await;
        *guard = Some(revision_id.to_string());

        Ok(())
    }

    async fn create_collection(&self, name: &str) -> DocumentStoreResult<()> {
        self.store
            .write()
            .await
            .entry(name.to_string())
            .or_insert_with(HashMap::new);

        Ok(())
    }

    async fn drop_collection(&self, name: &str) -> DocumentStoreResult<()> {
        let mut store = self.store.write().await;

        if store.remove(name).is_none() {
            return Err(DocumentStoreError::CollectionNotFound(name.to_string()));
        }

        Ok(())
    }

    async fn list_collections(&self) -> DocumentStoreResult<Vec<String>> {
        Ok(
            self.store
                .read()
                .await
                .keys()
                .cloned()
                .collect()
        )
    }

    async fn add_field(&self, collection: &str, field: &str, default: Bson) -> DocumentStoreResult<()> {
        let mut store = self.store.write().await;

        let collection_map = match store.get_mut(collection) {
            Some(col) => col,
            None => return Err(DocumentStoreError::CollectionNotFound(collection.to_string())),
        };

        // Add the field to every document in the collection
        for doc in collection_map.values_mut() {
            if let Some(doc_map) = doc.as_document_mut() {
                doc_map.insert(field.to_string(), default.clone());
            }
        }

        Ok(())
    }

    async fn drop_field(&self, collection: &str, field: &str) -> DocumentStoreResult<()> {
        let mut store = self.store.write().await;

        let collection_map = match store.get_mut(collection) {
            Some(col) => col,
            None => return Err(DocumentStoreError::CollectionNotFound(collection.to_string())),
        };

        // Remove the field from every document in the collection
        for doc in collection_map.values_mut() {
            if let Some(doc_map) = doc.as_document_mut() {
                doc_map.remove(field);
            }
        }

        Ok(())
    }

    async fn rename_field(&self, collection: &str, field: &str, new: &str) -> DocumentStoreResult<()> {
        let mut store = self.store.write().await;

        let collection_map = match store.get_mut(collection) {
            Some(col) => col,
            None => return Err(DocumentStoreError::CollectionNotFound(collection.to_string())),
        };

        // Rename the field in every document in the collection
        for doc in collection_map.values_mut() {
            if let Some(doc_map) = doc.as_document_mut() {
                if let Some(value) = doc_map.remove(field) {
                    doc_map.insert(new.to_string(), value);
                }
            }
        }

        Ok(())
    }

    async fn add_index(&self, _collection: &str, _field: &str, _unique: bool) -> DocumentStoreResult<()> {
        // In-memory store does not support indexing (no-op)
        Ok(())
    }

    async fn drop_index(&self, _collection: &str, _field: &str) -> DocumentStoreResult<()> {
        // In-memory store does not support indexing (no-op)
        Ok(())
    }
}


/// Builder for constructing [`InMemoryStore`] instances.
///
/// Currently a no-op builder, but can be extended in future versions
/// to support configuration options like capacity hints or concurrency settings.
///
/// # Example
///
/// ```ignore
/// use doclayer_memory::InMemoryStore;
/// use doclayer::backend::StoreBackendBuilder;
///
/// #[tokio::main]
/// async fn main() {
///     let store = InMemoryStore::builder().build().await.unwrap();
/// }
/// ```
#[derive(Default)]
pub struct InMemoryStoreBuilder;

#[async_trait]
impl StoreBackendBuilder for InMemoryStoreBuilder {
    type Backend = InMemoryStore;

    /// Builds and returns a new [`InMemoryStore`] instance.
    ///
    /// This always succeeds and returns a freshly initialized store.
    async fn build(self) -> DocumentStoreResult<Self::Backend> {
        Ok(InMemoryStore::new())
    }
}
