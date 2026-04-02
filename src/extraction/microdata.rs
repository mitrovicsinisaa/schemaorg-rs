//! Microdata extractor — stub for Phase 1b.

use super::{ExtractionOutput, Extractor};
use crate::error::ExtractionError;

/// Extracts Schema.org structured data from HTML Microdata attributes.
pub struct MicrodataExtractor;

impl Extractor for MicrodataExtractor {
    fn extract(&self, _html: &str) -> Result<ExtractionOutput, ExtractionError> {
        Ok(ExtractionOutput {
            nodes: Vec::new(),
            warnings: Vec::new(),
        })
    }
}
