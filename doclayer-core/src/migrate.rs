//! Schema migration framework for document stores.
//!
//! This module provides tools for versioning and migrating document schemas across different versions.
//! It supports bidirectional migrations (upgrade and downgrade) using a directed graph of revisions.
//!
//! # Migration Traits
//!
//! - [`Migration`] - Individual migration step (upgrade/downgrade)
//! - [`Migrations`] - Registry of all available migrations
//! - [`Migrator`] - Auto-implemented trait for running migrations
//!
//! # Example
//!
//! ```ignore
//! use doclayer::migrate::{Migration, Migrations, MigrateOp, MigrationRef};
//! use doclayer::error::DocumentStoreResult;
//!
//! struct MyMigration;
//!
//! #[async_trait::async_trait]
//! impl Migration for MyMigration {
//!     fn id(&self) -> &'static str { "001_initial" }
//!     fn previous_id(&self) -> Option<&'static str> { None }
//!
//!     async fn up(&self, op: &MigrateOp<'_>) -> DocumentStoreResult<()> {
//!         op.create_collection("users").await?;
//!         op.add_field("users", "name", bson::Bson::Null).await?;
//!         Ok(())
//!     }
//!
//!     async fn down(&self, op: &MigrateOp<'_>) -> DocumentStoreResult<()> {
//!         op.drop_collection("users").await?;
//!         Ok(())
//!     }
//! }
//!
//! struct MyMigrations;
//!
//! impl Migrations for MyMigrations {
//!    fn migrations() -> Vec<MigrationRef> {
//!         vec![
//!             Box::new(MyMigration)
//!         ]
//!    }
//! }
//! ```
//!
//! Usage with the document store:
//! ```ignore
//! use doclayer::store::DocumentStore;
//! use doclayer::migrate::Migrator;
//!
//! let store: DocumentStore = /* initialize your document store */;
//! store.upgrade::<MyMigrations>().await?;
//!
//! store.downgrade_to::<MyMigrations>("001_initial").await?;
//! ```

use async_trait::async_trait;
use bson::{Bson, Uuid};
use std::{
    collections::{HashMap, HashSet},
    marker::PhantomData,
};

use crate::{
    document::Document,
    error::{DocumentStoreError, DocumentStoreResult},
    query::Query,
    store::{AsDynDocumentStore, DynDocumentStoreRef},
};

/// Direction of schema migration (upgrade or downgrade to different version).
pub enum MigrationDirection {
    /// Upgrade to a newer schema version.
    Up,
    /// Downgrade to an older schema version.
    Down,
}

/// A single migration step in the schema evolution chain.
///
/// Implementations define how to upgrade and downgrade between two schema versions.
/// Each migration must have a unique ID and optionally specify the previous migration ID
/// to form the directed graph of migrations.
#[async_trait]
pub trait Migration: Send + Sync {
    /// Returns a unique identifier for this migration.
    fn id(&self) -> &'static str;

    /// Returns the ID of the migration this one follows (for ordering).
    /// Should return `None` for the initial migration.
    fn previous_id(&self) -> Option<&'static str>;

