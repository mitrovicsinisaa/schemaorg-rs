//! Type-level validation: unknown, deprecated, and pending type detection.

use std::fmt::Write;

use crate::vocabulary;

use super::diagnostics::{DiagnosticCode, Severity, ValidationDiagnostic};
use super::suggest_similar;

/// Checks a single type name against the Schema.org vocabulary.
///
/// Produces diagnostics for:
/// - Unknown types (with "did you mean?" suggestions)
/// - Deprecated/attic types
/// - Pending types
pub(crate) fn check_type(type_name: &str, path: &str, diagnostics: &mut Vec<ValidationDiagnostic>) {
    if let Some(type_def) = vocabulary::lookup_type(type_name) {
        if type_def.is_attic {
            diagnostics.push(ValidationDiagnostic {
                path: path.to_string(),
                severity: Severity::Warning,
                code: DiagnosticCode::DeprecatedType,
                message: format!(
                    "Type '{type_name}' has been retired from Schema.org (moved to attic)"
                ),
                source_location: None,
            });
        } else if type_def.is_pending {
            diagnostics.push(ValidationDiagnostic {
                path: path.to_string(),
                severity: Severity::Info,
                code: DiagnosticCode::PendingType,
                message: format!(
                    "Type '{type_name}' is pending (not yet part of the stable Schema.org vocabulary)"
                ),
                source_location: None,
            });
        }
    } else {
        // Unknown type -- try to suggest a similar one
        let mut message = format!("Unknown type '{type_name}'");
        if let Some(suggestion) = suggest_similar(type_name, vocabulary::all_type_names()) {
            write!(message, ". Did you mean '{suggestion}'?").ok();
        }

        diagnostics.push(ValidationDiagnostic {
            path: path.to_string(),
            severity: Severity::Error,
            code: DiagnosticCode::UnknownType,
            message,
            source_location: None,
        });
    }
}
