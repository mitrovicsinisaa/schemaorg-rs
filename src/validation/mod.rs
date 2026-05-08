//! Schema.org vocabulary validation engine.
//!
//! Validates a [`StructuredDataGraph`]
//! against the official Schema.org vocabulary definitions. Produces a
//! [`ValidationResult`] containing typed diagnostics with severity levels,
//! machine-readable codes, and human-readable messages.
//!
//! # Architecture
//!
//! ```text
//! StructuredDataGraph
//!     +---- for each SchemaNode:
//!         +---- Type checker:     unknown, deprecated, pending
//!         +---- Property checker: unknown, wrong domain, superseded
//!         +---- Value checker:    type mismatch, coercion, enum validation
//! ```
//!
//! All Schema.org knowledge is compiled into static lookup functions at
//! build time -- see [`vocabulary`] for details.
//!
//! # Examples
//!
//! ```no_run
//! # #[cfg(feature = "validation")]
//! # {
//! use schemaorg_rs::{extract_all, validation};
//!
//! let html = r#"<script type="application/ld+json">{
//!   "@context": "https://schema.org",
//!   "@type": "Product",
//!   "name": "Widget"
//! }</script>"#;
//!
//! let graph = extract_all(html).unwrap();
//! let result = validation::validate(&graph);
//!
//! if result.has_errors() {
//!     for diag in result.errors() {
//!         eprintln!("{}: {}", diag.path, diag.message);
//!     }
//! }
//! # }
//! ```

pub mod diagnostics;
mod property_checker;
mod type_checker;
mod value_checker;

pub use diagnostics::{DiagnosticCode, Severity, ValidationDiagnostic};

use crate::graph::StructuredDataGraph;
use crate::types::{SchemaNode, SchemaValue};
use crate::vocabulary;

/// Result of validating a [`StructuredDataGraph`] against Schema.org.
///
/// Contains all diagnostics found during validation, accessible via
/// convenience methods for filtering by severity.
#[derive(Debug, Clone, Default)]
#[must_use]
pub struct ValidationResult {
    /// All diagnostics produced during validation.
    pub diagnostics: Vec<ValidationDiagnostic>,
}

impl ValidationResult {
    /// Returns an iterator over error-level diagnostics.
    pub fn errors(&self) -> impl Iterator<Item = &ValidationDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
    }

    /// Returns an iterator over warning-level diagnostics.
    pub fn warnings(&self) -> impl Iterator<Item = &ValidationDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
    }

    /// Returns an iterator over info-level diagnostics.
    pub fn infos(&self) -> impl Iterator<Item = &ValidationDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Info)
    }

    /// Returns `true` if any error-level diagnostics exist.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    /// Returns `true` if any warning-level diagnostics exist.
    #[must_use]
    pub fn has_warnings(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Warning)
    }

    /// Returns the total number of diagnostics.
    #[must_use]
    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    /// Returns `true` if no diagnostics were produced.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

/// Validates a [`StructuredDataGraph`] against the Schema.org vocabulary.
///
/// Checks all nodes for:
/// - Unknown or deprecated types
/// - Unknown or misplaced properties
/// - Value type mismatches
/// - Deprecated/superseded properties
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "validation")]
/// # {
/// use schemaorg_rs::{extract_all, validation};
///
/// let html = r#"<script type="application/ld+json">{
///   "@context": "https://schema.org",
///   "@type": "Produc",
///   "name": "Widget"
/// }</script>"#;
///
/// let graph = extract_all(html).unwrap();
/// let result = validation::validate(&graph);
/// assert!(result.has_errors());
/// # }
/// ```
pub fn validate(graph: &StructuredDataGraph) -> ValidationResult {
    let mut diagnostics = Vec::new();
    for node in &graph.nodes {
        let type_label = if node.types.is_empty() {
            "(unknown)".to_string()
        } else {
            node.types.join(", ")
        };
        validate_node(node, &type_label, &mut diagnostics);
    }
    ValidationResult { diagnostics }
}

