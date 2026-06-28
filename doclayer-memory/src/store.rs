//! In-memory storage implementation for document stores.
//!
//! This module provides a simple but powerful in-memory backend that stores
//! documents as BSON values in HashMaps with async-safe read-write locks.

use std::{collections::HashMap, sync::{Arc, atomic::{AtomicBool, Ordering as AtomicOrdering}}, cmp::Ordering};
use async_trait::async_trait;
use mea::rwlock::RwLock;
use bson::{Uuid, Bson};

use doclayer_core::{
    query::{Expr, Query, SortDirection},
    error::{DocumentStoreError, DocumentStoreResult},
    backend::{StoreBackend, StoreBackendBuilder},
    page::{Cursor, CursorDirection, CursorPosition, Page, Pagination},
};

use crate::{
    evaluator::{DocumentEvaluator, Comparable},
    path::BsonPath,
};

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
    /// Whether [`StoreBackend::shutdown`] has already been called.
    shut_down: Arc<AtomicBool>,
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
            shut_down: Arc::new(AtomicBool::new(false)),
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

    fn ensure_not_shut_down(&self) -> DocumentStoreResult<()> {
        if self.shut_down.load(AtomicOrdering::SeqCst) {
            return Err(DocumentStoreError::AlreadyShutDown);
        }

        Ok(())
    }

    fn apply_projection(doc: &Bson, fields: &[String]) -> Bson {
        let mut result = bson::Document::new();

        for path in fields {
            if let Some(value) = BsonPath::new(path).resolve(doc) {
                Self::insert_at_path(&mut result, path, value.clone());
            }
        }

        Bson::Document(result)
    }

    fn insert_at_path(doc: &mut bson::Document, path: &str, value: Bson) {
        let mut segments = path.splitn(2, '.');

        match (segments.next(), segments.next()) {
            (Some(key), None) => {
                doc.insert(key, value);
            }
            (Some(key), Some(rest)) => {
                let nested = doc
                    .entry(key.to_string())
                    .or_insert_with(|| Bson::Document(bson::Document::new()));

                if let Bson::Document(nested_doc) = nested {
                    Self::insert_at_path(nested_doc, rest, value);
                }
            }
            _ => {}
        }
    }
}


