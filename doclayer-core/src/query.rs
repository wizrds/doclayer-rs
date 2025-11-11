//! Query construction and filtering API for document stores.
//!
//! This module provides type-safe query construction with filtering, sorting, pagination,
//! and a visitor pattern for query execution across different backends.
//!
//! # Query Building
//!
//! Queries can be constructed using the fluent builder API:
//!
//! ```ignore
//! use doclayer::query::{Query, Filter};
//!
//! let query = Query::builder()
//!     .filter(Filter::eq("name", "Alice"))
//!     .limit(10)
//!     .offset(0)
//!     .sort("created_at", SortDirection::Desc)
//!     .build();
//! ```
//!
//! # Filter Expression API
//!
//! The [`Filter`] struct provides a collection of static methods for building filter expressions:
//!
//! - Comparison: `eq`, `ne`, `gt`, `gte`, `lt`, `lte`
//! - String: `starts_with`, `ends_with`, `contains`, `not_contains`
//! - Existence: `exists`, `not_exists`
//! - Array: `any_of`, `none_of`
//! - Logical: `and`, `or`
//!
//! Expressions can be combined using chainable methods for more complex queries.

use bson::Bson;

use crate::error::DocumentStoreError;

/// Sort direction for query results.
#[derive(Debug, Clone)]
pub enum SortDirection {
    /// Ascending order (A to Z, 0 to 9, earliest to latest).
    Asc,
    /// Descending order (Z to A, 9 to 0, latest to earliest).
    Desc,
}

/// Sort specification for query results.
///
/// Specifies which field to sort by and in which direction.
#[derive(Debug, Clone)]
pub struct Sort {
    /// The field name to sort by.
    pub field: String,
    /// The sort direction.
    pub direction: SortDirection,
}

/// Field comparison operators for filter expressions.
#[derive(Debug, Clone)]
pub enum FieldOp {
    /// Equal to (exact match).
    Eq,
    /// Not equal to.
    Ne,
    /// Greater than.
    Gt,
    /// Greater than or equal to.
    Gte,
    /// Less than.
    Lt,
    /// Less than or equal to.
    Lte,
    /// String or array contains value.
    Contains,
    /// String or array does not contain value.
    NotContains,
    /// String starts with value.
    StartsWith,
    /// String ends with value.
    EndsWith,
    /// Array contains any of the values.
    AnyOf,
    /// Array contains none of the values.
    NoneOf,
}

/// A filter expression for querying documents.
///
/// Expressions can be combined using logical operators (`And`, `Or`, `Not`)
/// to build complex filter predicates.
///
/// # Example
///
/// ```ignore
/// use doclayer::query::{Expr, Filter, FieldOp};
///
/// // Simple equality check
/// let expr1 = Filter::eq("status", "active");
///
/// // Complex nested expression
/// let expr2 = Filter::and(vec![
///     Filter::eq("status", "active"),
///     Filter::gt("age", 18)
/// ]);
/// ```
#[derive(Debug, Clone)]
pub enum Expr {
    /// Logical AND of multiple expressions (all must match).
    And(Vec<Expr>),
    /// Logical OR of multiple expressions (any must match).
    Or(Vec<Expr>),
    /// Logical NOT of an expression (inverts the result).
    Not(Box<Expr>),
    /// Checks if a field exists or doesn't exist.
    Exists(String, bool),
    /// Field comparison expression.
    Field {
        /// The field name to compare.
        field: String,
        /// The comparison operator.
        op: FieldOp,
        /// The value to compare against.
        value: Bson,
    },
}

impl Expr {
    /// Creates a field comparison expression.
    pub fn field(field: String, op: FieldOp, value: Bson) -> Self {
        Expr::Field { field, op, value }
    }

    /// Combines this expression with another using logical AND.
    ///
    /// If this expression is already an AND, the other expression is appended
    /// to the list. Otherwise, a new AND expression is created.
    pub fn and(self, other: Expr) -> Self {
        match self {
            Expr::And(mut list) => {
                list.push(other);
                Expr::And(list)
            }
            _ => Expr::And(vec![self, other]),
        }
    }

    /// Combines this expression with another using logical OR.
    ///
    /// If this expression is already an OR, the other expression is appended
    /// to the list. Otherwise, a new OR expression is created.
    pub fn or(self, other: Expr) -> Self {
        match self {
            Expr::Or(mut list) => {
                list.push(other);
                Expr::Or(list)
            }
            _ => Expr::Or(vec![self, other]),
        }
    }