/// Recursively validates a single [`SchemaNode`].
fn validate_node(node: &SchemaNode, path: &str, diagnostics: &mut Vec<ValidationDiagnostic>) {
    // 1. Check types
    for type_name in &node.types {
        type_checker::check_type(type_name, path, diagnostics);
    }

    // 2. Check properties
    for (prop_name, values) in &node.properties {
        let prop_path = format!("{path}.{prop_name}");
        property_checker::check_property(prop_name, &node.types, &prop_path, diagnostics);

        // 3. Check values
        if let Some(prop_def) = vocabulary::lookup_property(prop_name) {
            for (i, value) in values.iter().enumerate() {
                let value_path = if values.len() > 1 {
                    format!("{prop_path}[{i}]")
                } else {
                    prop_path.clone()
                };

                value_checker::check_value(
                    value,
                    prop_name,
                    prop_def.expected_types,
                    &value_path,
                    diagnostics,
                );

                // 4. Recurse into nested nodes
                if let SchemaValue::Node(nested) = value {
                    let nested_type_label = if nested.types.is_empty() {
                        format!("{value_path}.(unknown)")
                    } else {
                        format!("{value_path}.{}", nested.types.join(", "))
                    };
                    validate_node(nested, &nested_type_label, diagnostics);
                }
            }
        }
    }
}

// "Did you mean?" suggestions
/// Maximum Levenshtein distance for suggestions.
const MAX_DISTANCE: usize = 3;

/// Maximum length difference for suggestions (filters noise).
const MAX_LENGTH_DIFF: usize = 3;

/// Suggests the closest match from a list of candidates using Levenshtein distance.
///
/// Returns `None` if no candidate is within the threshold.
pub(crate) fn suggest_similar<'a>(input: &str, candidates: &'a [&str]) -> Option<&'a str> {
    let input_len = input.len();

    candidates
        .iter()
        .filter(|c| {
            let len_diff = if c.len() > input_len {
                c.len() - input_len
            } else {
                input_len - c.len()
            };
            len_diff <= MAX_LENGTH_DIFF
        })
        .map(|c| (*c, levenshtein(input, c)))
        .filter(|(_, d)| *d <= MAX_DISTANCE && *d > 0) // d > 0 to exclude exact matches
        .min_by_key(|(_, d)| *d)
        .map(|(c, _)| c)
}

/// Computes the Levenshtein edit distance between two strings.
///
/// O(n*m) time, O(min(n,m)) space using a single-row optimization.
fn levenshtein(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    // Ensure b is the shorter string for space efficiency
    let (a_bytes, b_bytes) = if a_len < b_len {
        (b.as_bytes(), a.as_bytes())
    } else {
        (a.as_bytes(), b.as_bytes())
    };

    let b_len = b_bytes.len();
    let mut row: Vec<usize> = (0..=b_len).collect();

    for (i, a_byte) in a_bytes.iter().enumerate() {
        let mut prev = i;
        row[0] = i + 1;

        for (j, b_byte) in b_bytes.iter().enumerate() {
            let cost = usize::from(!a_byte.eq_ignore_ascii_case(b_byte));
            let val = (row[j + 1] + 1).min(row[j] + 1).min(prev + cost);
            prev = row[j + 1];
            row[j + 1] = val;
        }
    }

    row[b_len]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levenshtein_basic() {
        assert_eq!(levenshtein("", ""), 0);
        assert_eq!(levenshtein("a", ""), 1);
        assert_eq!(levenshtein("", "a"), 1);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("Product", "Produc"), 1);
        assert_eq!(levenshtein("Product", "product"), 0); // case-insensitive
        assert_eq!(levenshtein("name", "namee"), 1);
    }

    #[test]
    fn suggest_similar_finds_close_match() {
        let candidates = &["Product", "Person", "Place", "Event"];
        assert_eq!(suggest_similar("Produc", candidates), Some("Product"));
        assert_eq!(suggest_similar("Prduct", candidates), Some("Product"));
        assert_eq!(suggest_similar("Perso", candidates), Some("Person"));
    }

    #[test]
    fn suggest_similar_none_for_distant() {
        let candidates = &["Product", "Person", "Place"];
        assert_eq!(suggest_similar("XYZ123", candidates), None);
    }

    #[test]
    fn suggest_similar_none_for_exact() {
        let candidates = &["Product", "Person"];
        assert_eq!(suggest_similar("Product", candidates), None);
    }
}
