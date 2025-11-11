//! Pagination and result types for managing query results.
//!
//! This module provides pagination support for large result sets,
//! including the [`Page`] struct for result pages and [`PaginationParams`]
//! for specifying pagination parameters.

use serde::{Deserialize, Serialize};
use std::cmp::min;

/// A single page of paginated results.
///
/// This struct represents a subset of results from a larger dataset,
/// along with metadata for navigating through the pages.
///
/// # Type Parameters
///
/// * `T` - The type of items contained in this page
///
/// # Example
///
/// ```ignore
/// use doclayer::page::Page;
///
/// let page: Page<String> = Page::builder(vec!["item1".to_string()])
///     .with_count(100)
///     .with_next_page(Some(2))
///     .build();
///
/// assert_eq!(page.items.len(), 1);
/// assert_eq!(page.count, 100);
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Page<T> {
    /// The items contained in this page.
    pub items: Vec<T>,
    /// Total count of items across all pages.
    pub count: usize,
    /// The next page number (if more pages exist).
    pub next_page: Option<usize>,
    /// The previous page number (if this is not the first page).
    pub previous_page: Option<usize>,
}

impl<T> Page<T> {
    /// Creates a new builder for constructing a page with custom settings.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let page = Page::builder(vec![1, 2, 3])
    ///     .with_count(10)
    ///     .with_next_page(Some(2))
    ///     .build();
    /// ```
    pub fn builder(items: Vec<T>) -> PageBuilder<T> {
        PageBuilder::new(items)
    }
}

impl<T> Default for Page<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            count: 0,
            next_page: None,
            previous_page: None,
        }
    }
}

/// Builder for constructing [`Page`] instances with fluent API.
///
/// This builder allows incremental construction of a page with
/// pagination metadata.
pub struct PageBuilder<T> {
    items: Vec<T>,
    count: usize,
    next_page: Option<usize>,
    previous_page: Option<usize>,
}

impl<T> PageBuilder<T> {
    /// Creates a new builder with the given items.
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items,
            count: 0,
            next_page: None,
            previous_page: None,
        }
    }

    /// Sets the total count of items across all pages.
    pub fn with_count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }

    /// Sets the next page number (or `None` if this is the last page).
    pub fn with_next_page(mut self, next_page: Option<usize>) -> Self {
        self.next_page = next_page;
        self
    }

    /// Sets the previous page number (or `None` if this is the first page).
    pub fn with_previous_page(mut self, previous_page: Option<usize>) -> Self {
        self.previous_page = previous_page;
        self
    }

    /// Builds and returns the final [`Page`] instance.
    pub fn build(self) -> Page<T> {
        Page {
            items: self.items,
            count: self.count,
            next_page: self.next_page,
            previous_page: self.previous_page,
        }
    }
}

/// Parameters for paginating through large result sets.
///
/// This struct specifies which page to retrieve and how many items per page.
/// Pages are 1-indexed (page 1 is the first page).
///
/// # Example
///
/// ```ignore
/// use doclayer::page::PaginationParams;
///
/// let params = PaginationParams::new(2, 50);
/// // Retrieves page 2 with 50 items per page
/// // Offset is (2-1) * 50 = 50
/// assert_eq!(params.offset(), 50);
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PaginationParams {
    /// The page number (1-indexed).
    pub page: usize,
    /// Number of items per page.
    pub per_page: usize,
}

impl PaginationParams {
    /// Creates new pagination parameters.
    ///
    /// # Arguments
    ///
    /// * `page` - The page number (1-indexed)
    /// * `per_page` - Number of items per page
    pub fn new(page: usize, per_page: usize) -> Self {
        Self { page, per_page }
    }

    /// Creates a new builder for constructing pagination parameters.
    pub fn builder() -> PaginationParamsBuilder {
        PaginationParamsBuilder::new()
    }

    /// Calculates the offset (number of items to skip) for this page.
    ///
    /// This is useful for database queries using LIMIT/OFFSET.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let params = PaginationParams::new(3, 20);
    /// assert_eq!(params.offset(), 40);  // Skip 40 items for page 3
    /// ```
    pub fn offset(&self) -> usize {
        (self.page - 1) * self.per_page
    }

    /// Paginates a vec of items according to these parameters.
    ///
    /// This helper method extracts the appropriate slice of items for this page
    /// and returns them wrapped in a [`Page`] with proper navigation metadata.
    ///
    /// # Arguments
    ///
    /// * `items` - All items to paginate
    ///
    /// # Returns
    ///
    /// A [`Page`] containing the appropriate slice of items
    ///
    /// # Example
    ///
    /// ```ignore
    /// let items: Vec<i32> = (1..=100).collect();
    /// let params = PaginationParams::new(2, 10);
    /// let page = params.paginate(items);
    ///
    /// assert_eq!(page.items, vec![11, 12, 13, 14, 15, 16, 17, 18, 19, 20]);
    /// assert_eq!(page.next_page, Some(3));
    /// assert_eq!(page.previous_page, Some(1));
    /// ```
    pub fn paginate<T>(&self, items: Vec<T>) -> Page<T>
    where
        T: Clone,
    {
        // Return empty page if items list is empty or offset is beyond the list
        if items.is_empty() || (self.offset() >= items.len()) {
            return Page::default();
        }

        // Calculate the end index, clamping to the vector length
        let end = min(self.offset() + self.per_page, items.len());
        let paginated_items = items[self.offset()..end].to_vec();

        // Build the page with proper navigation metadata
        Page::builder(paginated_items)
            .with_count(items.len())
            .with_next_page(if end < items.len() {
                Some(self.page + 1)
            } else {
                None
            })
            .with_previous_page(if self.page > 1 {
                Some(self.page - 1)
            } else {
                None
            })
            .build()
    }
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self { page: 1, per_page: 10 }
    }
}

/// Builder for constructing [`PaginationParams`] instances.
///
/// This builder allows flexible construction of pagination parameters
/// with optional overrides from defaults.
pub struct PaginationParamsBuilder {
    page: Option<usize>,
    per_page: Option<usize>,
}

impl PaginationParamsBuilder {
    /// Creates a new builder with no parameters set.
    pub fn new() -> Self {
        Self { page: None, per_page: None }
    }

    /// Sets the page number (1-indexed).
    pub fn with_page(mut self, page: usize) -> Self {
        self.page = Some(page);
        self
    }

    /// Sets the number of items per page.
    pub fn with_per_page(mut self, per_page: usize) -> Self {
        self.per_page = Some(per_page);
        self
    }

    /// Builds and returns the [`PaginationParams`].
    ///
    /// Uses defaults for any unset values (page=1, per_page=10).
    pub fn build(self) -> PaginationParams {
        PaginationParams {
            page: self.page.unwrap_or(1),
            per_page: self.per_page.unwrap_or(10),
        }
    }
}

impl Default for PaginationParamsBuilder {
    fn default() -> Self {
        Self::new()
    }
}
