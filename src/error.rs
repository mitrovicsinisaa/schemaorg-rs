//! Error and warning types for structured data extraction.
//!
//! This module provides two levels of diagnostics:
//!
//! - [`ExtractionError`] -- fatal errors that prevent extraction from completing.
//!   These are rare; most issues are captured as warnings instead.
//! - [`ExtractionWarning`] -- non-fatal issues that did not prevent extraction
//!   but may affect data quality. Each warning carries a machine-readable
//!   [`WarningCode`] for programmatic handling.
//!
//! # Design
//!
//! The extraction pipeline is designed to be lenient: individual format
//! failures (e.g. invalid JSON in a `<script>` tag) are captured as warnings
//! so that other formats can still produce results. Only truly unrecoverable
//! errors propagate as [`ExtractionError`].
//!
//! # Examples
//!
//! ```
//! use schemaorg_rs::error::{ExtractionWarning, WarningCode};
//!
//! let warning = ExtractionWarning {
//!     message: "empty JSON-LD script tag".into(),
//!     source_location: None,
//!     code: WarningCode::MalformedJsonLd,
//! };
//!
//! assert_eq!(warning.code, WarningCode::MalformedJsonLd);
//! ```

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::types::SourceLocation;
use thiserror::Error;

/// Fatal extraction errors that prevent further processing.
///
/// Most extraction issues are captured as [`ExtractionWarning`]s instead.
/// This enum is reserved for errors that make it impossible to produce
/// any meaningful output.
///
/// # Examples
///
/// ```
/// use schemaorg_rs::error::ExtractionError;
///
/// let err = ExtractionError::Internal("unexpected state".into());
/// assert_eq!(err.to_string(), "extraction failed: unexpected state");
/// ```
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ExtractionError {
    /// A JSON-LD script body contained invalid JSON.
    ///
    /// Note: This variant is available for library consumers who parse
    /// JSON-LD outside the extractor pipeline. The built-in extractors
    /// convert JSON parse failures into warnings instead.
    #[cfg(feature = "extraction")]
    #[error("invalid JSON in <script type=\"application/ld+json\">")]
    JsonParse(#[source] serde_json::Error),

    /// An internal error occurred during extraction.
    #[error("extraction failed: {0}")]
    Internal(String),
}

/// A non-fatal warning produced during extraction.
///
/// Warnings indicate issues that did not prevent extraction but may
/// affect data quality (e.g. malformed markup, unresolvable references).
///
/// # Examples
///
/// ```
/// use schemaorg_rs::error::{ExtractionWarning, WarningCode};
///
/// let warning = ExtractionWarning {
/// message: "JSON-LD object has no @type".into(),
/// source_location: None,
/// code: WarningCode::EmptyType,
/// };
///
/// assert_eq!(warning.code, WarningCode::EmptyType);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractionWarning {
    /// Human-readable description of the warning.
    pub message: String,
    /// Location in the HTML where the issue was found.
    pub source_location: Option<SourceLocation>,
    /// Machine-readable warning code.
    pub code: WarningCode,
}

/// Machine-readable warning codes.
///
/// Each code corresponds to a specific class of extraction issue.
/// Use these for programmatic filtering and reporting.
///
/// # Examples
///
/// ```
/// use schemaorg_rs::error::WarningCode;
///
/// let code = WarningCode::MalformedJsonLd;
/// assert_eq!(code.to_string(), "malformed-json-ld");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum WarningCode {
    /// Invalid or unparseable JSON-LD content.
    MalformedJsonLd,
    /// Invalid or incomplete Microdata markup.
    MalformedMicrodata,
    /// Invalid or incomplete `RDFa` markup.
    MalformedRdfa,
    /// An `@id` reference could not be resolved.
    UnresolvableReference,
    /// A structured data node has no `@type` / `itemtype` / `typeof`.
    EmptyType,
    /// Multiple nodes share the same `@id`.
    DuplicateId,
    /// An entire extractor failed (captured so other formats still run).
    ExtractorFailed,
}

impl fmt::Display for WarningCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedJsonLd => write!(f, "malformed-json-ld"),
            Self::MalformedMicrodata => write!(f, "malformed-microdata"),
            Self::MalformedRdfa => write!(f, "malformed-rdfa"),
            Self::UnresolvableReference => write!(f, "unresolvable-reference"),
            Self::EmptyType => write!(f, "empty-type"),
            Self::DuplicateId => write!(f, "duplicate-id"),
            Self::ExtractorFailed => write!(f, "extractor-failed"),
        }
    }
}
