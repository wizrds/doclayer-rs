use async_trait::async_trait;
use futures::{stream::iter, StreamExt, TryStreamExt};
use bson::{Document, Bson, Uuid, doc, deserialize_from_bson};
use mongodb::{
    Client, Collection as MongoCollection, IndexModel,
    options::{ClientOptions, FindOptions, IndexOptions},
};
use doclayer_core::{
    backend::{StoreBackend, StoreBackendBuilder},
    error::{DocumentStoreError, DocumentStoreResult},
    page::{Cursor, CursorDirection, CursorPosition, Page, Pagination},
    query::{Query, QueryVisitor, Sort, SortDirection},
};

use crate::{sanitizer::ValueSanitizer, query::MongoQueryTranslator};


#[derive(Debug)]
pub struct MongoDbStore {
    client: Client,
    database: String,
}

impl MongoDbStore {
    pub fn new(client: Client, database: String) -> Self {
        Self { client, database }
    }

    pub fn builder(dsn: &str, database: &str) -> MongoDbStoreBuilder {
        MongoDbStoreBuilder::new(dsn, database)
    }

    fn get_collection(&self, collection_name: &str) -> MongoCollection<Document> {
        self.client
            .database(&self.database)
            .collection(&ValueSanitizer::sanitize_string(collection_name))
    }

    fn prepare_document(&self, id: &Uuid, document: &Bson) -> DocumentStoreResult<Document> {
        Ok(Document::from_iter(
            ValueSanitizer::sanitize_value(document)
                .as_document()
                .cloned()
                .ok_or_else(|| DocumentStoreError::InvalidDocument("Expected document".into()))?
                .into_iter()
                .chain(vec![("_id".to_string(), id.into())].into_iter()),
        ))
    }

    fn restore_document(&self, document: &Document) -> DocumentStoreResult<Bson> {
        Ok(ValueSanitizer::restore_value(&Bson::Document(
            Document::from_iter(
                document
                    .clone()
                    .into_iter()
                    .filter(|(k, _)| !["_id"].contains(&k.as_str()))
            )
        )))
    }

    /// Builds a MongoDB sort document for `sort`, always including `_id` as a
    /// secondary key so documents sharing the same primary value still sort
    /// deterministically. `reversed` flips both keys, for walking backward.
    fn sort_doc(&self, sort: &Sort, reversed: bool) -> Document {
        let primary = match (&sort.direction, reversed) {
            (SortDirection::Asc, false) => 1,
            (SortDirection::Asc, true) => -1,
            (SortDirection::Desc, false) => -1,
            (SortDirection::Desc, true) => 1,
        };

        let mut result = Document::new();
        result.insert(sort.field.clone(), primary);
        result.insert("_id", if reversed { -1 } else { 1 });

        result
    }

    /// Builds the filter clause that seeks past `position` along `field`,
    /// walking `cursor_direction` relative to `sort_direction`'s display
    /// order, with `_id` as the tiebreak when values are equal.
    fn seek_filter(
        &self,
        field: &str,
        sort_direction: &SortDirection,
        cursor_direction: CursorDirection,
        position: &CursorPosition,
    ) -> Document {
        let primary_op = match (sort_direction, cursor_direction) {
            (SortDirection::Asc, CursorDirection::Forward)
            | (SortDirection::Desc, CursorDirection::Backward) => "$gt",
            _ => "$lt",
        };
        let tie_op = match cursor_direction {
            CursorDirection::Forward => "$gt",
            CursorDirection::Backward => "$lt",
        };

        let mut primary_cmp = Document::new();
        primary_cmp.insert(primary_op, position.sort_value.clone());

        let mut tie_cmp = Document::new();
        tie_cmp.insert(tie_op, position.id);

        let mut primary_clause = Document::new();
        primary_clause.insert(field.to_string(), primary_cmp);

        let mut tie_clause = Document::new();
        tie_clause.insert(field.to_string(), position.sort_value.clone());
        tie_clause.insert("_id", tie_cmp);

        doc! { "$or": [primary_clause, tie_clause] }
    }

    /// Builds the filter clause that seeks past `position` by `_id` alone,
    /// for queries with no explicit sort.
    fn seek_filter_by_id(&self, cursor_direction: CursorDirection, position: &CursorPosition) -> Document {
        let mut cmp = Document::new();
        match cursor_direction {
            CursorDirection::Forward => cmp.insert("$gt", position.id),
            CursorDirection::Backward => cmp.insert("$lt", position.id),
        };

        doc! { "_id": cmp }
    }

    async fn shutdown(self) -> DocumentStoreResult<()> {
        self.client.shutdown().await;

        Ok(())
    }
}

#[async_trait]
impl StoreBackend for MongoDbStore {
    async fn insert_documents(&self, documents: Vec<(Uuid, Bson)>, collection: &str) -> DocumentStoreResult<()> {
        self.get_collection(collection)
            .insert_many(
                documents
                    .iter()
                    .map(|(id, doc)| self.prepare_document(id, doc))
                    .collect::<DocumentStoreResult<Vec<_>>>()?,
            )
            .await
            .map_err(|e| DocumentStoreError::Backend(e.to_string()))?;

        Ok(())
    }