    /// Executes this migration in the upgrade direction.
    ///
    /// # Arguments
    ///
    /// * `op` - Operation context providing access to the document store for this migration
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`] if migration fails.
    async fn up(&self, op: &MigrateOp<'_>) -> DocumentStoreResult<()>;

    /// Executes this migration in the downgrade direction (reverses the changes from `up`).
    ///
    /// # Arguments
    ///
    /// * `op` - Operation context providing access to the document store for this migration
    ///
    /// # Errors
    ///
    /// Returns a [`DocumentStoreError`] if migration fails.
    async fn down(&self, op: &MigrateOp<'_>) -> DocumentStoreResult<()>;
}

pub type MigrationRef = Box<dyn Migration>;

pub trait Migrations: Send + Sync {
    fn migrations() -> Vec<MigrationRef>;
}

pub struct MigrateOp<'a> {
    store: &'a DynDocumentStoreRef<'a>,
}

impl<'a> MigrateOp<'a> {
    pub fn new(store: &'a DynDocumentStoreRef<'a>) -> Self {
        Self { store }
    }

    pub async fn create_collection(&self, name: &str) -> DocumentStoreResult<()> {
        self.store.create_collection(name).await
    }

    pub async fn drop_collection(&self, name: &str) -> DocumentStoreResult<()> {
        self.store.drop_collection(name).await
    }

    pub async fn list_collections(&self) -> DocumentStoreResult<Vec<String>> {
        self.store.list_collections().await
    }

    pub async fn add_field(
        &self,
        collection: &str,
        field: &str,
        default: impl Into<bson::Bson>,
    ) -> DocumentStoreResult<()> {
        self.store
            .add_field(collection, field, default.into())
            .await
    }

    pub async fn drop_field(&self, collection: &str, field: &str) -> DocumentStoreResult<()> {
        self.store
            .drop_field(collection, field)
            .await
    }

    pub async fn rename_field(
        &self,
        collection: &str,
        field: &str,
        new: &str,
    ) -> DocumentStoreResult<()> {
        self.store
            .rename_field(collection, field, new)
            .await
    }

    pub async fn add_index(
        &self,
        collection: &str,
        field: &str,
        unique: bool,
    ) -> DocumentStoreResult<()> {
        self.store
            .add_index(collection, field, unique)
            .await
    }

    pub async fn drop_index(&self, collection: &str, field: &str) -> DocumentStoreResult<()> {
        self.store
            .drop_index(collection, field)
            .await
    }

    pub async fn insert_typed<D: Document>(&self, docs: Vec<D>) -> DocumentStoreResult<()> {
        self.store
            .typed_collection::<D>()
            .insert(docs)
            .await
    }

    pub async fn update_typed<D: Document>(&self, docs: Vec<D>) -> DocumentStoreResult<()> {
        self.store
            .typed_collection::<D>()
            .update(docs)
            .await
    }

    pub async fn delete_typed<U, D>(&self, ids: Vec<U>) -> DocumentStoreResult<()>
    where
        U: Into<Uuid> + Send + Sync + 'static,
        D: Document,
    {
        self.store
            .typed_collection::<D>()
            .delete(ids)
            .await
    }

    pub async fn get_typed<U, D>(&self, ids: Vec<U>) -> DocumentStoreResult<Vec<D>>
    where
        U: Into<Uuid> + Send + Sync + 'static,
        D: Document,
    {
        self.store
            .typed_collection::<D>()
            .get(ids)
            .await
    }

    pub async fn query_typed<D: Document>(&self, query: Query) -> DocumentStoreResult<Vec<D>> {
        self.store
            .typed_collection::<D>()
            .query(query)
            .await
    }

    pub async fn insert(
        &self,
        collection: &str,
        docs: Vec<(Uuid, Bson)>,
    ) -> DocumentStoreResult<()> {
        self.store
            .collection(collection)
            .insert(docs)
            .await
    }

    pub async fn update(
        &self,
        collection: &str,
        docs: Vec<(Uuid, Bson)>,
    ) -> DocumentStoreResult<()> {
        self.store
            .collection(collection)
            .update(docs)
            .await
    }

    pub async fn delete<U>(&self, collection: &str, ids: Vec<U>) -> DocumentStoreResult<()>
    where
        U: Into<Uuid> + Send + Sync + 'static,
    {
        self.store
            .collection(collection)
            .delete(ids)
            .await
    }

    pub async fn get<U>(&self, collection: &str, ids: Vec<U>) -> DocumentStoreResult<Vec<Bson>>
    where
        U: Into<Uuid> + Send + Sync + 'static,
    {
        self.store
            .collection(collection)
            .get(ids)
            .await
    }

    pub async fn query(&self, collection: &str, query: Query) -> DocumentStoreResult<Vec<Bson>> {
        self.store
            .collection(collection)
            .query(query)
            .await
    }
}

struct RevisionGraph {
    children: HashMap<String, Vec<String>>,
    parents: HashMap<String, String>,
}

impl RevisionGraph {
    fn new(migrations: &[&MigrationRef]) -> Self {
        let (children, parents) = migrations.iter().fold(
            (HashMap::new(), HashMap::new()),
            |(mut children, mut parents), migration| {
                migration.previous_id().map(|prev_id| {
                    children
                        .entry(prev_id.to_string())
                        .or_insert_with(Vec::new)
                        .push(migration.id().to_string());

                    parents.insert(migration.id().to_string(), prev_id.to_string());
                });

                (children, parents)
            },
        );

        Self { children, parents }
    }

    fn find_path<F>(&self, from: &str, to: &str, next: F) -> Option<Vec<String>>
    where
        F: Fn(&RevisionGraph, &str) -> Vec<String>,
    {
        if from == to {
            return Some(vec![from.to_string()]);
        }

        let mut visited = HashSet::new();
        let mut queue = vec![(from.to_string(), vec![from.to_string()])];

        while let Some((cur, path)) = queue.pop() {
            if !visited.insert(cur.clone()) {
                continue;
            }

            for neighbor in next(self, &cur) {
                if neighbor == to {
                    return Some([path.clone(), vec![to.to_string()]].concat());
                }

                queue.push((neighbor.clone(), [path.clone(), vec![neighbor]].concat()));
            }
        }

        None
    }

    fn find_up_path(&self, from: &str, to: &str) -> Option<Vec<String>> {
        self.find_path(from, to, |graph, node| {
            graph
                .children
                .get(node)
                .cloned()
                .unwrap_or_default()
        })
    }

    fn find_down_path(&self, from: &str, to: &str) -> Option<Vec<String>> {
        self.find_path(from, to, |graph, node| {
            graph
                .parents
                .get(node)
                .map(|parent| vec![parent.clone()])
                .unwrap_or_default()
        })
    }
}

struct RevisionChain {
    revisions: HashMap<String, MigrationRef>,
    graph: RevisionGraph,
    head: Option<String>,
    tail: Option<String>,
}

impl RevisionChain {
    fn new(migrations: Vec<MigrationRef>) -> Self {
        let revisions = migrations
            .into_iter()
            .map(|migration| (migration.id().to_string(), migration))
            .collect::<HashMap<_, _>>();

        let graph = RevisionGraph::new(&revisions.values().collect::<Vec<_>>());

        let head = revisions
            .keys()
            .find(|id| !graph.children.contains_key(*id))
            .cloned();

        let tail = revisions
            .keys()
            .find(|id| !graph.parents.contains_key(*id))
            .cloned();

        Self { revisions, graph, head, tail }
    }

    fn get(&self, id: &str) -> Option<&MigrationRef> {
        self.revisions.get(id)
    }

    fn head(&self) -> Option<&str> {
        self.head.as_deref()
    }

    fn tail(&self) -> Option<&str> {
        self.tail.as_deref()
    }

    fn get_upgrade_path(&self, from: &str, to: &str) -> Option<Vec<&MigrationRef>> {
        self.graph
            .find_up_path(from, to)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.get(id))
                    .collect()
            })
    }

    fn get_downgrade_path(&self, from: &str, to: &str) -> Option<Vec<&MigrationRef>> {
        self.graph
            .find_down_path(from, to)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.get(id))
                    .collect()
            })
    }
}

pub struct MigrationRunner<M: Migrations> {
    chain: RevisionChain,
    _marker: PhantomData<M>,
}

impl<M: Migrations> MigrationRunner<M> {
    pub fn new() -> Self {
        Self {
            chain: RevisionChain::new(M::migrations()),
            _marker: PhantomData,
        }
    }

    pub async fn upgrade<'a>(&self, store: DynDocumentStoreRef<'a>) -> DocumentStoreResult<()> {
        self.upgrade_to(
            store,
            self.chain
                .head()
                .ok_or(DocumentStoreError::Migration(
                    "No head revision found for upgrade".to_string(),
                ))?,
        )
        .await
    }

    pub async fn downgrade<'a>(&self, store: DynDocumentStoreRef<'a>) -> DocumentStoreResult<()> {
        self.downgrade_to(
            store,
            self.chain
                .tail()
                .ok_or(DocumentStoreError::Migration(
                    "No tail revision found for downgrade".to_string(),
                ))?,
        )
        .await
    }

    pub async fn upgrade_to<'a>(
        &self,
        store: DynDocumentStoreRef<'a>,
        target_revision: &str,
    ) -> DocumentStoreResult<()> {
        self.apply(store, target_revision, MigrationDirection::Up)
            .await
    }

    pub async fn downgrade_to<'a>(
        &self,
        store: DynDocumentStoreRef<'a>,
        target_revision: &str,
    ) -> DocumentStoreResult<()> {
        self.apply(store, target_revision, MigrationDirection::Down)
            .await
    }

    pub async fn apply<'a>(
        &self,
        store: DynDocumentStoreRef<'a>,
        target_revision: &str,
        direction: MigrationDirection,
    ) -> DocumentStoreResult<()> {
        let current_revision = store.current_revision_id().await?;
        let path = match direction {
            MigrationDirection::Up => {
                let from = current_revision
                    .as_deref()
                    .unwrap_or_else(|| self.chain.tail().unwrap_or(""));

                self.chain
                    .get_upgrade_path(from, target_revision)
                    .ok_or(DocumentStoreError::Migration(format!(
                        "No upgrade path from revision '{}' to '{}'",
                        from, target_revision
                    )))?
            }
            MigrationDirection::Down => {
                let from = current_revision
                    .as_deref()
                    .unwrap_or_else(|| self.chain.head().unwrap_or(""));

                self.chain
                    .get_downgrade_path(from, target_revision)
                    .ok_or(DocumentStoreError::Migration(format!(
                        "No downgrade path from revision '{}' to '{}'",
                        from, target_revision
                    )))?
            }
        };

        let op = MigrateOp::new(&store);
        for migration in path {
            match direction {
                MigrationDirection::Up => migration.up(&op).await?,
                MigrationDirection::Down => migration.down(&op).await?,
            };
            store
                .set_revision_id(migration.id())
                .await?;
        }

        Ok(())
    }
}

#[async_trait]
pub trait Migrator: Send + Sync {
    async fn upgrade_to<M: Migrations>(&self, target_revision: &str) -> DocumentStoreResult<()>;
    async fn downgrade_to<M: Migrations>(&self, target_revision: &str) -> DocumentStoreResult<()>;
    async fn upgrade<M: Migrations>(&self) -> DocumentStoreResult<()>;
    async fn downgrade<M: Migrations>(&self) -> DocumentStoreResult<()>;
}

#[async_trait]
impl<T> Migrator for T
where
    T: AsDynDocumentStore + Send + Sync,
{
    async fn upgrade_to<M: Migrations>(&self, target_revision: &str) -> DocumentStoreResult<()> {
        MigrationRunner::<M>::new()
            .upgrade_to(self.as_dyn(), target_revision)
            .await
    }

    async fn downgrade_to<M: Migrations>(&self, target_revision: &str) -> DocumentStoreResult<()> {
        MigrationRunner::<M>::new()
            .downgrade_to(self.as_dyn(), target_revision)
            .await
    }

    async fn upgrade<M: Migrations>(&self) -> DocumentStoreResult<()> {
        MigrationRunner::<M>::new()
            .upgrade(self.as_dyn())
            .await
    }

    async fn downgrade<M: Migrations>(&self) -> DocumentStoreResult<()> {
        MigrationRunner::<M>::new()
            .downgrade(self.as_dyn())
            .await
    }
}
