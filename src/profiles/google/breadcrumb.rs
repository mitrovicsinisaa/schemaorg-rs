//! Google Rich Results profile for `BreadcrumbList`.
//!
//! Source: <https://developers.google.com/search/docs/appearance/structured-data/breadcrumb>
//! Verified: 2026-04-01

use crate::types::{SchemaNode, SchemaValue};
use crate::validation::diagnostics::{DiagnosticCode, Severity, ValidationDiagnostic};
use crate::validation::ValidationDiagnostic as VD;

use super::common::{get_nested_nodes, has_non_empty_property, require_property};
use crate::profiles::{NodeProfileResult, Profile, TypeEligibility};

/// Google Rich Results profile for `BreadcrumbList` structured data.
pub struct GoogleBreadcrumbProfile;

impl Profile for GoogleBreadcrumbProfile {
    fn name(&self) -> &'static str {
        "google"
    }

    fn version(&self) -> &'static str {
        "2026-04-01"
    }

    fn source_url(&self) -> &'static str {
        "https://developers.google.com/search/docs/appearance/structured-data/breadcrumb"
    }

    fn supported_types(&self) -> &[&str] {
        &["BreadcrumbList"]
    }

    fn evaluate_node(&self, node: &SchemaNode, _vocab_diagnostics: &[VD]) -> NodeProfileResult {
        let path = "BreadcrumbList";
        let mut diagnostics = Vec::new();
        let mut required_missing = Vec::new();

        // Required: itemListElement
        if let Some(d) = require_property(node, "itemListElement", path) {
            required_missing.push("itemListElement".to_string());
            diagnostics.push(d);
        }

        let items = get_nested_nodes(node, "itemListElement");

        if items.is_empty() && has_non_empty_property(node, "itemListElement") {
            // itemListElement exists but has no nested nodes
            diagnostics.push(ValidationDiagnostic {
                path: format!("{path}.itemListElement"),
                severity: Severity::Error,
                code: DiagnosticCode::InvalidFieldValue,
                message: "itemListElement must contain ListItem objects".to_string(),
                source_location: None,
            });
        }

        let total_items = items.len();

        for (i, item) in items.iter().enumerate() {
            let item_path = format!("{path}.itemListElement[{i}]");
            let is_last = i == total_items - 1;

            // ListItem: position required
            if let Some(d) = require_property(item, "position", &item_path) {
                diagnostics.push(d);
            }

            // ListItem: name required
            if let Some(d) = require_property(item, "name", &item_path) {
                diagnostics.push(d);
            }

            // ListItem: item required (except last item, which represents current page)
            if !is_last {
                if let Some(d) = require_property(item, "item", &item_path) {
                    diagnostics.push(d);
                }
            }

            // Validate position is sequential (starting at 1)
            if let Some(values) = item.properties.get("position") {
                #[allow(clippy::cast_precision_loss)] // position index is always tiny
                let expected = (i + 1) as f64;
                let actual = values.first().and_then(|v| match v {
                    SchemaValue::Number(n) => Some(*n),
                    SchemaValue::Text(s) => s.parse::<f64>().ok(),
                    _ => None,
                });

                #[allow(clippy::float_cmp)] // position is always an integer
                if let Some(pos) = actual {
                    if pos != expected {
                        diagnostics.push(ValidationDiagnostic {
                            path: format!("{item_path}.position"),
                            severity: Severity::Warning,
                            code: DiagnosticCode::InvalidFieldValue,
                            message: format!(
                                "Position should be {}, got {}. \
                                 Positions must be sequential starting at 1",
                                i + 1,
                                pos,
                            ),
                            source_location: None,
                        });
                    }
                }
            }
        }

        let eligible = required_missing.is_empty()
            && !diagnostics.iter().any(|d| d.severity == Severity::Error);

        NodeProfileResult {
            type_eligibility: TypeEligibility {
                schema_type: "BreadcrumbList".to_string(),
                eligible,
                required_missing,
                recommended_missing: Vec::new(),
                field_diagnostics: diagnostics,
            },
        }
    }
}
