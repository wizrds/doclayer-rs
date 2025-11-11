//! Query translation from doclayer AST to MongoDB query syntax.
//!
//! This module translates doclayer's abstract query expressions into
//! MongoDB BSON documents for execution by the MongoDB query engine.

use bson::{Document, Bson, doc};

use doclayer_core::{
    query::{QueryVisitor, Expr, FieldOp},
    error::DocumentStoreError,
};


/// Translates doclayer query expressions into MongoDB query documents.
///
/// This struct implements the [`QueryVisitor`] trait to convert abstract
/// query expressions into MongoDB's native BSON query syntax.
pub(crate) struct MongoQueryTranslator;

impl QueryVisitor for MongoQueryTranslator {
    type Output = Document;
    type Error = DocumentStoreError;

    fn visit_and(&mut self, exprs: &[Expr]) -> Result<Self::Output, Self::Error> {
        Ok(doc! {
            "$and": exprs
                .iter()
                .map(|expr| self.visit_expr(expr))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    fn visit_or(&mut self, exprs: &[Expr]) -> Result<Self::Output, Self::Error> {
        Ok(doc! {
            "$or": exprs
                .iter()
                .map(|expr| self.visit_expr(expr))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    fn visit_not(&mut self, expr: &Expr) -> Result<Self::Output, Self::Error> {
        Ok(doc! {
            "$not": self.visit_expr(expr)?,
        })
    }

    fn visit_exists(&mut self, field: &str, should_exist: bool) -> Result<Self::Output, Self::Error> {
        Ok(doc! {
            field: { "$exists": should_exist },
        })
    }

    fn visit_field(&mut self, field: &str, op: &FieldOp, value: &Bson) -> Result<Self::Output, Self::Error> {
        Ok(doc! {
            field: match op {
                FieldOp::Eq => doc! { "$eq": value },
                FieldOp::Ne => doc! { "$ne": value },
                FieldOp::Gt => doc! { "$gt": value },
                FieldOp::Gte => doc! { "$gte": value },
                FieldOp::Lt => doc! { "$lt": value },
                FieldOp::Lte => doc! { "$lte": value },
                FieldOp::Contains => match value {
                    Bson::String(s) => doc! { "$regex": format!(".*{}.*", s), "$options": "i" },
                    Bson::Array(arr) => doc! { "$all": arr },
                    _ => return Err(DocumentStoreError::Backend("Contains operator requires a string or array value".to_string())),
                },
                FieldOp::NotContains => match value {
                    Bson::String(s) => doc! { "$not": { "$regex": format!(".*{}.*", s), "$options": "i" } },
                    Bson::Array(arr) => doc! { "$nin": arr },
                    _ => return Err(DocumentStoreError::Backend("NotContains operator requires a string or array value".to_string())),
                },
                FieldOp::StartsWith => match value {
                    Bson::String(s) => doc! { "$regex": format!("^{}", s), "$options": "i" },
                    _ => return Err(DocumentStoreError::Backend("StartsWith operator requires a string value".to_string())),
                },
                FieldOp::EndsWith => match value {
                    Bson::String(s) => doc! { "$regex": format!("{}$", s), "$options": "i" },
                    _ => return Err(DocumentStoreError::Backend("EndsWith operator requires a string value".to_string())),
                },
                FieldOp::AnyOf => doc! { "$in": value },
                FieldOp::NoneOf => doc! { "$nin": value },
            }
        })
    }
}
