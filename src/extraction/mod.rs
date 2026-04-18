//! Structured data extractors for JSON-LD, Microdata, and `RDFa` Lite.
//!
//! This module provides the [`Extractor`] trait and concrete implementations
//! for each structured data format:
//!
//! - [`JsonLdExtractor`] -- `<script type="application/ld+json">` tags
//! - [`MicrodataExtractor`] -- `itemscope`/`itemprop` attributes
//! - [`RdfaLiteExtractor`] -- `vocab`/`typeof`/`property` attributes
//!
//! Each extractor produces an [`ExtractionOutput`] containing extracted
//! [`SchemaNode`]s and any non-fatal warnings. For most use cases, prefer
//! [`extract_all`](crate::graph::extract_all) which runs all extractors
//! and merges results.
//!
//! # Examples
//!
//! ```
//! use schemaorg_rs::extraction::{Extractor, JsonLdExtractor};
//!
//! let html = r#"<html><head>
//! <script type="application/ld+json">{
//!   "@context": "https://schema.org",
//!   "@type": "Product",
//!   "name": "Widget"
//! }</script>
//! </head></html>"#;
//!
//! let output = JsonLdExtractor.extract(html).unwrap();
//! assert_eq!(output.nodes.len(), 1);
//! ```

mod jsonld;
mod microdata;
mod rdfa;

pub use jsonld::JsonLdExtractor;
pub use microdata::MicrodataExtractor;
pub use rdfa::RdfaLiteExtractor;

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::error::{ExtractionError, ExtractionWarning};
use crate::types::{SchemaNode, SchemaValue};

/// Output from a single extractor run.
///
/// Contains the extracted nodes and any non-fatal warnings encountered
/// during extraction.
///
/// # Examples
///
/// ```
/// use schemaorg_rs::extraction::{Extractor, JsonLdExtractor};
///
/// let output = JsonLdExtractor.extract("<html></html>").unwrap();
/// assert!(output.nodes.is_empty());
/// ```
#[must_use]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExtractionOutput {
    /// Extracted structured data nodes.
    pub nodes: Vec<SchemaNode>,
    /// Non-fatal warnings encountered during extraction.
    pub warnings: Vec<ExtractionWarning>,
}

/// Trait implemented by each extraction format (JSON-LD, Microdata, `RDFa`).
///
/// Provides a unified interface for extracting structured data from raw HTML.
/// Each implementation parses the HTML internally using `scraper`.
///
/// For better performance when running multiple extractors, use the
/// format-specific `extract_from_document()` methods which accept a
/// pre-parsed `scraper::Html` document.
///
/// # Examples
///
/// ```
/// use schemaorg_rs::extraction::{Extractor, MicrodataExtractor};
///
/// let html = r#"<html><body>
/// <div itemscope itemtype="https://schema.org/Product">
///   <span itemprop="name">Widget</span>
/// </div>
/// </body></html>"#;
///
/// let output = MicrodataExtractor.extract(html).unwrap();
/// assert_eq!(output.nodes[0].types, vec!["Product"]);
/// ```
pub trait Extractor: Send + Sync {
    /// Extracts structured data nodes from an HTML document.
    ///
    /// # Errors
    ///
    /// Returns [`ExtractionError`] if a fatal error prevents extraction.
    /// Most issues are captured as warnings in the returned
    /// [`ExtractionOutput`] instead.
    fn extract(&self, html: &str) -> Result<ExtractionOutput, ExtractionError>;
}

/////////////////////////////////////////////////////////////////////////////
// Shared helpers used by all three extractors
/////////////////////////////////////////////////////////////////////////////

/// Schema.org URL prefixes to strip from type names and property URIs.
const SCHEMA_PREFIXES: &[&str] = &["https://schema.org/", "http://schema.org/", "schema:"];

/// Strips Schema.org URL prefixes from a type or property name.
///
/// Returns the local name with the prefix removed, or the original
/// string if no known prefix is present. Returns `Cow::Borrowed` when
/// no prefix matched (zero-alloc fast path for plain terms like `"Product"`).
///
/// `https://schema.org/Product` -> `Product`,
/// `http://schema.org/Product` -> `Product`, `schema:Product` -> `Product`.
pub(crate) fn strip_schema_prefix(name: &str) -> Cow<'_, str> {
    for prefix in SCHEMA_PREFIXES {
        if let Some(stripped) = name.strip_prefix(prefix) {
            return Cow::Owned(stripped.to_string());
        }
    }
    Cow::Borrowed(name)
}

