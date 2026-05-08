//! Google Rich Results profile for `FAQPage`.
//!
//! Source: <https://developers.google.com/search/docs/appearance/structured-data/faqpage>
//! Verified: 2026-04-01
//!
//! **Note:** Since 2024, `FAQPage` rich results are restricted to authoritative
//! government and health-focused websites. This profile validates structure
//! but emits an `EligibilityRestricted` info diagnostic.

use crate::types::{SchemaNode, SchemaValue};
use crate::validation::diagnostics::{DiagnosticCode, Severity, ValidationDiagnostic};
use crate::validation::ValidationDiagnostic as VD;

use super::common::{get_nested_nodes, has_non_empty_property, require_property};
use crate::profiles::{NodeProfileResult, Profile, TypeEligibility};

/// Google Rich Results profile for `FAQPage` structured data.
pub struct GoogleFaqPageProfile;

impl Profile for GoogleFaqPageProfile {
    fn name(&self) -> &'static str {
        "google"
    }

    fn version(&self) -> &'static str {
        "2026-04-01"
    }

    fn source_url(&self) -> &'static str {
        "https://developers.google.com/search/docs/appearance/structured-data/faqpage"
    }

    fn supported_types(&self) -> &[&str] {
        &["FAQPage"]
    }

    fn evaluate_node(&self, node: &SchemaNode, _vocab_diagnostics: &[VD]) -> NodeProfileResult {
        let path = "FAQPage";
        let mut diagnostics = Vec::new();
        let mut required_missing = Vec::new();

        // Eligibility restriction notice
        diagnostics.push(ValidationDiagnostic {
            path: path.to_string(),
            severity: Severity::Info,
            code: DiagnosticCode::EligibilityRestricted,
            message: "FAQPage rich results eligibility is restricted to authoritative sites \
                      since 2024. Structural validation passed, but display depends on \
                      Google's site authority assessment."
                .to_string(),
            source_location: None,
        });

        // Required: mainEntity (array of Questions)
        if let Some(d) = require_property(node, "mainEntity", path) {
            required_missing.push("mainEntity".to_string());
            diagnostics.push(d);
        }

        // Validate each Question in mainEntity
        let questions = get_nested_nodes(node, "mainEntity");
        if questions.is_empty() && has_non_empty_property(node, "mainEntity") {
            // mainEntity exists but contains no nested nodes (might be text/URL)
            // Check if it's an array of SchemaValues that are nodes
            if let Some(values) = node.properties.get("mainEntity") {
                let has_any_node = values.iter().any(|v| matches!(v, SchemaValue::Node(_)));
                if !has_any_node {
                    diagnostics.push(ValidationDiagnostic {
                        path: format!("{path}.mainEntity"),
                        severity: Severity::Error,
                        code: DiagnosticCode::InvalidFieldValue,
                        message: "mainEntity must contain Question objects".to_string(),
                        source_location: None,
                    });
                }
            }
        }

        for (i, question) in questions.iter().enumerate() {
            let q_path = if questions.len() > 1 {
                format!("{path}.mainEntity[{i}]")
            } else {
                format!("{path}.mainEntity")
            };

            // Question: name required
            if let Some(d) = require_property(question, "name", &q_path) {
                diagnostics.push(d);
            }

            // Question: acceptedAnswer required
            if let Some(d) = require_property(question, "acceptedAnswer", &q_path) {
                diagnostics.push(d);
            }

            // Answer: text required
            for answer in get_nested_nodes(question, "acceptedAnswer") {
                let a_path = format!("{q_path}.acceptedAnswer");
                if let Some(d) = require_property(answer, "text", &a_path) {
                    diagnostics.push(d);
                }
            }
        }

        let eligible = required_missing.is_empty()
            && !diagnostics.iter().any(|d| {
                d.severity == Severity::Error && d.code != DiagnosticCode::EligibilityRestricted
            });

        NodeProfileResult {
            type_eligibility: TypeEligibility {
                schema_type: "FAQPage".to_string(),
                eligible,
                required_missing,
                recommended_missing: Vec::new(),
                field_diagnostics: diagnostics,
            },
        }
    }
}
