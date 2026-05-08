//! Google Rich Results profile for `LocalBusiness` (and subtypes).
//!
//! Source: <https://developers.google.com/search/docs/appearance/structured-data/local-business>
//! Verified: 2026-04-01

use crate::types::SchemaNode;
use crate::validation::ValidationDiagnostic as VD;

use super::common::{recommend_property, require_property, validate_nested};
use crate::profiles::{NodeProfileResult, Profile, TypeEligibility};

/// Google Rich Results profile for `LocalBusiness` structured data.
///
/// Applies to: `LocalBusiness` and all subtypes (`Restaurant`, `Store`,
/// `MedicalClinic`, etc.) via inheritance.
pub struct GoogleLocalBusinessProfile;

impl Profile for GoogleLocalBusinessProfile {
    fn name(&self) -> &'static str {
        "google"
    }

    fn version(&self) -> &'static str {
        "2026-04-01"
    }

    fn source_url(&self) -> &'static str {
        "https://developers.google.com/search/docs/appearance/structured-data/local-business"
    }

    fn supported_types(&self) -> &[&str] {
        &["LocalBusiness"]
    }

    fn evaluate_node(&self, node: &SchemaNode, _vocab_diagnostics: &[VD]) -> NodeProfileResult {
        let type_name = node.types.first().map_or("LocalBusiness", |t| t.as_str());
        let path = type_name;
        let mut diagnostics = Vec::new();
        let mut required_missing = Vec::new();
        let mut recommended_missing = Vec::new();

        // Required fields
        if let Some(d) = require_property(node, "name", path) {
            required_missing.push("name".to_string());
            diagnostics.push(d);
        }
        if let Some(d) = require_property(node, "address", path) {
            required_missing.push("address".to_string());
            diagnostics.push(d);
        }

        // Recommended fields
        for prop in &[
            "image",
            "telephone",
            "url",
            "openingHoursSpecification",
            "geo",
            "priceRange",
        ] {
            if let Some(d) = recommend_property(node, prop, path) {
                recommended_missing.push((*prop).to_string());
                diagnostics.push(d);
            }
        }

        // Nested PostalAddress validation
        let address_diags = validate_nested(
            node,
            "address",
            "PostalAddress",
            &[
                "streetAddress",
                "addressLocality",
                "postalCode",
                "addressCountry",
            ],
            &["addressRegion"],
            path,
        );
        diagnostics.extend(address_diags);

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