    /// Negates this expression (logical NOT).
    pub fn not(self) -> Self {
        Expr::Not(Box::new(self))
    }
}

/// A structured query for retrieving and filtering documents.
///
/// This struct encapsulates filters, limits, offsets, and sort specifications
/// for document queries. Use [`QueryBuilder`] for ergonomic construction.
///
/// # Example
///
/// ```ignore
/// use doclayer::query::{Query, Filter, SortDirection};
///
/// let query = Query::builder()
///     .filter(Filter::eq("status", "active"))
///     .limit(10)
///     .offset(0)
///     .sort("created_at", SortDirection::Desc)
///     .build();
/// ```
#[derive(Debug, Clone, Default)]
pub struct Query {
    /// Optional filter expression to match documents.
    pub filter: Option<Expr>,
    /// Maximum number of documents to return.
    pub limit: Option<usize>,
    /// Number of documents to skip (for pagination).
    pub offset: Option<usize>,
    /// Sort specification for results.
    pub sort: Option<Sort>,
}

impl Query {
    /// Creates a new empty query with no filters or limits.
    pub fn new() -> Self {
        Query {
            filter: None,
            limit: None,
            offset: None,
            sort: None,
        }
    }

    /// Creates a new query builder for fluent construction.
    pub fn builder() -> QueryBuilder {
        QueryBuilder::new()
    }
}

/// Helper struct for constructing filter expressions.
///
/// Provides static methods to construct common filter expressions in a type-safe manner.
/// All methods accept field names and values as `Into<String>` and `Into<Bson>` for ergonomics.
///
/// # Example
///
/// ```ignore
/// use doclayer::query::Filter;
///
/// let expr = Filter::eq("name", "Alice")
///     .and(Filter::gt("age", 18));
/// ```
pub struct Filter;

impl Filter {
    /// Creates an equality filter expression.
    ///
    /// Matches documents where the field equals the specified value.
    pub fn eq(field: impl Into<String>, value: impl Into<Bson>) -> Expr {
        Expr::field(field.into(), FieldOp::Eq, value.into())
    }

    /// Creates a not-equal filter expression.
    ///
    /// Matches documents where the field does not equal the specified value.
    pub fn ne(field: impl Into<String>, value: impl Into<Bson>) -> Expr {
        Expr::field(field.into(), FieldOp::Ne, value.into())
    }

    /// Creates a greater-than filter expression.
    ///
    /// Matches documents where the field is greater than the specified value.
    pub fn gt(field: impl Into<String>, value: impl Into<Bson>) -> Expr {
        Expr::field(field.into(), FieldOp::Gt, value.into())
    }

    /// Creates a greater-than-or-equal filter expression.
    ///
    /// Matches documents where the field is greater than or equal to the specified value.
    pub fn gte(field: impl Into<String>, value: impl Into<Bson>) -> Expr {
        Expr::field(field.into(), FieldOp::Gte, value.into())
    }

    /// Creates a less-than filter expression.
    ///
    /// Matches documents where the field is less than the specified value.
    pub fn lt(field: impl Into<String>, value: impl Into<Bson>) -> Expr {
        Expr::field(field.into(), FieldOp::Lt, value.into())
    }

    /// Creates a less-than-or-equal filter expression.
    ///
    /// Matches documents where the field is less than or equal to the specified value.
    pub fn lte(field: impl Into<String>, value: impl Into<Bson>) -> Expr {
        Expr::field(field.into(), FieldOp::Lte, value.into())
    }

    /// Creates a string prefix filter expression.
    ///
    /// Matches documents where the string field starts with the specified value.
    pub fn starts_with(field: impl Into<String>, value: impl Into<Bson>) -> Expr {
        Expr::field(field.into(), FieldOp::StartsWith, value.into())
    }

    /// Creates a string suffix filter expression.
    ///
    /// Matches documents where the string field ends with the specified value.
    pub fn ends_with(field: impl Into<String>, value: impl Into<Bson>) -> Expr {
        Expr::field(field.into(), FieldOp::EndsWith, value.into())
    }

    /// Creates a contains filter expression.
    ///
    /// Matches documents where the field (string or array) contains the specified value.
    pub fn contains(field: impl Into<String>, value: impl Into<Bson>) -> Expr {
        Expr::field(field.into(), FieldOp::Contains, value.into())
    }

