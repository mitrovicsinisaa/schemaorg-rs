//! Unified structured data graph combining all extraction formats.

use serde::{Deserialize, Serialize};

use crate::error::{ExtractionError, ExtractionWarning};
use crate::extraction::{Extractor, JsonLdExtractor, MicrodataExtractor, RdfaLiteExtractor};
use crate::types::SchemaNode;

/// A unified graph of all structured data extracted from an HTML document.
///
/// Combines results from JSON-LD, Microdata, and RDFa Lite extractors.
/// Each node retains its [`SourceFormat`](crate::types::SourceFormat) so
/// callers can distinguish which markup produced it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredDataGraph {
    /// All extracted structured data nodes.
    pub nodes: Vec<SchemaNode>,
    /// Non-fatal warnings from all extractors.
    pub warnings: Vec<ExtractionWarning>,
}

/// Extract all structured data from an HTML document.
///
/// Runs JSON-LD, Microdata, and RDFa Lite extractors and merges the
/// results into a single [`StructuredDataGraph`].
///
/// Individual extractor failures are captured as warnings; only truly
/// fatal errors (e.g. inability to parse HTML) propagate as errors.
///
/// # Example
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
    let extractors: Vec<Box<dyn Extractor>> = vec![
        Box::new(JsonLdExtractor),
        Box::new(MicrodataExtractor),
        Box::new(RdfaLiteExtractor),
    ];

    let mut nodes = Vec::new();
    let mut warnings = Vec::new();

    for extractor in &extractors {
        let output = extractor.extract(html)?;
        nodes.extend(output.nodes);
        warnings.extend(output.warnings);
    }

    Ok(StructuredDataGraph { nodes, warnings })
}

#[cfg(test)]
mod tests {
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
