//! Google Rich Results profile for Product.
//!
//! Source: <https://developers.google.com/search/docs/appearance/structured-data/product>
//! Verified: 2026-04-01

use crate::types::SchemaNode;
use crate::validation::diagnostics::{DiagnosticCode, Severity, ValidationDiagnostic};
use crate::validation::ValidationDiagnostic as VD;

use super::common::{
    get_nested_nodes, recommend_property, require_one_of, require_property, validate_nested,
};
use crate::profiles::{NodeProfileResult, Profile, TypeEligibility};

/// Google Rich Results profile for Product structured data.
pub struct GoogleProductProfile;

impl Profile for GoogleProductProfile {
    fn name(&self) -> &'static str {
        "google"
    }

    fn version(&self) -> &'static str {
        "2026-04-01"
    }

    fn source_url(&self) -> &'static str {
        "https://developers.google.com/search/docs/appearance/structured-data/product"
    }

    fn supported_types(&self) -> &[&str] {
        &["Product"]
    }

    fn evaluate_node(&self, node: &SchemaNode, _vocab_diagnostics: &[VD]) -> NodeProfileResult {
        let path = "Product";
        let mut diagnostics: Vec<ValidationDiagnostic> = Vec::new();
        let mut required_missing = Vec::new();
        let mut recommended_missing = Vec::new();

        // Required: name
        if let Some(d) = require_property(node, "name", path) {
            required_missing.push("name".to_string());
            diagnostics.push(d);
        }

        // Recommended fields
        for prop in &["image", "description", "brand", "sku"] {
            if let Some(d) = recommend_property(node, prop, path) {
                recommended_missing.push((*prop).to_string());
                diagnostics.push(d);
            }
        }

        // Recommended: at least one global identifier
        if let Some(d) = require_one_of(
            node,
            &["gtin", "gtin8", "gtin13", "gtin14", "isbn", "mpn"],
            path,
            Severity::Warning,
        ) {
            recommended_missing.push("gtin/isbn/mpn".to_string());
            diagnostics.push(d);
        }

        // Recommended: offers or aggregateOffer
        if let Some(d) =
            require_one_of(node, &["offers", "aggregateOffer"], path, Severity::Warning)
        {
            recommended_missing.push("offers".to_string());
            diagnostics.push(d);
        }

        // Recommended: aggregateRating or review
        if let Some(d) = require_one_of(
            node,
            &["aggregateRating", "review"],
            path,
            Severity::Warning,
        ) {
            recommended_missing.push("aggregateRating/review".to_string());
            diagnostics.push(d);
        }

        // Nested Offer validation
        let offer_diags = validate_nested(
            node,
            "offers",
            "Offer",
            &["price", "priceCurrency", "availability"],
            &["url", "priceValidUntil", "itemCondition"],
            path,
        );
        diagnostics.extend(offer_diags);

        // Nested AggregateOffer validation
        let agg_offer_diags = validate_nested(
            node,
            "aggregateOffer",
            "AggregateOffer",
            &["lowPrice", "priceCurrency"],
            &["highPrice"],
            path,
        );
        diagnostics.extend(agg_offer_diags);

        // Nested AggregateRating validation
        for rating_node in get_nested_nodes(node, "aggregateRating") {
            let rating_path = format!("{path}.aggregateRating");
            if let Some(d) = require_property(rating_node, "ratingValue", &rating_path) {
                diagnostics.push(d);
            }
            if let Some(d) = require_one_of(
                rating_node,
                &["ratingCount", "reviewCount"],
                &rating_path,
                Severity::Error,
            ) {
                diagnostics.push(d);
            }
        }

        // Nested Review validation
        let review_diags = validate_nested(
            node,
            "review",
            "Review",
            &["author"],
            &["reviewRating"],
            path,
        );
        diagnostics.extend(review_diags);

        let eligible = required_missing.is_empty()
            && !diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::NestedRequiredFieldMissing);

        NodeProfileResult {
            type_eligibility: TypeEligibility {
                schema_type: "Product".to_string(),
                eligible,
                required_missing,
                recommended_missing,
                field_diagnostics: diagnostics,
            },
        }
    }
}
