//! Property-level validation: unknown, wrong domain, deprecated, and pending properties.

use std::fmt::Write;

use crate::vocabulary;

use super::diagnostics::{DiagnosticCode, Severity, ValidationDiagnostic};
use super::suggest_similar;

/// Checks a property name against the Schema.org vocabulary in the context
/// of a specific type.
///
/// Produces diagnostics for:
/// - Unknown properties (with "did you mean?" suggestions)
/// - Properties valid in Schema.org but not for this type
/// - Superseded/deprecated properties
/// - Pending properties
pub(crate) fn check_property(
    prop_name: &str,
    parent_types: &[String],
    path: &str,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    // Skip @-prefixed meta-properties (@id, @type, @context, etc.)
    if prop_name.starts_with('@') {
        return;
    }

    if let Some(prop_def) = vocabulary::lookup_property(prop_name) {
        // Check if the property is superseded
        if prop_def.is_superseded {
            let mut message = format!("Property '{prop_name}' is superseded");
            if let Some(by) = prop_def.superseded_by {
                write!(message, ". Use '{by}' instead").ok();
            }
            diagnostics.push(ValidationDiagnostic {
                path: path.to_string(),
                severity: Severity::Warning,
                code: DiagnosticCode::DeprecatedProperty,
                message,
                source_location: None,
            });
        }

        // Check if the property is pending
        if prop_def.is_pending {
            diagnostics.push(ValidationDiagnostic {
                path: path.to_string(),
                severity: Severity::Info,
                code: DiagnosticCode::PendingProperty,
                message: format!(
                    "Property '{prop_name}' is pending (not yet part of the stable Schema.org vocabulary)"
                ),
                source_location: None,
            });
        }

        // Check if the property is valid for any of the parent types.
        // If no parent types are known (empty types list), skip domain check.
        if !parent_types.is_empty() {
            let valid_for_any_type = parent_types.iter().any(|type_name| {
                vocabulary::lookup_type(type_name).is_some_and(|td| td.has_property(prop_name))
            });

            if !valid_for_any_type {
                let type_list = parent_types.join(", ");
                diagnostics.push(ValidationDiagnostic {
                    path: path.to_string(),
                    severity: Severity::Warning,
                    code: DiagnosticCode::PropertyNotForType,
                    message: format!("Property '{prop_name}' is not valid for type '{type_list}'"),
                    source_location: None,
                });
            }
        }
    } else {
        // Unknown property -- try to suggest a similar one
        let mut message = format!("Unknown property '{prop_name}'");
        if let Some(suggestion) = suggest_similar(prop_name, vocabulary::all_property_names()) {
            write!(message, ". Did you mean '{suggestion}'?").ok();
        }

        diagnostics.push(ValidationDiagnostic {
            path: path.to_string(),
            severity: Severity::Error,
            code: DiagnosticCode::UnknownProperty,
            message,
            source_location: None,
        });
    }
}