    async fn update_documents(&self, documents: Vec<(Uuid, Bson)>, collection: &str) -> DocumentStoreResult<()> {
        iter(documents)
            .then(async |(id, doc)| self.get_collection(collection)
                .update_one(
                    doc! { "_id": id },
                    doc! { "$set": self.prepare_document(&id, &doc)? },
                )
                .await
                .map_err(|e| DocumentStoreError::Backend(e.to_string()))
            )
            .try_collect::<Vec<_>>()
            .await?;

        Ok(())
    }

    async fn delete_documents(&self, ids: Vec<Uuid>, collection: &str) -> DocumentStoreResult<()> {
        self.get_collection(collection)
            .delete_many(doc! { "_id": { "$in": ids } })
            .await
            .map_err(|e| DocumentStoreError::Backend(e.to_string()))?;

        Ok(())
    }

    async fn get_documents(&self, ids: Vec<Uuid>, collection: &str) -> DocumentStoreResult<Vec<Bson>> {
        Ok(
            self.get_collection(collection)
                .find(doc! { "_id": { "$in": ids } })
                .await
                .map_err(|e| DocumentStoreError::Backend(e.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|e| DocumentStoreError::Backend(e.to_string()))?
                .into_iter()
                .map(|doc| self.restore_document(&doc))
                .collect::<DocumentStoreResult<Vec<_>>>()?
        )
    }

    async fn query_documents(&self, query: Query, collection: &str) -> DocumentStoreResult<Page<Bson>> {
        let base_filter = if let Some(expr) = &query.filter {
            MongoQueryTranslator.visit_expr(expr)?
        } else {
            doc! {}
        };

        let total_count = if query.include_total_count {
            Some(
                self.get_collection(collection)
                    .count_documents(base_filter.clone())
                    .await
                    .map_err(|e| DocumentStoreError::Backend(e.to_string()))? as usize
            )
        } else {
            None
        };

        let mut options = FindOptions::default();
        let mut filter = base_filter;
        let mut reversed = false;
        let mut has_boundary = false;

        match &query.pagination {
            Pagination::None => {
                if let Some(sort) = &query.sort {
                    options.sort = Some(self.sort_doc(sort, false));
                }
            }
            Pagination::Offset { offset, limit } => {
                options.skip = Some(*offset as u64);
                options.limit = Some(*limit as i64);

                if let Some(sort) = &query.sort {
                    options.sort = Some(self.sort_doc(sort, false));
                }
            }
            Pagination::Cursor { cursor, limit, direction } => {
                reversed = matches!(direction, CursorDirection::Backward);

                options.sort = Some(match &query.sort {
                    Some(sort) => self.sort_doc(sort, reversed),
                    None => doc! { "_id": if reversed { -1 } else { 1 } },
                });

                // Over-fetch by one so we can tell whether another page
                // exists without a second round trip.
                options.limit = Some(*limit as i64 + 1);

                if let Some(cursor) = cursor {
                    has_boundary = true;

                    let position = cursor.decode()?;
                    let seek = match &query.sort {
                        Some(sort) => self.seek_filter(
                            &sort.field,
                            &sort.direction,
                            *direction,
                            &position,
                        ),
                        None => self.seek_filter_by_id(*direction, &position),
                    };

                    filter = doc! { "$and": [filter, seek] };
                }
            }
        }

        let mut documents = self.get_collection(collection)
            .find(filter)
            .with_options(options)
            .await
            .map_err(|e| DocumentStoreError::Backend(e.to_string()))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| DocumentStoreError::Backend(e.to_string()))?;

        let has_more_in_fetch_direction = if let Pagination::Cursor { limit, .. } = &query.pagination {
            let has_more = documents.len() > *limit;

            documents.truncate(*limit);

            has_more
        } else {
            false
        };

        if reversed {
            documents.reverse();
        }

        let (next_cursor, previous_cursor) = match &query.pagination {
            Pagination::Cursor { direction: CursorDirection::Forward, .. } => (
                has_more_in_fetch_direction.then(|| documents.last()).flatten(),
                has_boundary.then(|| documents.first()).flatten(),
            ),
            Pagination::Cursor { direction: CursorDirection::Backward, .. } => (
                has_boundary.then(|| documents.last()).flatten(),
                has_more_in_fetch_direction.then(|| documents.first()).flatten(),
            ),
            _ => (None, None),
        };

        let sort_field = query.sort.as_ref().map(|sort| sort.field.clone());
        let to_cursor = |doc: &Document| -> DocumentStoreResult<Cursor> {
            Cursor::encode(&CursorPosition {
                sort_value: sort_field
                    .as_ref()
                    .and_then(|field| doc.get(field))
                    .cloned()
                    .unwrap_or(Bson::Null),
                id: deserialize_from_bson(
                    doc.get("_id")
                        .cloned()
                        .ok_or_else(|| DocumentStoreError::InvalidDocument("Expected document to have an _id".into()))?,
                )
                .map_err(|e| DocumentStoreError::InvalidDocument(e.to_string()))?,
            })
        };

        let next_cursor = next_cursor.map(to_cursor).transpose()?;
        let previous_cursor = previous_cursor.map(to_cursor).transpose()?;

        Ok(Page {
            items: documents
                .into_iter()
                .map(|doc| self.restore_document(&doc))
                .collect::<DocumentStoreResult<Vec<_>>>()?,
            next_cursor,
            previous_cursor,
            total_count,
        })
    }

