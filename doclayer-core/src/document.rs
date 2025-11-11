//! Core traits and types for document representation and serialization.
//!
//! This module provides the fundamental traits that all stored documents must implement,
//! as well as utilities for converting documents between different formats (BSON, JSON).

use bson::{Bson, Uuid, de::deserialize_from_bson, ser::serialize_to_bson};
use serde::{Deserialize, Serialize};
use serde_json::{Value, from_value, to_value};
use std::any::Any;

use crate::error::DocumentStoreResult;

/// Core trait that all documents stored in a document store must implement.
///
/// This trait defines the minimal interface required for a type to be used as a document.
/// Every document must have a unique identifier (UUID) and specify which collection it belongs to.
///
/// # Deriving with `#[derive]`
///
/// While `Document` cannot be automatically derived, you can derive its super-traits:
/// - `Serialize` (from serde)
/// - `Deserialize` (from serde)
/// - `Clone`
/// - `Debug`
///
/// # Example
///
/// ```ignore
/// use doclayer::document::Document;
/// use bson::Uuid;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// pub struct User {
///     pub id: Uuid,
///     pub name: String,
///     pub email: String,
/// }
///
/// impl Document for User {
///     fn id(&self) -> &Uuid {
///         &self.id
///     }
///
///     fn collection_name() -> &'static str {
///         "users"
///     }
/// }
/// ```
pub trait Document: Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static {
    /// Returns a reference to this document's unique identifier.
    fn id(&self) -> &Uuid;

    /// Returns the name of the collection this document belongs to.
    ///
    /// This should be a static, lowercase identifier (e.g., "users", "products").
    /// The collection will be automatically created if it doesn't exist.
    fn collection_name() -> &'static str;
}

/// Extension trait providing serialization/deserialization utilities for documents.
///
/// This trait is automatically implemented for all types that implement [`Document`].
/// It provides convenient methods to convert documents to and from BSON and JSON formats.
pub trait DocumentExt: Document {
    /// Converts this document to a BSON value for storage.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    fn to_bson(&self) -> DocumentStoreResult<Bson>;

    /// Creates a document from a BSON value.
    ///
    /// # Errors
    ///
    /// Returns an error if deserialization fails or the structure is invalid.
    fn from_bson(bson: Bson) -> DocumentStoreResult<Self>;

    /// Converts this document to a JSON value for serialization.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    fn to_json(&self) -> DocumentStoreResult<Value>;

    /// Creates a document from a JSON value.
    ///
    /// # Errors
    ///
    /// Returns an error if deserialization fails or the structure is invalid.
    fn from_json(value: Value) -> DocumentStoreResult<Self>;
}

impl<D: Document> DocumentExt for D {
    fn to_bson(&self) -> DocumentStoreResult<Bson> {
        Ok(serialize_to_bson(self)?)
    }

    fn from_bson(bson: Bson) -> DocumentStoreResult<Self> {
        Ok(deserialize_from_bson(bson)?)
    }

    fn to_json(&self) -> DocumentStoreResult<Value> {
        Ok(to_value(self)?)
    }

    fn from_json(value: Value) -> DocumentStoreResult<Self> {
        Ok(from_value(value)?)
    }
}

/// Type-erased document trait that allows working with documents of different types uniformly.
///
/// This trait enables dynamic dispatch for documents when the concrete type is not known
/// at compile time. It works similarly to standard trait objects but with additional
/// document-specific functionality.
///
/// Most users should use the concrete `Document` trait. This trait is useful in scenarios where
/// multiple document types need to be stored together or passed through APIs that don't know
/// the specific type.
pub trait AnyDocument: Send + Sync {
    /// Returns a reference to this document's unique identifier.
    fn document_id(&self) -> &Uuid;

    /// Returns the name of the collection this document belongs to.
    fn document_collection(&self) -> &'static str;

    /// Returns a reference to the document as a generic `Any` type.
    fn as_any(&self) -> &dyn Any;

    /// Returns a mutable reference to the document as a generic `Any` type.
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Clones the document into a new boxed `AnyDocument`.
    fn clone_box(&self) -> Box<dyn AnyDocument>;

    /// Converts this document to BSON format.
    fn to_any_bson(&self) -> DocumentStoreResult<Bson>;

    /// Converts this document to JSON format.
    fn to_any_json(&self) -> DocumentStoreResult<Value>;
}

impl dyn AnyDocument {
    /// Attempts to downcast a reference to a specific document type.
    ///
    /// Returns `Some(&D)` if this trait object contains a `D`, otherwise `None`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let doc: Box<dyn AnyDocument> = /* ... */;
    /// if let Some(user) = doc.downcast_ref::<User>() {
    ///     println!("User email: {}", user.email);
    /// }
    /// ```
    pub fn downcast_ref<D: Document>(&self) -> Option<&D> {
        self.as_any().downcast_ref::<D>()
    }

    /// Attempts to downcast a mutable reference to a specific document type.
    ///
    /// Returns `Some(&mut D)` if this trait object contains a `D`, otherwise `None`.
    pub fn downcast_mut<D: Document>(&mut self) -> Option<&mut D> {
        self.as_any_mut().downcast_mut::<D>()
    }
}

impl<D: Document> AnyDocument for D {
    fn document_id(&self) -> &Uuid {
        self.id()
    }

    fn document_collection(&self) -> &'static str {
        Self::collection_name()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn AnyDocument> {
        Box::new(self.clone())
    }

    fn to_any_bson(&self) -> DocumentStoreResult<Bson> {
        DocumentExt::to_bson(self)
    }

    fn to_any_json(&self) -> DocumentStoreResult<Value> {
        DocumentExt::to_json(self)
    }
}

impl Clone for Box<dyn AnyDocument> {
    fn clone(&self) -> Box<dyn AnyDocument> {
        self.clone_box()
    }
}

/// Conversion trait for converting any type into a boxed `AnyDocument`.
///
/// This trait enables ergonomic conversion from concrete document types
/// into type-erased documents for storage in heterogeneous collections.
///
/// # Example
///
/// ```ignore
/// use doclayer::document::IntoAnyDocument;
/// let user = User { /* ... */ };
/// let any_doc: Box<dyn AnyDocument> = user.into_any_document();
/// ```
pub trait IntoAnyDocument {
    /// Converts this value into a boxed `AnyDocument`.
    fn into_any_document(self) -> Box<dyn AnyDocument>;
}

impl<D: Document> IntoAnyDocument for D {
    fn into_any_document(self) -> Box<dyn AnyDocument> {
        Box::new(self) as Box<dyn AnyDocument>
    }
}

impl IntoAnyDocument for Box<dyn AnyDocument> {
    fn into_any_document(self) -> Box<dyn AnyDocument> {
        self
    }
}
