//! BSON value sanitization for MongoDB compatibility.
//!
//! This module handles sanitization and restoration of BSON values to make them
//! compatible with MongoDB field names and queries. MongoDB restricts field names
//! (keys) from containing certain characters like dots and dollar signs, which are
//! used in MongoDB query syntax.

use bson::Bson;


/// Sanitizes and restores BSON values to handle MongoDB field name restrictions.
///
/// MongoDB does not allow field names (document keys) to contain:
/// - Dots (`.`) - used for nested field access in queries
/// - Dollar signs (`$`) - used for operators in queries
/// - Null bytes (`\0`) - field name terminators
///
/// This sanitizer replaces problematic characters with safe escaped versions
/// that can be safely stored and retrieved.
pub(crate) struct ValueSanitizer;

impl ValueSanitizer {
    /// Character replacements for sanitization
    const REPLACEMENTS: [(&'static str, &'static str); 3] = [
        (".", "__dot__"),
        ("$", "__dollar__"),
        ("\0", "__null__"),
    ];

    /// Recursively sanitizes a BSON value, replacing problematic characters in keys and strings.
    ///
    /// This function processes:
    /// - Strings: replaces problematic characters
    /// - Arrays: recursively sanitizes each element
    /// - Documents: recursively sanitizes keys and values
    /// - Other types: returned as-is
    pub(crate) fn sanitize_value(value: &Bson) -> Bson {
        match value {
            Bson::String(s) => Bson::String(Self::sanitize_string(s)),
            Bson::Array(arr) => Bson::Array(
                arr
                    .iter()
                    .map(Self::sanitize_value)
                    .collect(),
            ),
            Bson::Document(doc) => Bson::Document(
                doc.iter()
                    .map(|(k, v)| (Self::sanitize_string(k), Self::sanitize_value(v)))
                    .collect(),
            ),
            _ => value.clone(),
        }
    }

    /// Sanitizes a string by replacing problematic characters with safe escaped versions.
    pub(crate) fn sanitize_string(input: &str) -> String {
        let mut sanitized = input.to_string();
        for (target, replacement) in Self::REPLACEMENTS.iter() {
            sanitized = sanitized.replace(*target, *replacement);
        }
        sanitized
    }

    /// Recursively restores a BSON value, reverting sanitization transformations.
    ///
    /// This is the inverse of `sanitize_value` and should be called on values
    /// retrieved from MongoDB to restore the original field names and content.
    pub(crate) fn restore_value(value: &Bson) -> Bson {
        match value {
            Bson::String(s) => Bson::String(Self::restore_string(s)),
            Bson::Array(arr) => Bson::Array(
                arr
                    .iter()
                    .map(Self::restore_value)
                    .collect(),
            ),
            Bson::Document(doc) => Bson::Document(
                doc.iter()
                    .map(|(k, v)| (Self::restore_string(k), Self::restore_value(v)))
                    .collect(),
            ),
            _ => value.clone(),
        }
    }

    /// Restores a string by reverting sanitization escapes.
    pub(crate) fn restore_string(input: &str) -> String {
        let mut restored = input.to_string();
        for (target, replacement) in Self::REPLACEMENTS.iter().rev() {
            restored = restored.replace(*replacement, *target);
        }
        restored
    }
}