    async fn current_revision_id(&self) -> DocumentStoreResult<Option<String>> {
        let result = self.get_collection("_revisions")
            .find_one(doc! { "_id": 0 })
            .await
            .map_err(|e| DocumentStoreError::Backend(e.to_string()))?;

        if let Some(doc) = result {
            if let Some(Bson::String(rev_id)) = doc.get("revision_id") {
                return Ok(Some(rev_id.clone()));
            }
        }

        Ok(None)
    }

    async fn set_revision_id(&self, revision_id: &str) -> DocumentStoreResult<()> {
        self.get_collection("_revisions")
            .update_one(
                doc! { "_id": 0 },
                doc! { "$set": { "revision_id": revision_id } },
            )
            .upsert(true)
            .await
            .map_err(|e| DocumentStoreError::Backend(e.to_string()))?;

        Ok(())
    }

    async fn create_collection(&self, name: &str) -> DocumentStoreResult<()> {
        self.client
            .database(&self.database)
            .create_collection(&ValueSanitizer::sanitize_string(name))
            .await
            .map_err(|e| DocumentStoreError::Backend(e.to_string()))?;

        Ok(())
    }

    async fn drop_collection(&self, name: &str) -> DocumentStoreResult<()> {
        self.get_collection(name)
            .drop()
            .await
            .map_err(|e| DocumentStoreError::Backend(e.to_string()))?;

        Ok(())
    }

    async fn list_collections(&self) -> DocumentStoreResult<Vec<String>> {
        Ok(
            self.client
                .database(&self.database)
                .list_collection_names()
                .await
                .map_err(|e| DocumentStoreError::Backend(e.to_string()))?
                .into_iter()
                .filter(|name| name != "_revisions")
                .collect()
        )
    }

    async fn add_field(&self, collection: &str, field: &str, default: Bson) -> DocumentStoreResult<()> {
        self.get_collection(collection)
            .update_many(
                doc! { field: { "$exists": false } },
                doc! { "$set": { field: ValueSanitizer::sanitize_value(&default) } },
            )
            .await
            .map_err(|e| DocumentStoreError::Backend(e.to_string()))?;

        Ok(())
    }

    async fn drop_field(&self, collection: &str, field: &str) -> DocumentStoreResult<()> {
        self.get_collection(collection)
            .update_many(
                doc! {},
                doc! { "$unset": { field: "" } },
            )
            .await
            .map_err(|e| DocumentStoreError::Backend(e.to_string()))?;

        Ok(())
    }

    async fn rename_field(&self, collection: &str, field: &str, new: &str) -> DocumentStoreResult<()> {
        self.get_collection(collection)
            .update_many(
                doc! { field: { "$exists": true } },
                doc! { "$rename": { field: new } },
            )
            .await
            .map_err(|e| DocumentStoreError::Backend(e.to_string()))?;

        Ok(())
    }

    async fn add_index(&self, collection: &str, field: &str, unique: bool) -> DocumentStoreResult<()> {
        self.get_collection(collection)
            .create_index(
                IndexModel::builder()
                .keys(doc! { field: 1 })
                .options(
                    IndexOptions::builder()
                    .unique(unique)
                    .build()
                )
                .build()
            )
            .await
            .map_err(|e| DocumentStoreError::Backend(e.to_string()))?;

        Ok(())
    }

    async fn drop_index(&self, collection: &str, field: &str) -> DocumentStoreResult<()> {
        self.get_collection(collection)
            .drop_index(field)
            .await
            .map_err(|e| DocumentStoreError::Backend(e.to_string()))?;

        Ok(())
    }

    async fn shutdown(self) -> DocumentStoreResult<()> {
        self.shutdown().await
    }
}

pub struct MongoDbStoreBuilder {
    dsn: String,
    database: String,
}

impl MongoDbStoreBuilder {
    pub fn new(dsn: &str, database: &str) -> Self {
        Self {
            dsn: dsn.to_string(),
            database: database.to_string(),
        }
    }
}

#[async_trait]
impl StoreBackendBuilder for MongoDbStoreBuilder {
    type Backend = MongoDbStore;

    async fn build(self) -> DocumentStoreResult<Self::Backend> {
        Ok(MongoDbStore::new(
            Client::with_options(
                ClientOptions::parse(&self.dsn)
                    .await
                    .map_err(|e| DocumentStoreError::Initialization(e.to_string()))?,
            )
            .map_err(|e| DocumentStoreError::Initialization(e.to_string()))?,
            self.database,
        ))
    }
}