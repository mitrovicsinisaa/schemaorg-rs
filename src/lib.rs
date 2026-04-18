//! # schemaorg-rs
//!
//! A high-performance Rust library for extracting and validating
//! [Schema.org](https://schema.org) structured data from HTML documents.
//!
//! ## Supported Formats
//!
//! - **JSON-LD** -- `<script type="application/ld+json">`
//! - **Microdata** -- `itemscope`/`itemprop` attributes
//! - **`RDFa` Lite** -- `vocab`/`typeof`/`property` attributes
//!
//! ## Quick Start
//!
//! ```no_run
//! # #[cfg(feature = "extraction")]
//! # {
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
//! assert_eq!(graph.nodes[0].types, vec!["Product"]);
//! # }
//! ```

// Lints
#![warn(missing_docs)]
#![warn(unreachable_pub)]
#![warn(clippy::pedantic)]
// Pedantic exceptions: these fire on every public type/method and add noise
// without meaningful benefit for a data-extraction library.
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

/* Core modules (always available) */
pub mod error;
pub mod types;

/* Extraction engine (feature-gated) */
#[cfg(feature = "extraction")]
pub mod extraction;
#[cfg(feature = "extraction")]
pub mod graph;

/* Public re-exports: always available */
pub use error::{ExtractionError, ExtractionWarning, WarningCode};
pub use types::{SchemaNode, SchemaValue, SourceFormat, SourceLocation};

/* Public re-exports: extraction feature */
#[cfg(feature = "extraction")]
pub use extraction::{
    ExtractionOutput, Extractor, JsonLdExtractor, MicrodataExtractor, RdfaLiteExtractor,
};
#[cfg(feature = "extraction")]
pub use graph::{extract_all, StructuredDataGraph};
#[cfg(feature = "extraction")]
pub use scraper::Html;