/// Classifies a text string as [`SchemaValue::Url`], [`SchemaValue::DateTime`],
/// or [`SchemaValue::Text`].
///
/// Uses heuristics:
/// - Starts with `http://`, `https://`, or `mailto:` -> `Url`
/// - Matches ISO 8601 date pattern (`YYYY-MM-DD`) -> `DateTime`
/// - Everything else -> `Text`
pub(crate) fn classify_text_value(s: &str) -> SchemaValue {
    if s.starts_with("http://") || s.starts_with("https://") || s.starts_with("mailto:") {
        return SchemaValue::Url(s.to_string());
    }
    // Dates always start with a digit; skip the full check for plain text
    if s.as_bytes().first().is_some_and(u8::is_ascii_digit) && is_iso_datetime(s) {
        return SchemaValue::DateTime(s.to_string());
    }
    SchemaValue::Text(s.to_string())
}

/// Checks if a string matches an ISO 8601 date/datetime pattern (`YYYY-MM-DD...`).
///
/// Validates the structural pattern and basic range checks (month 01-12,
/// day 01-31). Full date validation (leap years, etc.) is deferred to M2.
pub(crate) fn is_iso_datetime(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() < 10 {
        return false;
    }

    let valid_pattern = bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit);

    if !valid_pattern {
        return false;
    }

    // Range checks: month 01-12, day 01-31
    let month = (bytes[5] - b'0') * 10 + (bytes[6] - b'0');
    let day = (bytes[8] - b'0') * 10 + (bytes[9] - b'0');

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return false;
    }

    // Must be exactly a date, or followed by a valid ISO 8601 time separator
    if bytes.len() == 10 {
        return true;
    }
    matches!(bytes[10], b'T' | b't' | b'Z' | b'+' | b'-')
}

#[cfg(test)]
mod common_tests {
    use super::*;

    #[test]
    fn strip_schema_prefixes() {
        assert_eq!(strip_schema_prefix("Product").as_ref(), "Product");
        assert_eq!(strip_schema_prefix("https://schema.org/Product").as_ref(), "Product");
        assert_eq!(strip_schema_prefix("http://schema.org/Product").as_ref(), "Product");
        assert_eq!(strip_schema_prefix("schema:Product").as_ref(), "Product");

        // No-prefix path returns Cow::Borrowed (no allocation)
        assert!(matches!(strip_schema_prefix("Product"), Cow::Borrowed(_)));
        assert!(matches!(strip_schema_prefix("https://schema.org/Product"), Cow::Owned(_)));
    }

    #[test]
    fn classify_text_values() {
        assert_eq!(
            classify_text_value("hello"),
            SchemaValue::Text("hello".into())
        );
        assert_eq!(
            classify_text_value("https://example.com"),
            SchemaValue::Url("https://example.com".into())
        );
        assert_eq!(
            classify_text_value("2024-01-15"),
            SchemaValue::DateTime("2024-01-15".into())
        );
        assert_eq!(
            classify_text_value("2024-01-15T10:30:00Z"),
            SchemaValue::DateTime("2024-01-15T10:30:00Z".into())
        );
    }

    #[test]
    fn iso_datetime_detection() {
        assert!(is_iso_datetime("2024-01-15"));
        assert!(is_iso_datetime("2024-01-15T10:30:00"));
        assert!(is_iso_datetime("2024-01-15T10:30:00Z"));
        assert!(!is_iso_datetime("hello"));
        assert!(!is_iso_datetime("2024"));
        assert!(!is_iso_datetime("not-a-date"));
        // Invalid ranges
        assert!(!is_iso_datetime("2024-13-15"));
        assert!(!is_iso_datetime("2024-00-15"));
        assert!(!is_iso_datetime("2024-01-00"));
        assert!(!is_iso_datetime("2024-01-32"));
        // Valid edge cases
        assert!(is_iso_datetime("2024-01-01"));
        assert!(is_iso_datetime("2024-12-31"));
        // Timezone offsets
        assert!(is_iso_datetime("2024-01-15+02:00"));
        assert!(is_iso_datetime("2024-01-15-05:00"));
        assert!(is_iso_datetime("2024-01-15Z"));
        // Trailing T with no time part is accepted (valid ISO 8601 date indicator)
        assert!(is_iso_datetime("2024-01-15T"));
        // Space is a valid ISO 8601 separator but we reject it to avoid
        // false positives on sentences that start with a date.
        assert!(!is_iso_datetime("2024-01-15 10:30:00"));
        assert!(!is_iso_datetime("2024-01-15 is the deadline"));
        assert!(!is_iso_datetime("2024-01-15abc"));
        assert!(!is_iso_datetime("2024-01-15."));
    }
}
