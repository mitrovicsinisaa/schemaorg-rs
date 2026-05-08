//! Google Rich Results profile for Recipe.
//!
//! Source: <https://developers.google.com/search/docs/appearance/structured-data/recipe>
//! Verified: 2026-04-01

use crate::types::SchemaNode;
use crate::validation::ValidationDiagnostic as VD;

use super::common::{recommend_property, require_property, validate_nested};
use crate::profiles::{NodeProfileResult, Profile, TypeEligibility};

/// Google Rich Results profile for Recipe structured data.
pub struct GoogleRecipeProfile;

impl Profile for GoogleRecipeProfile {
    fn name(&self) -> &'static str {
        "google"
    }

    fn version(&self) -> &'static str {
        "2026-04-01"
    }

    fn source_url(&self) -> &'static str {
        "https://developers.google.com/search/docs/appearance/structured-data/recipe"
    }

    fn supported_types(&self) -> &[&str] {
        &["Recipe"]
    }

    fn evaluate_node(&self, node: &SchemaNode, _vocab_diagnostics: &[VD]) -> NodeProfileResult {
        let path = "Recipe";
        let mut diagnostics = Vec::new();
        let mut required_missing = Vec::new();
        let mut recommended_missing = Vec::new();

        // Required fields
        for prop in &["name", "image"] {
            if let Some(d) = require_property(node, prop, path) {
                required_missing.push((*prop).to_string());
                diagnostics.push(d);
            }
        }

        // Recommended fields
        for prop in &[
            "author",
            "datePublished",
            "description",
            "recipeCuisine",
            "prepTime",
            "cookTime",
            "totalTime",
            "recipeYield",
            "recipeCategory",
            "recipeIngredient",
            "recipeInstructions",
            "nutrition",
            "video",
        ] {
            if let Some(d) = recommend_property(node, prop, path) {
                recommended_missing.push((*prop).to_string());
                diagnostics.push(d);
            }
        }

        // Nested HowToStep validation (in recipeInstructions)
        let step_diags = validate_nested(
            node,
            "recipeInstructions",
            "HowToStep",
            &["text"],
            &[],
            path,
        );
        diagnostics.extend(step_diags);

        let eligible = required_missing.is_empty();

        NodeProfileResult {
            type_eligibility: TypeEligibility {
                schema_type: "Recipe".to_string(),
                eligible,
                required_missing,
                recommended_missing,
                field_diagnostics: diagnostics,
            },
        }
    }
}