#[async_trait]
impl StoreBackend for InMemoryStore {
    async fn insert_documents(&self, documents: Vec<(Uuid, Bson)>, collection: &str) -> DocumentStoreResult<()> {
        self.ensure_not_shut_down()?;

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
        self.ensure_not_shut_down()?;

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

    async fn upsert_documents(&self, documents: Vec<(Uuid, Bson)>, collection: &str) -> DocumentStoreResult<()> {
        self.ensure_not_shut_down()?;

        let mut store = self.store.write().await;
        let collection_map = store
            .entry(collection.to_string())
            .or_default();

        for (id, doc) in documents {
            collection_map.insert(id.to_string(), doc);
        }

        Ok(())
    }

    async fn delete_documents(&self, ids: Vec<Uuid>, collection: &str) -> DocumentStoreResult<()> {
        self.ensure_not_shut_down()?;

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
        self.ensure_not_shut_down()?;

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

    async fn query_documents(&self, query: Query, collection: &str) -> DocumentStoreResult<Page<Bson>> {
        self.ensure_not_shut_down()?;

        let store = self.store.read().await;
        let collection_map = match store.get(collection) {
            Some(col) => col,
            None => return Ok(Page {
                items: vec![],
                next_cursor: None,
                previous_cursor: None,
                total_count: query.include_total_count.then_some(0),
            }),
        };

        // The map key is the document's id; pair it back up with each
        // document so it can be used as a sort tiebreaker below.
        let mut documents = collection_map
            .iter()
            .map(|(key, doc)| {
                key.parse::<Uuid>()
                    .map(|id| (id, doc.clone()))
                    .map_err(|e| DocumentStoreError::Backend(e.to_string()))
            })
            .collect::<DocumentStoreResult<Vec<(_, _)>>>()?;

        if let Some(filter) = &query.filter {
            documents.retain(|(_, doc)| {
                DocumentEvaluator::new(doc)
                    .evaluate(filter)
                    .unwrap_or(false)
            });
        }

        let sort_value = |doc: &Bson| -> Bson {
            match &query.sort {
                Some(sort) => doc
                    .as_document()
                    .and_then(|fields| fields.get(&sort.field))
                    .cloned()
                    .unwrap_or(Bson::Null),
                None => Bson::Null,
            }
        };

        let direction = query
            .sort
            .as_ref()
            .map(|sort| sort.direction.clone())
            .unwrap_or(SortDirection::Asc);

        // Used both to sort the full result set and to locate a cursor's
        // remembered position within it.
        let cmp_position = |a_value: &Bson, a_id: &Uuid, b_value: &Bson, b_id: &Uuid| -> Ordering {
            let primary = match direction {
                SortDirection::Asc => Comparable::from(a_value)
                    .partial_cmp(&Comparable::from(b_value))
                    .unwrap_or(Ordering::Equal),
                SortDirection::Desc => Comparable::from(b_value)
                    .partial_cmp(&Comparable::from(a_value))
                    .unwrap_or(Ordering::Equal),
            };

            primary.then_with(|| a_id.cmp(b_id))
        };

        documents.sort_by(|(a_id, a_doc), (b_id, b_doc)| {
            cmp_position(&sort_value(a_doc), a_id, &sort_value(b_doc), b_id)
        });

        let total_count = query.include_total_count.then_some(documents.len());

        let (start, end) = match &query.pagination {
            Pagination::None => (0, documents.len()),
            Pagination::Offset { offset, limit } => {
                let start = (*offset).min(documents.len());
                let end = start.saturating_add(*limit).min(documents.len());

                (start, end)
            }
            Pagination::Cursor { cursor, limit, direction: cursor_direction } => {
                let boundary = cursor
                    .as_ref()
                    .map(Cursor::decode)
                    .transpose()?;

                match cursor_direction {
                    CursorDirection::Forward => {
                        let start = match &boundary {
                            Some(position) => documents
                                .iter()
                                .position(|(id, doc)| {
                                    cmp_position(&sort_value(doc), id, &position.sort_value, &position.id) == Ordering::Greater
                                })
                                .unwrap_or(documents.len()),
                            None => 0,
                        };
                        let end = start.saturating_add(*limit).min(documents.len());

                        (start, end)
                    }
                    CursorDirection::Backward => {
                        let end = match &boundary {
                            Some(position) => documents
                                .iter()
                                .rposition(|(id, doc)| {
                                    cmp_position(&sort_value(doc), id, &position.sort_value, &position.id) == Ordering::Less
                                })
                                .map(|idx| idx + 1)
                                .unwrap_or(0),
                            None => documents.len(),
                        };
                        let start = end.saturating_sub(*limit);

                        (start, end)
                    }
                }
            }
        };

        let page = &documents[start..end];

        let next_cursor = if end < documents.len() {
            page.last()
                .map(|(id, doc)| Cursor::encode(&CursorPosition { sort_value: sort_value(doc), id: *id }))
                .transpose()?
        } else {
            None
        };

        let previous_cursor = if start > 0 {
            page.first()
                .map(|(id, doc)| Cursor::encode(&CursorPosition { sort_value: sort_value(doc), id: *id }))
                .transpose()?
        } else {
            None
        };

        Ok(Page {
            items: page
                .iter()
                .map(|(_, doc)| match &query.projection {
                    Some(fields) => Self::apply_projection(doc, fields),
                    None => doc.clone(),
                })
                .collect(),
            next_cursor,
            previous_cursor,
            total_count,
        })
    }

    async fn count_documents(
        &self,
        filter: Option<Expr>,
        collection: &str,
    ) -> DocumentStoreResult<u64> {
        self.ensure_not_shut_down()?;

        let store = self.store.read().await;
        let collection_map = match store.get(collection) {
            Some(col) => col,
            None => return Ok(0),
        };

        let count = collection_map
            .values()
            .filter(|doc| match &filter {
                Some(expr) => DocumentEvaluator::new(doc)
                    .evaluate(expr)
                    .unwrap_or(false),
                None => true,
            })
            .count();

        Ok(count as u64)
    }

    async fn current_revision_id(&self) -> DocumentStoreResult<Option<String>> {
        self.ensure_not_shut_down()?;

        Ok(
            self.current_revision
                .read()
                .await
                .clone()
        )
    }

    async fn set_revision_id(&self, revision_id: &str) -> DocumentStoreResult<()> {
        self.ensure_not_shut_down()?;

        let mut guard = self.current_revision.write().await;
        *guard = Some(revision_id.to_string());

        Ok(())
    }

    async fn create_collection(&self, name: &str) -> DocumentStoreResult<()> {
        self.ensure_not_shut_down()?;

        self.store
            .write()
            .await
            .entry(name.to_string())
            .or_insert_with(HashMap::new);

        Ok(())
    }

    async fn drop_collection(&self, name: &str) -> DocumentStoreResult<()> {
        self.ensure_not_shut_down()?;

        let mut store = self.store.write().await;

        if store.remove(name).is_none() {
            return Err(DocumentStoreError::CollectionNotFound(name.to_string()));
        }

        Ok(())
    }

    async fn list_collections(&self) -> DocumentStoreResult<Vec<String>> {
        self.ensure_not_shut_down()?;

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
        self.ensure_not_shut_down()?;

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
        self.ensure_not_shut_down()?;

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
        self.ensure_not_shut_down()?;

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
        self.ensure_not_shut_down()?;

        // In-memory store does not support indexing (no-op)
        Ok(())
    }

    async fn drop_index(&self, _collection: &str, _field: &str) -> DocumentStoreResult<()> {
        self.ensure_not_shut_down()?;

        // In-memory store does not support indexing (no-op)
        Ok(())
    }

    async fn shutdown(&self) -> DocumentStoreResult<()> {
        self.shut_down.store(true, AtomicOrdering::SeqCst);

        Ok(())
    }

    fn is_shut_down(&self) -> bool {
        self.shut_down.load(AtomicOrdering::SeqCst)
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

#[cfg(test)]
mod tests {
    use bson::{Bson, Uuid, doc};

    use doclayer_core::{
        backend::StoreBackend,
        query::{Expr, FieldOp, Query},
    };

    use super::InMemoryStore;

    async fn store_with_docs() -> InMemoryStore {
        let store = InMemoryStore::new();

        store
            .insert_documents(
                vec![
                    (Uuid::new(), Bson::Document(doc! { "name": "Alice", "age": 30, "address": { "city": "Denver", "zip": "80201" } })),
                    (Uuid::new(), Bson::Document(doc! { "name": "Bob", "age": 25, "address": { "city": "Austin", "zip": "73301" } })),
                    (Uuid::new(), Bson::Document(doc! { "name": "Carol", "age": 35, "address": { "city": "Denver", "zip": "80202" } })),
                ],
                "users",
            )
            .await
            .unwrap();

        store
    }

    #[tokio::test]
    async fn count_all_documents_returns_total() {
        let store = store_with_docs().await;
        assert_eq!(store.count_documents(None, "users").await.unwrap(), 3);
    }

    #[tokio::test]
    async fn count_with_filter_returns_matching_count() {
        let store = store_with_docs().await;

        let count = store
            .count_documents(
                Some(Expr::Field {
                    field: "address.city".to_string(),
                    op: FieldOp::Eq,
                    value: "Denver".into(),
                }),
                "users",
            )
            .await
            .unwrap();

        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn count_missing_collection_returns_zero() {
        let store = InMemoryStore::new();
        assert_eq!(store.count_documents(None, "users").await.unwrap(), 0);
    }

    #[tokio::test]
    async fn projection_returns_only_requested_fields() {
        let store = store_with_docs().await;

        let page = store
            .query_documents(Query::new().project(["name"]), "users")
            .await
            .unwrap();

        for item in &page.items {
            let doc = item.as_document().unwrap();
            assert!(doc.contains_key("name"), "should have 'name'");
            assert!(!doc.contains_key("age"), "should not have 'age'");
            assert!(!doc.contains_key("address"), "should not have 'address'");
        }
    }

    #[tokio::test]
    async fn projection_on_nested_field_returns_reconstructed_document() {
        let store = store_with_docs().await;

        let page = store
            .query_documents(Query::new().project(["address.city"]), "users")
            .await
            .unwrap();

        for item in &page.items {
            let doc = item.as_document().unwrap();
            let address = doc.get_document("address").unwrap();
            assert!(address.contains_key("city"), "should have 'address.city'");
            assert!(!address.contains_key("zip"), "should not have 'address.zip'");
        }
    }

    #[tokio::test]
    async fn projection_missing_field_omits_it_silently() {
        let store = store_with_docs().await;

        let page = store
            .query_documents(Query::new().project(["nonexistent"]), "users")
            .await
            .unwrap();

        for item in &page.items {
            assert!(item.as_document().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn no_projection_returns_full_documents() {
        let store = store_with_docs().await;

        let page = store
            .query_documents(Query::new(), "users")
            .await
            .unwrap();

        for item in &page.items {
            let doc = item.as_document().unwrap();
            assert!(doc.contains_key("name"));
            assert!(doc.contains_key("age"));
            assert!(doc.contains_key("address"));
        }
    }
}
