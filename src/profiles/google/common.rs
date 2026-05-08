//! Shared helpers for Google Rich Results profile implementations.
//!
//! Provides reusable functions for checking required/recommended properties,
//! validating nested objects, and generating profile diagnostics.

use crate::types::{SchemaNode, SchemaValue};
use crate::validation::diagnostics::{DiagnosticCode, Severity, ValidationDiagnostic};

/// Checks that a node has a required property with a non-empty value.
///
/// Returns `Some(diagnostic)` if the property is missing or empty.
pub(crate) fn require_property(
    node: &SchemaNode,
    prop: &str,
    path: &str,
) -> Option<ValidationDiagnostic> {
    if has_non_empty_property(node, prop) {
        None
    } else {
        Some(ValidationDiagnostic {
            path: format!("{path}.{prop}"),
            severity: Severity::Error,
            code: DiagnosticCode::RequiredFieldMissing,
            message: format!("Required field '{prop}' is missing"),
            source_location: None,
        })
    }
}

/// Checks that a node has a recommended property.
///
/// Returns `Some(diagnostic)` if the property is missing.
pub(crate) fn recommend_property(
    node: &SchemaNode,
    prop: &str,
    path: &str,
) -> Option<ValidationDiagnostic> {
    if has_non_empty_property(node, prop) {
        None
    } else {
        Some(ValidationDiagnostic {
            path: format!("{path}.{prop}"),
            severity: Severity::Warning,
            code: DiagnosticCode::RecommendedFieldMissing,
            message: format!("Recommended field '{prop}' is missing"),
            source_location: None,
        })
    }
}

/// Checks that a node has at least one of the given properties.
///
/// Returns `Some(diagnostic)` if none are present.
pub(crate) fn require_one_of(
    node: &SchemaNode,
    props: &[&str],
    path: &str,
    severity: Severity,
) -> Option<ValidationDiagnostic> {
    if props.iter().any(|p| has_non_empty_property(node, p)) {
        None
    } else {
        let list = props.join("' or '");
        let code = if severity == Severity::Error {
            DiagnosticCode::RequiredFieldMissing
        } else {
            DiagnosticCode::RecommendedFieldMissing
        };
        Some(ValidationDiagnostic {
            path: path.to_string(),
            severity,
            code,
            message: format!("At least one of '{list}' should be present"),
            source_location: None,
        })
    }
}

/// Validates a nested node (e.g., Offer inside Product).
///
/// Checks the nested object for required and recommended properties.
/// Returns a list of diagnostics for the nested object.
pub(crate) fn validate_nested(
    node: &SchemaNode,
    prop: &str,
    expected_type: &str,
    required: &[&str],
    recommended: &[&str],
    path: &str,
) -> Vec<ValidationDiagnostic> {
    let mut diagnostics = Vec::new();

    let Some(values) = node.properties.get(prop) else {
        return diagnostics;
    };

    for (i, value) in values.iter().enumerate() {
        let nested_path = if values.len() > 1 {
            format!("{path}.{prop}[{i}]")
        } else {
            format!("{path}.{prop}")
        };

        if let SchemaValue::Node(nested) = value {
            // Check type if specified
            if !expected_type.is_empty()
                && !nested.types.is_empty()
                && !nested.types.iter().any(|t| t == expected_type)
            {
                // Type doesn't match but we still validate the structure
            }

            for req in required {
                if !has_non_empty_property(nested, req) {
                    diagnostics.push(ValidationDiagnostic {
                        path: format!("{nested_path}.{req}"),
                        severity: Severity::Error,
                        code: DiagnosticCode::NestedRequiredFieldMissing,
                        message: format!(
                            "Required field '{req}' is missing in nested {expected_type}"
                        ),
                        source_location: None,
                    });
                }
            }

            for rec in recommended {
                if !has_non_empty_property(nested, rec) {
                    diagnostics.push(ValidationDiagnostic {
                        path: format!("{nested_path}.{rec}"),
                        severity: Severity::Warning,
                        code: DiagnosticCode::RecommendedFieldMissing,
                        message: format!(
                            "Recommended field '{rec}' is missing in nested {expected_type}"
                        ),
                        source_location: None,
                    });
                }
            }
        }
    }

    diagnostics
}

/// Checks if a node has a property with at least one non-empty value.
pub(crate) fn has_non_empty_property(node: &SchemaNode, prop: &str) -> bool {
    node.properties
        .get(prop)
        .is_some_and(|values| !values.is_empty())
}

/// Gets nested nodes from a property.
pub(crate) fn get_nested_nodes<'a>(node: &'a SchemaNode, prop: &str) -> Vec<&'a SchemaNode> {
    node.properties
        .get(prop)
        .map(|values| {
            values
                .iter()
                .filter_map(|v| {
                    if let SchemaValue::Node(n) = v {
                        Some(n.as_ref())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}
