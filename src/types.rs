//! Core data types for Schema.org structured data extraction.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Source format of the extracted structured data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceFormat {
    /// JSON-LD (`<script type="application/ld+json">`)
    JsonLd,
    /// HTML Microdata (`itemscope`, `itemprop`)
    Microdata,
    /// RDFa Lite 1.1 (`vocab`, `typeof`, `property`)
    RdfaLite,
}

/// Location in the original HTML document.
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
/// # `PartialEq` note
///
/// The `Number(f64)` variant uses `f64` partial equality via the derived impl.
/// This means `NaN != NaN`, which is acceptable for test assertions but not for
/// production equality checks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SchemaValue {
    /// Plain text content.
    Text(String),
    /// A URL value (starts with `http://`, `https://`, or `mailto:`).
    Url(String),
    /// A nested structured data node.
    Node(Box<SchemaNode>),
    /// A boolean value.
    Boolean(bool),
    /// A numeric value (IEEE 754 f64).
    Number(f64),
    /// A raw datetime string. Actual datetime validation happens in M2.
    DateTime(String),
}

/// A single structured data node (e.g. a `Product`, an `Offer`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaNode {
    /// Schema.org type(s), e.g. `["Product", "IndividualProduct"]`.
    pub types: Vec<String>,
    /// Properties: key → list of values (insertion-ordered).
    pub properties: IndexMap<String, Vec<SchemaValue>>,
    /// Source format that this node was extracted from.
    pub source_format: SourceFormat,
    /// Location in the original HTML document.
    pub source_location: Option<SourceLocation>,
}

impl SchemaNode {
    /// Returns the `@id` of this node, if present.
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
