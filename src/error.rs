//! Error and warning types for structured data extraction.

use serde::{Deserialize, Serialize};

use crate::types::SourceLocation;
use thiserror::Error;

/// Fatal extraction errors that prevent further processing.
#[derive(Debug, Error)]
pub enum ExtractionError {
    /// The HTML document could not be parsed at all.
    #[error("failed to parse HTML: {0}")]
    HtmlParse(String),

    /// A JSON-LD script body contained invalid JSON.
    #[error("invalid JSON in <script type=\"application/ld+json\">: {0}")]
    JsonParse(#[from] serde_json::Error),

    /// An internal error occurred during extraction.
    #[error("extraction failed: {0}")]
    Internal(String),
}

/// A non-fatal warning produced during extraction.
///
/// Warnings indicate issues that did not prevent extraction but may
/// affect data quality (e.g. malformed markup, unresolvable references).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractionWarning {
    /// Human-readable description of the warning.
    pub message: String,
    /// Location in the HTML where the issue was found.
    pub source_location: Option<SourceLocation>,
    /// Machine-readable warning code.
    pub code: WarningCode,
}

/// Machine-readable warning codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WarningCode {
    /// Invalid or unparseable JSON-LD content.
    MalformedJsonLd,
    /// Invalid or incomplete Microdata markup.
    MalformedMicrodata,
    /// Invalid or incomplete RDFa markup.
    MalformedRdfa,
    /// An `@id` reference could not be resolved.
    UnresolvableReference,
    /// A structured data node has no `@type` / `itemtype` / `typeof`.
    EmptyType,
    /// Multiple nodes share the same `@id`.
    DuplicateId,
}
