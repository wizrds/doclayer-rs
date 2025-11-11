//! Error types and result types for document store operations.
//!
//! This module provides comprehensive error handling for all document store operations.
//! Use [`DocumentStoreResult<T>`] as the return type for fallible operations.

use bson::error::Error as BsonError;
use serde_json::Error as SerdeJsonError;
use thiserror::Error;

/// Represents all possible errors that can occur when interacting with a document store.
///
/// This enum covers serialization errors, document lifecycle issues, collection management,
/// and backend-specific errors.
#[derive(Error, Debug)]
pub enum DocumentStoreError {
    /// Serialization/deserialization error when converting between document formats (BSON, JSON).
    #[error("Serialization error: {0}")]
    Serialization(String),
    /// Error during store initialization or connection setup.
    #[error("Initialization error: {0}")]
    Initialization(String),
    /// A document with the given ID already exists in the collection.
    /// The first argument is the document ID, the second is the collection name.
    #[error("Document {0} already exists in collection {1}")]
    DocumentAlreadyExists(String, String),
    /// The requested document was not found in the collection.
    /// The first argument is the document ID, the second is the collection name.
    #[error("Document not found {0} in collection {1}")]
    DocumentNotFound(String, String),
    /// The requested collection does not exist in the store.
    #[error("Collection not found: {0}")]
    CollectionNotFound(String),
    /// The document violates schema constraints or has invalid structure.
    #[error("Invalid document: {0}")]
    InvalidDocument(String),
    /// An error occurred in the underlying storage backend.
    #[error("Backend error: {0}")]
    Backend(String),
    /// An error occurred during schema migration.
    #[error("Migration error: {0}")]
    Migration(String),
    /// An unknown error occurred.
    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// A specialized `Result` type for document store operations.
///
/// This type alias is used throughout the crate to indicate operations that may fail
/// with a [`DocumentStoreError`].
pub type DocumentStoreResult<T> = Result<T, DocumentStoreError>;

impl From<BsonError> for DocumentStoreError {
    fn from(err: BsonError) -> Self {
        DocumentStoreError::Serialization(err.to_string())
    }
}

impl From<SerdeJsonError> for DocumentStoreError {
    fn from(err: SerdeJsonError) -> Self {
        DocumentStoreError::Serialization(err.to_string())
    }
}
