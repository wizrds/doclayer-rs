use async_trait::async_trait;
use futures::{stream::iter, StreamExt, TryStreamExt};
use bson::{Document, Bson, Uuid, doc};
use mongodb::{
    Client, Collection as MongoCollection, IndexModel,
    options::{ClientOptions, FindOptions, IndexOptions},
};
use doclayer_core::{
    backend::{StoreBackend, StoreBackendBuilder},
    error::{DocumentStoreError, DocumentStoreResult},
    query::{Query, QueryVisitor, SortDirection},
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
                    .collect::<DocumentStoreResult<Vec<Document>>>()?,
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
                .try_collect::<Vec<Document>>()
                .await
                .map_err(|e| DocumentStoreError::Backend(e.to_string()))?
                .into_iter()
                .map(|doc| self.restore_document(&doc))
                .collect::<DocumentStoreResult<Vec<Bson>>>()?
        )
    }

    async fn query_documents(&self, query: Query, collection: &str) -> DocumentStoreResult<Vec<Bson>> {
        let mut options = FindOptions::default();

        if let Some(limit) = query.limit {
            options.limit = Some(limit as i64);
        }
        if let Some(skip) = query.offset {
            options.skip = Some(skip as u64);
        }
        if let Some(sort) = &query.sort {
            options.sort = Some(doc! {
                sort.field.clone(): match sort.direction {
                    SortDirection::Asc => 1,
                    SortDirection::Desc => -1,
                }
            })
        }

        Ok(
            self.get_collection(collection)
                .find(
                    if let Some(expr) = &query.filter {
                        MongoQueryTranslator.visit_expr(expr)?
                    } else {
                        doc! {}
                    },
                )
                .with_options(options)
                .await
                .map_err(|e| DocumentStoreError::Backend(e.to_string()))?
                .try_collect::<Vec<Document>>()
                .await
                .map_err(|e| DocumentStoreError::Backend(e.to_string()))?
                .into_iter()
                .map(|doc| self.restore_document(&doc))
                .collect::<DocumentStoreResult<Vec<Bson>>>()?
        )
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