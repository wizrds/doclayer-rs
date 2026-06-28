//! Query expression evaluation for in-memory document filtering.
//!
//! This module provides the evaluation engine for query expressions,
//! enabling filtering and comparison operations on BSON documents.

use std::{collections::HashMap, cmp::Ordering};
use bson::{Bson, datetime::DateTime};

use doclayer_core::{
    query::{QueryVisitor, Expr, FieldOp},
    error::{DocumentStoreError, DocumentStoreResult},
};

use crate::path::BsonPath;


/// Type-erased, comparable representation of BSON values.
///
/// This enum wraps BSON values and provides comparison operations for
/// filtering queries. It normalizes numeric types to f64 for easy comparison.
///
/// # Note
///
/// This is a private implementation detail used for query evaluation.
#[derive(Debug)]
pub(crate) enum Comparable<'a> {
    /// Null value
    Null,
    /// Boolean value
    Bool(bool),
    /// Numeric value (all integers and floats normalized to f64)
    Number(f64),
    /// DateTime value
    DateTime(DateTime),
    /// String value
    String(&'a str),
    /// Array of comparable values
    Array(Vec<Comparable<'a>>),
    /// Map/Object of comparable values
    Map(HashMap<&'a str, Comparable<'a>>),
}

impl<'a> From<&'a Bson> for Comparable<'a> {
    fn from(bson: &'a Bson) -> Self {
        match bson {
            Bson::Null => Comparable::Null,
            Bson::Boolean(value) => Comparable::Bool(*value),
            Bson::Int32(value) => Comparable::Number(*value as f64),
            Bson::Int64(value) => Comparable::Number(*value as f64),
            Bson::Double(value) => Comparable::Number(*value),
            Bson::DateTime(value) => Comparable::DateTime(*value),
            Bson::String(value) => Comparable::String(value),
            Bson::Array(arr) => Comparable::Array(
                arr
                    .iter()
                    .map(Comparable::from)
                    .collect::<Vec<_>>()
            ),
            Bson::Document(doc) => Comparable::Map(
                doc
                    .iter()
                    .map(|(k, v)| (k.as_str(), Comparable::from(v)))
                    .collect::<HashMap<_, _>>()
            ),
            _ => Comparable::Null, // Other types are not comparable
        }
    }
}

impl<'a> PartialEq for Comparable<'a> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Comparable::Null, Comparable::Null) => true,
            (Comparable::Bool(a), Comparable::Bool(b)) => a == b,
            (Comparable::Number(a), Comparable::Number(b)) => a == b,
            (Comparable::DateTime(a), Comparable::DateTime(b)) => a == b,
            (Comparable::String(a), Comparable::String(b)) => a == b,
            (Comparable::Array(a), Comparable::Array(b)) => a == b,
            (Comparable::Map(a), Comparable::Map(b)) => a == b,
            _ => false,
        }
    }
}

impl<'a> PartialOrd for Comparable<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Comparable::Bool(a), Comparable::Bool(b)) => a.partial_cmp(b),
            (Comparable::Number(a), Comparable::Number(b)) => a.partial_cmp(b),
            (Comparable::DateTime(a), Comparable::DateTime(b)) => a.partial_cmp(b),
            (Comparable::String(a), Comparable::String(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}

pub(crate) struct DocumentEvaluator<'a> {
    document: &'a Bson,
}

impl<'a> DocumentEvaluator<'a> {
    pub fn new(document: &'a Bson) -> Self {
        Self { document }
    }

    pub fn evaluate(&mut self, expr: &Expr) -> DocumentStoreResult<bool> {
        self.visit_expr(expr)
    }
}

impl<'a> QueryVisitor for DocumentEvaluator<'a> {
    type Output = bool;
    type Error = DocumentStoreError;

