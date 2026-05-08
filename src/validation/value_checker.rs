//! Value-level validation: type matching, coercion, enum validation.
//!
//! Checks that property values match the expected types defined by
//! `rangeIncludes` in the Schema.org vocabulary.

use crate::types::SchemaValue;
use crate::vocabulary;

use super::diagnostics::{DiagnosticCode, Severity, ValidationDiagnostic};

/// Schema.org `DataType` names that are primitive value type constraints.
const PRIMITIVE_TYPES: &[&str] = &[
    "Text",
    "Number",
    "Boolean",
    "Date",
    "DateTime",
    "Time",
    "URL",
    "Integer",
    "Float",
    "CssSelectorType",
    "PronounceableText",
    "XPathType",
];

/// Checks a single property value against the expected types.
///
/// `prop_name` is used for diagnostic messages.
/// `expected_types` comes from `PropertyDef.expected_types`.
pub(crate) fn check_value(
    value: &SchemaValue,
    prop_name: &str,
    expected_types: &[&str],
    path: &str,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    // If no expected types are defined, skip value checking
    if expected_types.is_empty() {
        return;
    }

    match value {
        SchemaValue::Text(s) => check_text_value(s, prop_name, expected_types, path, diagnostics),
        SchemaValue::Url(_) => check_url_value(expected_types, path, diagnostics),
        SchemaValue::Node(node) => {
            check_node_value(&node.types, prop_name, expected_types, path, diagnostics);
        }
        SchemaValue::Number(_) => check_number_value(expected_types, path, diagnostics),
        SchemaValue::Boolean(_) => check_boolean_value(expected_types, path, diagnostics),
        SchemaValue::DateTime(_) => check_datetime_value(expected_types, path, diagnostics),
    }
}

/// Checks if a text value matches the expected types.
fn check_text_value(
    text: &str,
    prop_name: &str,
    expected: &[&str],
    path: &str,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    // Text is compatible with: Text, URL, Date, DateTime, Time, PronounceableText,
    // CssSelectorType, XPathType
    if has_any(
        expected,
        &["Text", "PronounceableText", "CssSelectorType", "XPathType"],
    ) {
        return;
    }

    // Text can also be a date string
    if has_any(expected, &["Date", "DateTime", "Time"]) {
        return;
    }

    // Text might be a number in string form
    if has_any(expected, &["Number", "Integer", "Float"]) {
        if text.parse::<f64>().is_err() {
            diagnostics.push(ValidationDiagnostic {
                path: path.to_string(),
                severity: Severity::Warning,
                code: DiagnosticCode::InvalidNumber,
                message: format!("Property '{prop_name}' expects a number, got text: \"{text}\""),
                source_location: None,
            });
        }
        return;
    }

    // Text might be "true"/"false" for boolean
    if has_any(expected, &["Boolean"]) && (text == "true" || text == "false") {
        diagnostics.push(ValidationDiagnostic {
            path: path.to_string(),
            severity: Severity::Warning,
            code: DiagnosticCode::InvalidBoolean,
            message: format!(
                "Property '{prop_name}' expects a boolean, got string \"{text}\". Use a boolean value instead"
            ),
            source_location: None,
        });
        return;
    }

    // Text might be a URL that should have been typed as URL
    if has_any(expected, &["URL"]) {
        if !text.starts_with("http://")
            && !text.starts_with("https://")
            && !text.starts_with("mailto:")
        {
            diagnostics.push(ValidationDiagnostic {
                path: path.to_string(),
                severity: Severity::Warning,
                code: DiagnosticCode::ExpectedUrlGotText,
                message: format!("Property '{prop_name}' expects a URL"),
                source_location: None,
            });
        }
        return;
    }

    // Text might be an enum value
    let enum_types: Vec<&&str> = expected
        .iter()
        .filter(|t| !PRIMITIVE_TYPES.contains(t))
        .collect();

    if !enum_types.is_empty() {
        // Check if the text is a known enum member
        if vocabulary::lookup_enum_member(text).is_some() {
            return; // Valid enum member
        }
        // Check if it could be a valid type used as a value (e.g. "InStock" for ItemAvailability)
        if vocabulary::lookup_type(text).is_some() {
            return; // Type name used as value -- valid for enum-like properties
        }

        // Unknown enum value
        let expected_list = enum_types
            .iter()
            .map(|t| format!("'{t}'"))
            .collect::<Vec<_>>()
            .join(", ");
        diagnostics.push(ValidationDiagnostic {
            path: path.to_string(),
            severity: Severity::Error,
            code: DiagnosticCode::InvalidEnumValue,
            message: format!(
                "'{text}' is not a valid value for property '{prop_name}' (expected {expected_list} member)"
            ),
            source_location: None,
        });
        return;
    }

    // Text provided but only non-text types expected
    let expected_list = expected.join(", ");
    diagnostics.push(ValidationDiagnostic {
        path: path.to_string(),
        severity: Severity::Error,
        code: DiagnosticCode::InvalidValueType,
        message: format!("Property '{prop_name}' expects {expected_list}, got Text"),
        source_location: None,
    });
}

