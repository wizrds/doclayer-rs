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
//!     .offset_page(0, 10)
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
//!
//! # Building Optional and Conditional Filters
//!
//! When some or all of a filter's conditions are optional, use [`FilterBuilder`]
//! (via [`Filter::all`] or [`Filter::any`]) instead of chaining [`Expr::and`]/[`Expr::or`]
//! off of a guaranteed seed condition. A [`FilterBuilder`] starts empty and accepts
//! conditions one at a time, conditionally based on a `bool`, an `Option<T>`, or a
//! closure, and collapses to `None` if nothing was ever added:
//!
//! ```ignore
//! use doclayer::query::{Query, Filter};
//!
//! let query = Query::builder()
//!     .filter(
//!         Filter::all()
//!             .add_opt(name_filter, |name| Filter::contains("name", name))
//!             .add_if(include_admins, Filter::eq("role", "admin")),
//!     )
//!     .offset_page(0, 10)
//!     .build();
//! ```

use bson::Bson;

use crate::{
    error::DocumentStoreError,
    page::{Cursor, CursorDirection, Pagination},
};

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
///     .offset_page(0, 10)
///     .sort("created_at", SortDirection::Desc)
///     .build();
/// ```
#[derive(Debug, Clone, Default)]
pub struct Query {
    /// Optional filter expression to match documents.
    pub filter: Option<Expr>,
    /// How this query should be paginated. Offset-based and cursor-based
    /// pagination are mutually exclusive modes of this single field.
    pub pagination: Pagination,
    /// Sort specification for results.
    pub sort: Option<Sort>,
    /// Whether to compute the total number of matching documents across all
    /// pages. Left `false` by default since counting can be expensive on
    /// some backends.
    pub include_total_count: bool,
}

