//! Unified structured data graph combining all extraction formats.
//!
//! This module provides the primary entry point [`extract_all`] which runs
//! all three extractors (JSON-LD, Microdata, `RDFa` Lite) against an HTML
//! document and merges the results into a single [`StructuredDataGraph`].
//!
//! # Pipeline
//!
//! 1. Parse the HTML once using `scraper::Html`
//! 2. Run each extractor against the parsed DOM
//! 3. Merge all nodes and warnings into a single graph
//! 4. Individual extractor failures are captured as warnings (not errors)
//!
//! # Examples
//!
//! ```
//! use schemaorg_rs::extract_all;
//!
//! let html = r#"<html><head>
//! <script type="application/ld+json">{
//!   "@context": "https://schema.org",
//!   "@type": "Product",
//!   "name": "Widget"
//! }</script>
//! </head></html>"#;
//!
//! let graph = extract_all(html).unwrap();
//! assert_eq!(graph.nodes.len(), 1);
//! assert_eq!(graph.nodes[0].types, vec!["Product"]);
//! assert!(graph.warnings.is_empty());
//! ```

use serde::{Deserialize, Serialize};

use crate::error::{ExtractionError, ExtractionWarning, WarningCode};
use crate::extraction::{ExtractionOutput, JsonLdExtractor, MicrodataExtractor, RdfaLiteExtractor};
use crate::types::SchemaNode;

/// A unified graph of all structured data extracted from an HTML document.
///
/// Combines results from JSON-LD, Microdata, and `RDFa` Lite extractors.
/// Each node retains its [`SourceFormat`](crate::types::SourceFormat) so
/// callers can distinguish which markup produced it.
///
/// # Examples
///
/// ```
/// use schemaorg_rs::extract_all;
///
/// let graph = extract_all("<html></html>").unwrap();
/// assert!(graph.nodes.is_empty());
/// assert!(graph.warnings.is_empty());
/// ```
#[must_use]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructuredDataGraph {
    /// All extracted structured data nodes.
    pub nodes: Vec<SchemaNode>,
    /// Non-fatal warnings from all extractors.
    pub warnings: Vec<ExtractionWarning>,
}

/// Extracts all structured data from an HTML document.
///
/// Runs JSON-LD, Microdata, and `RDFa` Lite extractors and merges the
/// results into a single [`StructuredDataGraph`].
///
/// Individual extractor failures are captured as warnings; only truly
/// fatal errors (e.g. inability to parse HTML) propagate as errors.
///
/// # Errors
///
/// Returns [`ExtractionError::Internal`] if a fatal, unrecoverable error
/// occurs during HTML parsing. In practice this function is infallible:
/// individual format failures are captured as
/// [`WarningCode::ExtractorFailed`] warnings.
///
/// # Examples
///
/// ```
/// use schemaorg_rs::extract_all;
///
/// let html = r#"<html><head>
/// <script type="application/ld+json">{
///   "@context": "https://schema.org",
///   "@type": "Product",
///   "name": "Widget"
/// }</script>
/// </head></html>"#;
///
/// let graph = extract_all(html).unwrap();
/// assert_eq!(graph.nodes.len(), 1);
/// assert_eq!(graph.nodes[0].types, vec!["Product"]);
/// ```
pub fn extract_all(html: &str) -> Result<StructuredDataGraph, ExtractionError> {
    let document = scraper::Html::parse_document(html);

    let mut nodes = Vec::new();
    let mut warnings = Vec::new();

    // JSON-LD needs both the parsed document and the raw HTML string
    // for source-location computation (byte offsets of <script> tags).
    collect_or_warn(
        JsonLdExtractor.extract_from_document(&document, html),
        &mut nodes,
        &mut warnings,
    );

    // Microdata and RDFa only need the parsed document.
    collect_or_warn(
        MicrodataExtractor.extract_from_document(&document),
        &mut nodes,
        &mut warnings,
    );
    collect_or_warn(
        RdfaLiteExtractor.extract_from_document(&document),
        &mut nodes,
        &mut warnings,
    );

    Ok(StructuredDataGraph { nodes, warnings })
}

/// Merges extractor output or captures failures as warnings.
fn collect_or_warn(
    result: Result<ExtractionOutput, ExtractionError>,
    nodes: &mut Vec<SchemaNode>,
    warnings: &mut Vec<ExtractionWarning>,
) {
    match result {
        Ok(output) => {
            nodes.extend(output.nodes);
            warnings.extend(output.warnings);
        }
        Err(e) => {
            warnings.push(ExtractionWarning {
                message: format!("extractor failed: {e}"),
                source_location: None,
                code: WarningCode::ExtractorFailed,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::types::{SchemaValue, SourceFormat};

    #[test]
    fn extract_all_jsonld() {
        let html = r#"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "Product",
  "name": "Test"
}</script></head></html>"#;

        let graph = extract_all(html).expect("extraction failed");
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.nodes[0].types, vec!["Product"]);
        assert_eq!(graph.nodes[0].source_format, SourceFormat::JsonLd);
        assert_eq!(
            graph.nodes[0].properties["name"],
            vec![SchemaValue::Text("Test".into())]
        );
    }

    #[test]
    fn extract_all_empty_html() {
        let graph = extract_all("<html></html>").expect("extraction failed");
        assert!(graph.nodes.is_empty());
        assert!(graph.warnings.is_empty());
    }
}
