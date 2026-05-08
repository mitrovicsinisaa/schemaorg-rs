//! Baseline Schema.org profile -- generic best-practices validation.
//!
//! Not platform-specific. Checks common quality signals that every
//! structured data implementation should satisfy.

use crate::types::{SchemaNode, SchemaValue};
use crate::validation::diagnostics::{DiagnosticCode, Severity, ValidationDiagnostic};

use super::google::common::{has_non_empty_property, recommend_property, require_one_of};
use super::{NodeProfileResult, Profile, TypeEligibility};

/// Baseline profile checking generic Schema.org quality signals.
pub struct BaselineProfile;

impl Profile for BaselineProfile {
    fn name(&self) -> &'static str {
        "baseline"
    }

    fn version(&self) -> &'static str {
        "2026-04-01"
    }

    fn source_url(&self) -> &'static str {
        "https://schema.org/docs/gs.html"
    }

    fn supported_types(&self) -> &[&str] {
        // Matches all types -- use a wildcard-like approach
        &["Thing"]
    }

    fn evaluate_node(
        &self,
        node: &SchemaNode,
        _vocab_diagnostics: &[ValidationDiagnostic],
    ) -> NodeProfileResult {
        let type_name = node.types.first().map_or("Thing", |t| t.as_str());
        let path = type_name;
        let mut diagnostics = Vec::new();
        let mut recommended_missing = Vec::new();

        // Every node should have name or headline
        if let Some(d) = require_one_of(node, &["name", "headline"], path, Severity::Warning) {
            recommended_missing.push("name/headline".to_string());
            diagnostics.push(d);
        }

        // image is recommended on all top-level types
        if let Some(d) = recommend_property(node, "image", path) {
            recommended_missing.push("image".to_string());
            diagnostics.push(d);
        }

        // description is recommended on all top-level types
        if let Some(d) = recommend_property(node, "description", path) {
            recommended_missing.push("description".to_string());
            diagnostics.push(d);
        }

        // Check for HTTPS in URL properties
        check_url_https(node, "url", path, &mut diagnostics);
        check_url_https(node, "image", path, &mut diagnostics);

        let eligible = !diagnostics.iter().any(|d| d.severity == Severity::Error);

        NodeProfileResult {
            type_eligibility: TypeEligibility {
                schema_type: type_name.to_string(),
                eligible,
                required_missing: Vec::new(),
                recommended_missing,
                field_diagnostics: diagnostics,
            },
        }
    }
}

/// Checks that URL-typed properties use HTTPS when present.
fn check_url_https(
    node: &SchemaNode,
    prop: &str,
    path: &str,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    if !has_non_empty_property(node, prop) {
        return;
    }

    if let Some(values) = node.properties.get(prop) {
        for value in values {
            if let SchemaValue::Url(url) = value {
                if url.starts_with("http://") {
                    diagnostics.push(ValidationDiagnostic {
                        path: format!("{path}.{prop}"),
                        severity: Severity::Warning,
                        code: DiagnosticCode::InvalidFieldValue,
                        message: format!("URL should use HTTPS instead of HTTP: {url}"),
                        source_location: None,
                    });
                }
            }
        }
    }
}
