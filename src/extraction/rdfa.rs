//! RDFa Lite extractor — stub for Phase 1b.

use super::{ExtractionOutput, Extractor};
use crate::error::ExtractionError;

/// Extracts Schema.org structured data from RDFa Lite 1.1 attributes.
pub struct RdfaLiteExtractor;

impl Extractor for RdfaLiteExtractor {
    fn extract(&self, _html: &str) -> Result<ExtractionOutput, ExtractionError> {
        Ok(ExtractionOutput {
            nodes: Vec::new(),
            warnings: Vec::new(),
        })
    }
}