    fn visit_and(&mut self, exprs: &[Expr]) -> Result<Self::Output, Self::Error> {
        for expr in exprs {
            if !self.visit_expr(expr)? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn visit_or(&mut self, exprs: &[Expr]) -> Result<Self::Output, Self::Error> {
        for expr in exprs {
            if self.visit_expr(expr)? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn visit_not(&mut self, expr: &Expr) -> Result<Self::Output, Self::Error> {
        Ok(!self.visit_expr(expr)?)
    }

    fn visit_exists(&mut self, field: &str, should_exist: bool) -> Result<Self::Output, Self::Error> {
        Ok(BsonPath::new(field).resolve(self.document).is_some() == should_exist)
    }

    fn visit_field(&mut self, field: &str, op: &FieldOp, value: &Bson) -> Result<Self::Output, Self::Error> {
        match BsonPath::new(field).resolve(self.document) {
            Some(field_value) => match op {
                FieldOp::Eq => Ok(Comparable::from(field_value) == Comparable::from(value)),
                FieldOp::Ne => Ok(Comparable::from(field_value) != Comparable::from(value)),
                FieldOp::Gt | FieldOp::Gte | FieldOp::Lt | FieldOp::Lte => {
                    match Comparable::from(field_value).partial_cmp(&Comparable::from(value)) {
                        Some(ordering) => Ok(match op {
                            FieldOp::Gt => ordering == Ordering::Greater,
                            FieldOp::Gte => ordering == Ordering::Greater || ordering == Ordering::Equal,
                            FieldOp::Lt => ordering == Ordering::Less,
                            FieldOp::Lte => ordering == Ordering::Less || ordering == Ordering::Equal,
                            _ => unreachable!(),
                        }),
                        None => Ok(false),
                    }
                },
                FieldOp::Contains => match Comparable::from(field_value) {
                    Comparable::Array(array) => Ok(
                        array
                            .iter()
                            .any(|item| item == &Comparable::from(value))
                    ),
                    Comparable::String(left) => match Comparable::from(value) {
                        Comparable::String(right) => Ok(left.contains(right)),
                        _ => Ok(false),
                    },
                    _ => Ok(false),
                },
                FieldOp::NotContains => match Comparable::from(field_value) {
                    Comparable::Array(array) => Ok(
                        !array
                            .iter()
                            .any(|item| item == &Comparable::from(value))
                    ),
                    Comparable::String(left) => match Comparable::from(value) {
                        Comparable::String(right) => Ok(!left.contains(right)),
                        _ => Ok(true),
                    },
                    _ => Ok(true),
                },
                FieldOp::StartsWith => match (Comparable::from(field_value), Comparable::from(value)) {
                    (Comparable::String(left), Comparable::String(right)) => Ok(left.starts_with(right)),
                    _ => Ok(false),
                },
                FieldOp::EndsWith => match (Comparable::from(field_value), Comparable::from(value)) {
                    (Comparable::String(left), Comparable::String(right)) => Ok(left.ends_with(right)),
                    _ => Ok(false),
                },
                FieldOp::AnyOf => match (Comparable::from(field_value), Comparable::from(value)) {
                    (Comparable::Array(array), Comparable::Array(values)) => {
                        for val in values {
                            if array.iter().any(|item| item == &val) {
                                return Ok(true);
                            }
                        }
                        Ok(false)
                    },
                    (Comparable::Array(array), single_value) => {
                        for item in array {
                            if item == single_value {
                                return Ok(true);
                            }
                        }
                        Ok(false)
                    },
                    (single_value, Comparable::Array(values)) => {
                        for val in values {
                            if val == single_value {
                                return Ok(true);
                            }
                        }
                        Ok(false)
                    },
                    _ => Ok(false),
                },
                FieldOp::NoneOf => match (Comparable::from(field_value), Comparable::from(value)) {
                    (Comparable::Array(array), Comparable::Array(values)) => {
                        for val in values {
                            if array.iter().any(|item| item == &val) {
                                return Ok(false);
                            }
                        }
                        Ok(true)
                    },
                    (Comparable::Array(array), single_value) => {
                        for item in array {
                            if item == single_value {
                                return Ok(false);
                            }
                        }
                        Ok(true)
                    },
                    (single_value, Comparable::Array(values)) => {
                        for val in values {
                            if val == single_value {
                                return Ok(false);
                            }
                        }
                        Ok(true)
                    },
                    _ => Ok(true),
                },
            },
            None => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use bson::{Bson, doc};
    use doclayer_core::query::{Expr, FieldOp};
    use super::DocumentEvaluator;

    fn eval(document: bson::Document, expr: Expr) -> bool {
        DocumentEvaluator::new(&Bson::Document(document))
            .evaluate(&expr)
            .unwrap()
    }

    fn field(field: &str, op: FieldOp, value: impl Into<Bson>) -> Expr {
        Expr::Field {
            field: field.to_string(),
            op,
            value: value.into(),
        }
    }

    #[test]
    fn eq_top_level_nested_document_matches_exact_value() {
        let document = doc! { "address": { "city": "Denver", "zip": "80201" } };

        assert!(eval(
            document,
            field("address", FieldOp::Eq, doc! { "city": "Denver", "zip": "80201" }),
        ));
    }

    #[test]
    fn eq_top_level_nested_document_does_not_match_partial_value() {
        let document = doc! { "address": { "city": "Denver", "zip": "80201" } };

        assert!(!eval(
            document,
            field("address", FieldOp::Eq, doc! { "city": "Denver" }),
        ));
    }

    #[test]
    fn eq_dot_notation_resolves_nested_field() {
        let document = doc! { "address": { "city": "Denver", "zip": "80201" } };

        assert!(eval(document, field("address.city", FieldOp::Eq, "Denver")));
    }

    #[test]
    fn eq_dot_notation_returns_false_for_missing_nested_field() {
        let document = doc! { "address": { "city": "Denver" } };

        assert!(!eval(document, field("address.zip", FieldOp::Eq, "80201")));
    }

    #[test]
    fn eq_dot_notation_returns_false_when_intermediate_key_is_missing() {
        let document = doc! { "name": "Alice" };

        assert!(!eval(document, field("address.city", FieldOp::Eq, "Denver")));
    }

    #[test]
    fn gt_dot_notation_compares_nested_numeric_field() {
        let document = doc! { "stats": { "score": 95_i32 } };

        assert!(eval(document, field("stats.score", FieldOp::Gt, Bson::Int32(90))));
    }

    #[test]
    fn contains_dot_notation_checks_nested_array_membership() {
        let document = doc! { "meta": { "tags": ["rust", "async", "bson"] } };

        assert!(eval(document, field("meta.tags", FieldOp::Contains, "rust")));
    }

    #[test]
    fn not_contains_dot_notation_checks_nested_array_absence() {
        let document = doc! { "meta": { "tags": ["rust", "async", "bson"] } };

        assert!(!eval(document, field("meta.tags", FieldOp::Contains, "python")));
    }

    #[test]
    fn eq_deeply_nested_dot_notation_resolves_three_levels() {
        let document = doc! { "a": { "b": { "c": "found" } } };

        assert!(eval(document, field("a.b.c", FieldOp::Eq, "found")));
    }
}