    /// Creates a not-contains filter expression.
    ///
    /// Matches documents where the field (string or array) does not contain the specified value.
    pub fn not_contains(field: impl Into<String>, value: impl Into<Bson>) -> Expr {
        Expr::field(field.into(), FieldOp::NotContains, value.into())
    }

    /// Creates an existence filter expression.
    ///
    /// Matches documents where the field exists (is not null or missing).
    pub fn exists(field: impl Into<String>) -> Expr {
        Expr::Exists(field.into(), true)
    }

    /// Creates a non-existence filter expression.
    ///
    /// Matches documents where the field does not exist (is null or missing).
    pub fn not_exists(field: impl Into<String>) -> Expr {
        Expr::Exists(field.into(), false)
    }

    /// Creates a logical AND filter expression.
    ///
    /// Combines multiple expressions such that all must match for a document to be included.
    pub fn and(exprs: impl IntoIterator<Item = Expr>) -> Expr {
        Expr::And(exprs.into_iter().collect())
    }

    /// Creates a logical OR filter expression.
    ///
    /// Combines multiple expressions such that any can match for a document to be included.
    pub fn or(exprs: impl IntoIterator<Item = Expr>) -> Expr {
        Expr::Or(exprs.into_iter().collect())
    }

    /// Creates an array membership filter expression.
    ///
    /// Matches documents where the array field contains any of the specified values.
    pub fn any_of(field: impl Into<String>, value: impl Into<Bson>) -> Expr {
        Expr::field(field.into(), FieldOp::AnyOf, value.into())
    }

    /// Creates an array exclusion filter expression.
    ///
    /// Matches documents where the array field contains none of the specified values.
    pub fn none_of(field: impl Into<String>, value: impl Into<Bson>) -> Expr {
        Expr::field(field.into(), FieldOp::NoneOf, value.into())
    }
}

#[derive(Debug, Clone)]
pub struct QueryBuilder {
    query: Query,
}

impl QueryBuilder {
    /// Creates a new query builder.
    pub fn new() -> Self {
        QueryBuilder { query: Query::default() }
    }

    /// Sets the filter expression for this query.
    ///
    /// # Arguments
    ///
    /// * `filter` - The filter expression to apply
    pub fn filter(mut self, filter: Expr) -> Self {
        self.query.filter = Some(filter);
        self
    }

    /// Sets the maximum number of documents to return.
    ///
    /// # Arguments
    ///
    /// * `limit` - The maximum number of documents to return
    pub fn limit(mut self, limit: usize) -> Self {
        self.query.limit = Some(limit);
        self
    }

    /// Sets the number of documents to skip (for pagination).
    ///
    /// # Arguments
    ///
    /// * `offset` - The number of documents to skip
    pub fn offset(mut self, offset: usize) -> Self {
        self.query.offset = Some(offset);
        self
    }

    /// Sets the sort specification for the query results.
    ///
    /// # Arguments
    ///
    /// * `field` - The field name to sort by
    /// * `direction` - The sort direction (ascending or descending)
    pub fn sort(mut self, field: impl Into<String>, direction: SortDirection) -> Self {
        self.query.sort = Some(Sort { field: field.into(), direction });
        self
    }

    /// Builds and returns the final query.
    pub fn build(self) -> Query {
        self.query
    }
}

pub trait QueryVisitor {
    type Output;
    type Error: Into<DocumentStoreError>;

    fn visit_and(&mut self, exprs: &[Expr]) -> Result<Self::Output, Self::Error>;
    fn visit_or(&mut self, exprs: &[Expr]) -> Result<Self::Output, Self::Error>;
    fn visit_not(&mut self, expr: &Expr) -> Result<Self::Output, Self::Error>;
    fn visit_exists(
        &mut self,
        field: &str,
        should_exist: bool,
    ) -> Result<Self::Output, Self::Error>;
    fn visit_field(
        &mut self,
        field: &str,
        op: &FieldOp,
        value: &Bson,
    ) -> Result<Self::Output, Self::Error>;

    fn visit_expr(&mut self, expr: &Expr) -> Result<Self::Output, Self::Error> {
        match expr {
            Expr::And(exprs) => self.visit_and(exprs),
            Expr::Or(exprs) => self.visit_or(exprs),
            Expr::Not(expr) => self.visit_not(expr),
            Expr::Exists(field, should_exist) => self.visit_exists(field, *should_exist),
            Expr::Field { field, op, value } => self.visit_field(field, op, value),
        }
    }
}
