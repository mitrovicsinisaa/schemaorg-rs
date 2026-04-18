//! Core data types for Schema.org structured data extraction.
//!
//! This module defines the shared data model used across all extraction
//! formats (JSON-LD, Microdata, `RDFa` Lite). The central type is
//! [`SchemaNode`], which represents a single structured data entity
//! (e.g. a `Product`, an `Offer`). Nodes contain typed [`SchemaValue`]s
//! organized in insertion-ordered property maps.
//!
//! # Data Model
//!
//! ```text
//! StructuredDataGraph
//!   └── Vec<SchemaNode>
//!         ├── types: ["Product"]
//!         ├── properties: { "name" -> [Text("Widget")],
//!         │                  "offers" -> [Node(Offer { ... })] }
//!         ├── source_format: JsonLd
//!         └── source_location: Some({ line: 3, column: 1, byte_offset: 42 })
//! ```
//!
//! # Examples
//!
//! ```
//! use schemaorg_rs::types::{SchemaNode, SchemaValue, SourceFormat};
//! use indexmap::IndexMap;
//!
//! let node = SchemaNode {
//!     types: vec!["Product".into()],
//!     properties: IndexMap::from([(
//!         "name".into(),
//!         vec![SchemaValue::Text("Widget".into())],
//!     )]),
//!     source_format: SourceFormat::JsonLd,
//!     source_location: None,
//! };
//!
//! assert_eq!(node.types, vec!["Product"]);
//! ```

use std::fmt;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Source format of the extracted structured data.
///
/// Indicates which HTML markup format a [`SchemaNode`] was extracted from.
///
/// # Examples
///
/// ```
/// use schemaorg_rs::types::SourceFormat;
///
/// let format = SourceFormat::JsonLd;
/// assert_eq!(format.to_string(), "JSON-LD");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SourceFormat {
    /// JSON-LD (`<script type="application/ld+json">`)
    JsonLd,
    /// HTML Microdata (`itemscope`, `itemprop`)
    Microdata,
    /// `RDFa` Lite 1.1 (`vocab`, `typeof`, `property`)
    RdfaLite,
}

/// Location in the original HTML document.
///
/// Used to map extracted data back to the source markup for diagnostics
/// and error reporting.
///
/// # Examples
///
/// ```
/// use schemaorg_rs::types::SourceLocation;
///
/// let loc = SourceLocation { line: 5, column: 3, byte_offset: 120 };
/// assert_eq!(loc.line, 5);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocation {
    /// 1-indexed line number.
    pub line: usize,
    /// 1-indexed column number.
    pub column: usize,
    /// 0-indexed byte offset from the start of the HTML document.
    pub byte_offset: usize,
}

/// A value within a structured data node.
///
/// Represents the different value types that a Schema.org property can hold.
/// Properties are multi-valued (stored as `Vec<SchemaValue>` in [`SchemaNode`]).
///
/// # `PartialEq` note
///
/// The `Number(f64)` variant uses `f64` partial equality via the derived impl.
/// This means `NaN != NaN`, which is acceptable for test assertions but not for
/// production equality checks.
///
/// # Examples
///
/// ```
/// use schemaorg_rs::types::SchemaValue;
///
/// let text = SchemaValue::Text("Widget".into());
/// let url = SchemaValue::Url("https://example.com".into());
/// let flag = SchemaValue::Boolean(true);
/// let price = SchemaValue::Number(29.99);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SchemaValue {
    /// Plain text content.
    Text(String),
    /// A URL value (starts with `http://`, `https://`, or `mailto:`).
    Url(String),
    /// A nested structured data node.
    Node(Box<SchemaNode>),
    /// A boolean value.
    Boolean(bool),
    /// A numeric value (IEEE 754 `f64`).
    Number(f64),
    /// A raw datetime string. Actual datetime validation happens in M2.
    DateTime(String),
}

/// A single structured data node (e.g. a `Product`, an `Offer`).
///
/// Each node represents one Schema.org entity extracted from the HTML
/// document, retaining its [`SourceFormat`] so callers can distinguish
/// which markup produced it.
///
/// # Examples
///
/// ```
/// use schemaorg_rs::types::{SchemaNode, SchemaValue, SourceFormat};
/// use indexmap::IndexMap;
///
/// let node = SchemaNode {
///     types: vec!["Product".into()],
///     properties: IndexMap::from([(
///         "name".into(),
///         vec![SchemaValue::Text("Widget".into())],
///     )]),
///     source_format: SourceFormat::JsonLd,
///     source_location: None,
/// };
///
/// assert_eq!(node.id(), None);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaNode {
    /// Schema.org type(s), e.g. `["Product", "IndividualProduct"]`.
    pub types: Vec<String>,
    /// Properties: key -> list of values (insertion-ordered).
    pub properties: IndexMap<String, Vec<SchemaValue>>,
    /// Source format that this node was extracted from.
    pub source_format: SourceFormat,
    /// Location in the original HTML document.
    pub source_location: Option<SourceLocation>,
}

impl SchemaNode {
    /// Returns the `@id` of this node, if present.
    ///
    /// # Examples
    ///
    /// ```
    /// use schemaorg_rs::types::{SchemaNode, SchemaValue, SourceFormat};
    /// use indexmap::IndexMap;
    ///
    /// let node = SchemaNode {
    ///     types: vec!["Product".into()],
    ///     properties: IndexMap::from([(
    ///         "@id".into(),
    ///         vec![SchemaValue::Text("#product1".into())],
    ///     )]),
    ///     source_format: SourceFormat::JsonLd,
    ///     source_location: None,
    /// };
    ///
    /// assert_eq!(node.id(), Some("#product1"));
    /// ```
    #[must_use]
    #[inline]
    pub fn id(&self) -> Option<&str> {
        self.properties
            .get("@id")
            .and_then(|vals| vals.first())
            .and_then(|v| match v {
                SchemaValue::Text(s) => Some(s.as_str()),
                _ => None,
            })
    }
}

impl fmt::Display for SourceFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JsonLd => write!(f, "JSON-LD"),
            Self::Microdata => write!(f, "Microdata"),
            Self::RdfaLite => write!(f, "RDFa Lite"),
        }
    }
}

impl fmt::Display for SchemaValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(s) | Self::Url(s) | Self::DateTime(s) => write!(f, "{s}"),
            Self::Boolean(b) => write!(f, "{b}"),
            Self::Number(n) => write!(f, "{n}"),
            Self::Node(n) => write!(f, "[{} node]", n.types.join(", ")),
        }
    }
}