/// Checks if a URL value matches the expected types.
fn check_url_value(_expected: &[&str], _path: &str, _diagnostics: &mut Vec<ValidationDiagnostic>) {
    // URL is compatible with URL and Text (URLs are always valid as text).
    // URLs are also accepted for most other types in practice -- Schema.org
    // allows URLs as identifiers. We don't emit an error here because it's
    // extremely common (e.g., `"availability": "https://schema.org/InStock"`).
}

/// Checks if a nested node matches the expected types.
fn check_node_value(
    node_types: &[String],
    prop_name: &str,
    expected: &[&str],
    path: &str,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    // If expected types are only primitive types, a node is wrong
    let all_primitive = expected.iter().all(|t| PRIMITIVE_TYPES.contains(t));
    if all_primitive {
        let expected_list = expected.join(", ");
        diagnostics.push(ValidationDiagnostic {
            path: path.to_string(),
            severity: Severity::Error,
            code: DiagnosticCode::ExpectedTextGotNode,
            message: format!("Property '{prop_name}' expects {expected_list}, not a nested object"),
            source_location: None,
        });
        return;
    }

    // If the node has no types, we can't validate further
    if node_types.is_empty() {
        return;
    }

    // Check if any of the node's types match expected types (including inheritance)
    let matches = node_types.iter().any(|node_type| {
        expected.iter().any(|expected_type| {
            if PRIMITIVE_TYPES.contains(expected_type) {
                return false;
            }
            // Direct match
            if node_type == expected_type {
                return true;
            }
            // Inheritance: check if node_type is a subtype of expected_type
            is_subtype(node_type, expected_type)
        })
    });

    if !matches {
        let node_type_list = node_types.join(", ");
        let expected_list = expected
            .iter()
            .filter(|t| !PRIMITIVE_TYPES.contains(t))
            .map(|t| format!("'{t}'"))
            .collect::<Vec<_>>()
            .join(", ");
        diagnostics.push(ValidationDiagnostic {
            path: path.to_string(),
            severity: Severity::Error,
            code: DiagnosticCode::InvalidValueType,
            message: format!(
                "Property '{prop_name}' expects {expected_list}, got '{node_type_list}'"
            ),
            source_location: None,
        });
    }
}

/// Checks if a number value matches the expected types.
fn check_number_value(
    _expected: &[&str],
    _path: &str,
    _diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    // Number is compatible with Number, Integer, Float, and Text.
    // In practice, numbers are accepted broadly -- the main value of
    // number checking is for "expected Number got Object" which is
    // handled by the node checker.
}

/// Checks if a boolean value matches the expected types.
fn check_boolean_value(
    _expected: &[&str],
    _path: &str,
    _diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    // Boolean is compatible with Boolean and Text.
    // Similar to numbers, booleans are broadly accepted.
}

/// Checks if a datetime value matches the expected types.
fn check_datetime_value(
    _expected: &[&str],
    _path: &str,
    _diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    // DateTime is compatible with Date, DateTime, Time, and Text.
    // DateTime values are broadly accepted.
}

// Helpers
/// Returns `true` if `expected` contains any of the given `candidates`.
fn has_any(expected: &[&str], candidates: &[&str]) -> bool {
    candidates.iter().any(|c| expected.contains(c))
}

/// Checks if `child_type` is a subtype of `ancestor_type` by walking
/// all parents via BFS in the vocabulary.
fn is_subtype(child_type: &str, ancestor_type: &str) -> bool {
    vocabulary::is_subtype(child_type, ancestor_type)
}