impl Query {
    /// Creates a new empty query with no filters, pagination, or sort.
    pub fn new() -> Self {
        Query {
            filter: None,
            pagination: Pagination::None,
            sort: None,
            include_total_count: false,
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

    /// Starts an empty [`FilterBuilder`] that combines its conditions with
    /// logical AND, for composing a filter out of zero or more optional
    /// conditions in a single fluent chain.
    pub fn all() -> FilterBuilder {
        FilterBuilder { mode: LogicalOp::And, conditions: Vec::new() }
    }

    /// Starts an empty [`FilterBuilder`] that combines its conditions with
    /// logical OR, for composing a filter out of zero or more optional
    /// conditions in a single fluent chain.
    pub fn any() -> FilterBuilder {
        FilterBuilder { mode: LogicalOp::Or, conditions: Vec::new() }
    }
}

/// The logical combinator a [`FilterBuilder`] applies to its accumulated
/// conditions once built.
#[derive(Debug, Clone, Copy)]
enum LogicalOp {
    And,
    Or,
}

/// A fluent accumulator for composing a filter [`Expr`] out of zero or more
/// conditions, some or all of which may be conditionally present.
///
/// Unlike [`Filter`]'s static constructors, which always produce a concrete
/// [`Expr`] immediately, a [`FilterBuilder`] starts empty and lets a caller
/// add conditions one at a time, conditionally based on a `bool`, an
/// `Option<T>`, or a closure, without leaving the fluent chain. Build one
/// with [`Filter::all`] (AND-combined) or [`Filter::any`] (OR-combined).
///
/// # Example
///
/// ```
/// use doclayer_core::query::{Expr, FieldOp, Filter};
///
/// let name_filter: Option<&str> = Some("Alice");
/// let include_admins = false;
///
/// let filter = Filter::all()
///     .add_opt(name_filter, |name| Filter::contains("name", name))
///     .add_if(include_admins, Filter::eq("role", "admin"))
///     .build();
///
/// assert!(matches!(
///     filter,
///     Some(Expr::Field { op: FieldOp::Contains, .. })
/// ));
/// ```
#[derive(Debug, Clone)]
pub struct FilterBuilder {
    mode: LogicalOp,
    conditions: Vec<Expr>,
}

impl FilterBuilder {
    /// Unconditionally adds `expr` as another condition.
    pub fn add(mut self, expr: Expr) -> Self {
        self.conditions.push(expr);
        self
    }

    /// Adds `expr` as another condition only if `cond` is `true`.
    pub fn add_if(self, cond: bool, expr: Expr) -> Self {
        if cond { self.add(expr) } else { self }
    }

    /// Adds `f(value)` as another condition only if `opt` is `Some(value)`.
    pub fn add_opt<T>(self, opt: Option<T>, f: impl FnOnce(T) -> Expr) -> Self {
        match opt {
            Some(value) => self.add(f(value)),
            None => self,
        }
    }

    /// Adds the result of `f` as another condition only if it returns `Some`.
    pub fn add_with(self, f: impl FnOnce() -> Option<Expr>) -> Self {
        match f() {
            Some(expr) => self.add(expr),
            None => self,
        }
    }

    /// Adds the result of `f` as another condition only if it returns
    /// `Ok(Some(_))`. Propagates `Err` immediately, abandoning everything
    /// accumulated on this builder so far, matching ordinary `?`-style
    /// fallible chaining elsewhere in this crate.
    pub fn try_add_with<E>(self, f: impl FnOnce() -> Result<Option<Expr>, E>) -> Result<Self, E> {
        Ok(match f()? {
            Some(expr) => self.add(expr),
            None => self,
        })
    }

    /// Nests an AND-combined [`FilterBuilder`] group, built by `f`, as a
    /// single condition on this builder. If `f` adds no conditions to the
    /// nested builder, nothing is added here either.
    pub fn and_group(self, f: impl FnOnce(FilterBuilder) -> FilterBuilder) -> Self {
        match f(Filter::all()).build() {
            Some(expr) => self.add(expr),
            None => self,
        }
    }

    /// Nests an OR-combined [`FilterBuilder`] group, built by `f`, as a
    /// single condition on this builder. If `f` adds no conditions to the
    /// nested builder, nothing is added here either.
    pub fn or_group(self, f: impl FnOnce(FilterBuilder) -> FilterBuilder) -> Self {
        match f(Filter::any()).build() {
            Some(expr) => self.add(expr),
            None => self,
        }
    }

    /// Finishes this builder, collapsing its accumulated conditions into a
    /// single filter expression: `None` if nothing was ever added, the bare
    /// [`Expr`] itself if exactly one condition was added, or an
    /// [`Expr::And`]/[`Expr::Or`] of every condition otherwise.
    pub fn build(self) -> Option<Expr> {
        match self.conditions.len() {
            0 => None,
            1 => self.conditions.into_iter().next(),
            _ => Some(match self.mode {
                LogicalOp::And => Expr::And(self.conditions),
                LogicalOp::Or => Expr::Or(self.conditions),
            }),
        }
    }
}

impl From<FilterBuilder> for Option<Expr> {
    fn from(builder: FilterBuilder) -> Self {
        builder.build()
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
    /// Accepts a bare [`Expr`], an `Option<Expr>`, or a [`FilterBuilder`]
    /// directly (via its `Into<Option<Expr>>` conversion), so a filter built
    /// from entirely optional conditions can flow straight into this method
    /// without an intermediate `.build()` call. Passing a value that
    /// converts to `None` clears any previously set filter.
    ///
    /// # Arguments
    ///
    /// * `filter` - The filter expression to apply, or `None` for no filter
    pub fn filter(mut self, filter: impl Into<Option<Expr>>) -> Self {
        self.query.filter = filter.into();
        self
    }

    /// Paginates this query by offset: skips `offset` documents, then
    /// returns up to `limit` documents.
    ///
    /// This is mutually exclusive with [`QueryBuilder::cursor_page`]; calling
    /// either replaces whatever pagination mode was previously set.
    ///
    /// # Arguments
    ///
    /// * `offset` - The number of documents to skip
    /// * `limit` - The maximum number of documents to return
    pub fn offset_page(mut self, offset: usize, limit: usize) -> Self {
        self.query.pagination = Pagination::Offset { offset, limit };
        self
    }

    /// Paginates this query by cursor: returns up to `limit` documents
    /// starting after (or before, depending on `direction`) the position
    /// encoded in `cursor`. Pass `None` to start from the beginning
    /// (`CursorDirection::Forward`) or end (`CursorDirection::Backward`) of
    /// the result set.
    ///
    /// This is mutually exclusive with [`QueryBuilder::offset_page`]; calling
    /// either replaces whatever pagination mode was previously set.
    ///
    /// # Arguments
    ///
    /// * `cursor` - The cursor to resume from, or `None` to start from an end
    /// * `limit` - The maximum number of documents to return
    /// * `direction` - Which direction to walk relative to the cursor
    pub fn cursor_page(mut self, cursor: Option<Cursor>, limit: usize, direction: CursorDirection) -> Self {
        self.query.pagination = Pagination::Cursor { cursor, limit, direction };
        self
    }

    /// Sets whether to compute the total number of matching documents across
    /// all pages. Left `false` by default since counting can be expensive on
    /// some backends.
    ///
    /// # Arguments
    ///
    /// * `include` - Whether to compute the total count
    pub fn include_total_count(mut self, include: bool) -> Self {
        self.query.include_total_count = include;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn is_eq_field(expr: &Expr, field: &str) -> bool {
        matches!(expr, Expr::Field { field: f, op: FieldOp::Eq, .. } if f == field)
    }

    #[test]
    fn filter_builder_with_no_conditions_builds_none() {
        assert!(Filter::all().build().is_none());
        assert!(Filter::any().build().is_none());
    }

    #[test]
    fn filter_builder_with_one_condition_collapses_to_bare_expr() {
        let built = Filter::all().add(Filter::eq("status", "active")).build();

        assert!(matches!(built, Some(ref expr) if is_eq_field(expr, "status")));
    }

    #[test]
    fn filter_builder_and_mode_combines_multiple_conditions_in_order() {
        let built = Filter::all()
            .add(Filter::eq("status", "active"))
            .add(Filter::eq("role", "admin"))
            .build();

        match built {
            Some(Expr::And(conditions)) => {
                assert!(is_eq_field(&conditions[0], "status"));
                assert!(is_eq_field(&conditions[1], "role"));
            }
            other => panic!("expected Expr::And, got {other:?}"),
        }
    }

    #[test]
    fn filter_builder_or_mode_combines_multiple_conditions_in_order() {
        let built = Filter::any()
            .add(Filter::eq("role", "admin"))
            .add(Filter::eq("role", "owner"))
            .build();

        match built {
            Some(Expr::Or(conditions)) => {
                assert!(is_eq_field(&conditions[0], "role"));
                assert!(is_eq_field(&conditions[1], "role"));
            }
            other => panic!("expected Expr::Or, got {other:?}"),
        }
    }

    #[test]
    fn add_if_skips_when_false_and_adds_when_true() {
        let skipped = Filter::all()
            .add_if(false, Filter::eq("role", "admin"))
            .build();

        assert!(skipped.is_none());

        let added = Filter::all()
            .add_if(true, Filter::eq("role", "admin"))
            .build();

        assert!(matches!(added, Some(ref expr) if is_eq_field(expr, "role")));
    }

    #[test]
    fn add_opt_skips_on_none_and_unwraps_on_some() {
        let skipped = Filter::all()
            .add_opt(None::<&str>, |name| Filter::contains("name", name))
            .build();

        assert!(skipped.is_none());

        let added = Filter::all()
            .add_opt(Some("Alice"), |name| Filter::contains("name", name))
            .build();

        assert!(matches!(
            added,
            Some(Expr::Field { op: FieldOp::Contains, .. })
        ));
    }

    #[test]
    fn add_with_skips_on_none_and_adds_on_some() {
        let skipped = Filter::all().add_with(|| None).build();

        assert!(skipped.is_none());

        let added = Filter::all()
            .add_with(|| Some(Filter::eq("status", "active")))
            .build();

        assert!(matches!(added, Some(ref expr) if is_eq_field(expr, "status")));
    }

    #[test]
    fn try_add_with_propagates_ok_variants_and_short_circuits_on_err() {
        let skipped = Filter::all()
            .try_add_with(|| Ok::<_, &str>(None))
            .unwrap()
            .build();

        assert!(skipped.is_none());

        let added = Filter::all()
            .try_add_with(|| Ok::<_, &str>(Some(Filter::eq("status", "active"))))
            .unwrap()
            .build();

        assert!(matches!(added, Some(ref expr) if is_eq_field(expr, "status")));

        let failed = Filter::all()
            .add(Filter::eq("status", "active"))
            .try_add_with(|| Err("boom"));

        assert_eq!(failed.unwrap_err(), "boom");
    }

    #[test]
    fn and_group_nests_an_or_group_inside_an_and_chain() {
        let built = Filter::all()
            .add(Filter::eq("status", "active"))
            .or_group(|g| {
                g.add(Filter::eq("role", "admin"))
                    .add(Filter::eq("role", "owner"))
            })
            .build();

        match built {
            Some(Expr::And(conditions)) => {
                assert!(is_eq_field(&conditions[0], "status"));
                assert!(matches!(conditions[1], Expr::Or(ref nested) if nested.len() == 2));
            }
            other => panic!("expected Expr::And, got {other:?}"),
        }
    }

    #[test]
    fn group_with_no_conditions_added_leaves_outer_builder_unchanged() {
        let built = Filter::all()
            .add(Filter::eq("status", "active"))
            .or_group(|g| g.add_if(false, Filter::eq("role", "admin")))
            .build();

        assert!(matches!(built, Some(ref expr) if is_eq_field(expr, "status")));
    }

    #[test]
    fn query_builder_filter_accepts_a_filter_builder_directly() {
        let query = Query::builder()
            .filter(Filter::all().add_opt(None::<&str>, |name| Filter::contains("name", name)))
            .build();

        assert!(query.filter.is_none());

        let query = Query::builder()
            .filter(Filter::all().add(Filter::eq("status", "active")))
            .build();

        assert!(matches!(query.filter, Some(ref expr) if is_eq_field(expr, "status")));
    }
}
