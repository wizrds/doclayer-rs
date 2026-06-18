//! Pagination types for query execution and result paging.
//!
//! This module provides the [`Pagination`] type for requesting either an
//! offset-based or a cursor-based page of results, the [`Cursor`] opaque
//! token type used to resume cursor-based pagination, and the [`Page`] result
//! type that carries paginated results back to a consumer.

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use bson::{Bson, Uuid, deserialize_from_slice, serialize_to_vec};
use serde::{Deserialize, Serialize};

use crate::error::{DocumentStoreError, DocumentStoreResult};

/// The direction to walk when paginating by cursor.
///
/// [`Forward`](CursorDirection::Forward) requests items that come after the
/// cursor position according to the query's sort order; [`Backward`](CursorDirection::Backward)
/// requests items that come before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorDirection {
    Forward,
    Backward,
}

/// The exact position a cursor resumes from: the sort key's value and the
/// document id of the last item seen, used together as a tiebreaker so that
/// pagination remains deterministic even when many documents share the same
/// sort value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CursorPosition {
    pub sort_value: Bson,
    pub id: Uuid,
}

/// An opaque, resumable pagination token.
///
/// A [`Cursor`] is handed back to a consumer as part of a [`Page`] and is
/// meant to be passed back unmodified on a later query to continue from
/// where the previous page left off. Its string form is safe to embed in a
/// URL query parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor(String);

impl Cursor {
    /// Encodes a [`CursorPosition`] into an opaque [`Cursor`] token.
    pub fn encode(position: &CursorPosition) -> DocumentStoreResult<Self> {
        Ok(Cursor(BASE64.encode(serialize_to_vec(position)?)))
    }

    /// Decodes this [`Cursor`] back into the [`CursorPosition`] it was created from.
    ///
    /// # Errors
    ///
    /// Returns an error if the cursor string is not valid base64, or if the
    /// decoded bytes are not a valid encoded [`CursorPosition`]. This can
    /// happen if a caller passes back a tampered or malformed cursor string.
    pub fn decode(&self) -> DocumentStoreResult<CursorPosition> {
        Ok(deserialize_from_slice(
            &BASE64
                .decode(&self.0)
                .map_err(|e| DocumentStoreError::InvalidDocument(e.to_string()))?,
        )?)
    }

    /// Returns the opaque string form of this cursor.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for Cursor {
    fn from(value: String) -> Self {
        Cursor(value)
    }
}

/// How a [`crate::query::Query`] should be paginated.
///
/// `Offset` and `Cursor` are mutually exclusive by construction: a query is
/// always paginated in exactly one of these modes, so there is no way to
/// build a query with both an offset and a cursor set at once.
#[derive(Debug, Clone, Default)]
pub enum Pagination {
    /// No pagination; return every matching document.
    #[default]
    None,
    /// Skip `offset` documents, then return up to `limit` documents.
    Offset { offset: usize, limit: usize },
    /// Return up to `limit` documents starting after (or before, depending
    /// on `direction`) the position encoded in `cursor`. A `cursor` of
    /// `None` starts from the beginning (`Forward`) or end (`Backward`) of
    /// the result set.
    Cursor {
        cursor: Option<Cursor>,
        limit: usize,
        direction: CursorDirection,
    },
}

impl Pagination {
    /// Builds an offset-based [`Pagination`] from a 1-indexed page number and
    /// a page size, as a convenience over computing the offset by hand.
    pub fn page(page: usize, per_page: usize) -> Self {
        Pagination::Offset { offset: (page.max(1) - 1) * per_page, limit: per_page }
    }
}

/// A page of documents returned from [`crate::collection::Collection::query`]
/// and its typed/dynamic equivalents.
#[derive(Debug, Clone, PartialEq)]
pub struct Page<T> {
    pub items: Vec<T>,
    /// A cursor that resumes after the last item in this page, or `None` if
    /// there are no further items in the requested direction.
    pub next_cursor: Option<Cursor>,
    /// A cursor that resumes before the first item in this page, or `None`
    /// if this is already the first page.
    pub previous_cursor: Option<Cursor>,
    /// The total number of documents matching the query, across all pages.
    /// Only populated when the query set `include_total_count`, since
    /// counting can be expensive on some backends.
    pub total_count: Option<usize>,
}

impl<T> Page<T> {
    /// Fallibly maps each item in this page, carrying `next_cursor`,
    /// `previous_cursor`, and `total_count` through unchanged.
    pub fn try_map<U, E>(self, f: impl FnMut(T) -> Result<U, E>) -> Result<Page<U>, E> {
        Ok(Page {
            items: self
                .items
                .into_iter()
                .map(f)
                .collect::<Result<Vec<U>, E>>()?,
            next_cursor: self.next_cursor,
            previous_cursor: self.previous_cursor,
            total_count: self.total_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_round_trips_sort_value_and_id() {
        let position = CursorPosition { sort_value: Bson::Int32(42), id: Uuid::new() };

        let decoded = Cursor::encode(&position)
            .unwrap()
            .decode()
            .unwrap();

        assert_eq!(decoded, position);
    }

    #[test]
    fn cursor_decode_rejects_malformed_input() {
        let cursor = Cursor::from("not valid base64!!".to_string());

        assert!(cursor.decode().is_err());
    }

    #[test]
    fn pagination_page_computes_offset() {
        match Pagination::page(3, 20) {
            Pagination::Offset { offset, limit } => {
                assert_eq!(offset, 40);
                assert_eq!(limit, 20);
            }
            _ => panic!("expected Pagination::Offset"),
        }
    }

    #[test]
    fn pagination_page_clamps_page_zero_to_first_page() {
        match Pagination::page(0, 10) {
            Pagination::Offset { offset, .. } => assert_eq!(offset, 0),
            _ => panic!("expected Pagination::Offset"),
        }
    }
}
