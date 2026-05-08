//! Diagnostic types for Schema.org validation.
//!
//! [`ValidationDiagnostic`] represents a single validation finding with a
//! JSON-path-like location, severity level, machine-readable code, and
//! human-readable message.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::types::SourceLocation;

/// A single validation diagnostic (error, warning, or informational).
///
/// Each diagnostic describes a specific issue found during validation,
/// with enough context for both human readers and programmatic consumers.
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "validation")]
/// # {
/// use schemaorg_rs::validation::{ValidationDiagnostic, Severity, DiagnosticCode};
///
/// let diag = ValidationDiagnostic {
/// path: "Product.offers[0].price".into(),
/// severity: Severity::Error,
/// code: DiagnosticCode::InvalidValueType,
/// message: "Property 'price' expects Number or Text, got Person".into(),
/// source_location: None,
/// };
///
/// assert!(diag.severity == Severity::Error);
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationDiagnostic {
    /// JSON-path-like location in the structured data graph.
    ///
    /// Examples: `"Product"`, `"Product.offers[0].price"`.
    pub path: String,

    /// Severity level.
    pub severity: Severity,

    /// Machine-readable diagnostic code.
    pub code: DiagnosticCode,

    /// Human-readable message describing the issue.
    pub message: String,

    /// Location in the original HTML, if available.
    pub source_location: Option<SourceLocation>,
}

/// Severity level for validation diagnostics.
///
/// Ordered from most to least severe (`Error > Warning > Info`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Severity {
    /// Invalid per the Schema.org specification.
    Error,
    /// Deprecated, likely unintended, or potentially incorrect.
    Warning,
    /// Informational (e.g. pending types).
    Info,
}

/// Machine-readable diagnostic codes.
///
/// Each code corresponds to a specific class of validation issue.
/// Use these for programmatic filtering and reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DiagnosticCode {
    // Type-level
    /// The `@type` value is not a known Schema.org type.
    UnknownType,
    /// The type has been retired to `attic.schema.org`.
    DeprecatedType,
    /// The type is in `pending.schema.org` (not yet stable).
    PendingType,

    // Property-level
    /// The property name is not a known Schema.org property.
    UnknownProperty,
    /// The property exists but is not valid for the given type.
    PropertyNotForType,
    /// The property has been superseded by another.
    DeprecatedProperty,
    /// The property is in `pending.schema.org`.
    PendingProperty,

    // Value-level
    /// The value type does not match any expected `rangeIncludes` type.
    InvalidValueType,
    /// A URL was expected but plain text was provided.
    ExpectedUrlGotText,
    /// A text value was expected but a nested object was provided.
    ExpectedTextGotNode,
    /// The value should be an enumeration member but is not.
    InvalidEnumValue,
    /// A boolean value was provided as a string (`"true"` / `"false"`).
    InvalidBoolean,
    /// A non-numeric string was provided where a number was expected.
    InvalidNumber,

    // Profile-level
    /// A required field is missing (profile-specific).
    RequiredFieldMissing,
    /// A recommended field is missing (profile-specific).
    RecommendedFieldMissing,
    /// A required field in a nested object is missing.
    NestedRequiredFieldMissing,
    /// A field value does not meet profile-specific constraints.
    InvalidFieldValue,
    /// Eligibility is restricted by external factors (e.g. site authority).
    EligibilityRestricted,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
            Self::Info => write!(f, "info"),
        }
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::UnknownType => "unknown-type",
            Self::DeprecatedType => "deprecated-type",
            Self::PendingType => "pending-type",
            Self::UnknownProperty => "unknown-property",
            Self::PropertyNotForType => "property-not-for-type",
            Self::DeprecatedProperty => "deprecated-property",
            Self::PendingProperty => "pending-property",
            Self::InvalidValueType => "invalid-value-type",
            Self::ExpectedUrlGotText => "expected-url-got-text",
            Self::ExpectedTextGotNode => "expected-text-got-node",
            Self::InvalidEnumValue => "invalid-enum-value",
            Self::InvalidBoolean => "invalid-boolean",
            Self::InvalidNumber => "invalid-number",
            Self::RequiredFieldMissing => "required-field-missing",
            Self::RecommendedFieldMissing => "recommended-field-missing",
            Self::NestedRequiredFieldMissing => "nested-required-field-missing",
            Self::InvalidFieldValue => "invalid-field-value",
            Self::EligibilityRestricted => "eligibility-restricted",
        };
        write!(f, "{s}")
    }
}
