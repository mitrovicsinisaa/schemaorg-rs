//! Google Rich Results profile for Event.
//!
//! Source: <https://developers.google.com/search/docs/appearance/structured-data/event>
//! Verified: 2026-04-01

use crate::types::SchemaNode;
use crate::validation::ValidationDiagnostic as VD;

use super::common::{recommend_property, require_property, validate_nested};
use crate::profiles::{NodeProfileResult, Profile, TypeEligibility};

/// Google Rich Results profile for Event structured data.
pub struct GoogleEventProfile;

impl Profile for GoogleEventProfile {
    fn name(&self) -> &'static str {
        "google"
    }

    fn version(&self) -> &'static str {
        "2026-04-01"
    }

    fn source_url(&self) -> &'static str {
        "https://developers.google.com/search/docs/appearance/structured-data/event"
    }

    fn supported_types(&self) -> &[&str] {
        &["Event"]
    }

    fn evaluate_node(&self, node: &SchemaNode, _vocab_diagnostics: &[VD]) -> NodeProfileResult {
        let type_name = node.types.first().map_or("Event", |t| t.as_str());
        let path = type_name;
        let mut diagnostics = Vec::new();
        let mut required_missing = Vec::new();
        let mut recommended_missing = Vec::new();

        // Required fields
        for prop in &["name", "startDate", "location"] {
            if let Some(d) = require_property(node, prop, path) {
                required_missing.push((*prop).to_string());
                diagnostics.push(d);
            }
        }

        // Recommended fields
        for prop in &[
            "image",
            "description",
            "endDate",
            "offers",
            "performer",
            "organizer",
            "eventStatus",
            "eventAttendanceMode",
        ] {
            if let Some(d) = recommend_property(node, prop, path) {
                recommended_missing.push((*prop).to_string());
                diagnostics.push(d);
            }
        }

        // Nested Place validation
        let place_diags =
            validate_nested(node, "location", "Place", &["name", "address"], &[], path);
        diagnostics.extend(place_diags);

        // Nested VirtualLocation validation
        let virtual_diags =
            validate_nested(node, "location", "VirtualLocation", &["url"], &[], path);
        diagnostics.extend(virtual_diags);

        let eligible = required_missing.is_empty();

        NodeProfileResult {
            type_eligibility: TypeEligibility {
                schema_type: type_name.to_string(),
                eligible,
                required_missing,
                recommended_missing,
                field_diagnostics: diagnostics,
            },
        }
    }
}
