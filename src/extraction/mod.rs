//! Structured data extractors for JSON-LD, Microdata, and RDFa Lite.

mod jsonld;
mod microdata;
mod rdfa;

pub use jsonld::JsonLdExtractor;
pub use microdata::MicrodataExtractor;
pub use rdfa::RdfaLiteExtractor;

use crate::error::{ExtractionError, ExtractionWarning};
use crate::types::SchemaNode;

/// Output from a single extractor run.
#[derive(Debug, Clone)]
pub struct ExtractionOutput {
    /// Extracted structured data nodes.
    pub nodes: Vec<SchemaNode>,
    /// Non-fatal warnings encountered during extraction.
    pub warnings: Vec<ExtractionWarning>,
}

/// Trait implemented by each extraction format (JSON-LD, Microdata, RDFa).
pub trait Extractor {
    /// Extract structured data nodes from an HTML document.
    fn extract(&self, html: &str) -> Result<ExtractionOutput, ExtractionError>;
}
