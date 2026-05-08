//! Google Rich Results profile for Article (and subtypes).
//!
//! Source: <https://developers.google.com/search/docs/appearance/structured-data/article>
//! Verified: 2026-04-01

use crate::types::SchemaNode;
use crate::validation::ValidationDiagnostic as VD;

use super::common::{recommend_property, require_property, validate_nested};
use crate::profiles::{NodeProfileResult, Profile, TypeEligibility};

/// Google Rich Results profile for Article structured data.
///
/// Applies to: `Article`, `NewsArticle`, `BlogPosting`, `TechArticle`,
/// `ScholarlyArticle` (all subtypes via inheritance).
pub struct GoogleArticleProfile;

impl Profile for GoogleArticleProfile {
    fn name(&self) -> &'static str {
        "google"
    }

    fn version(&self) -> &'static str {
        "2026-04-01"
    }

    fn source_url(&self) -> &'static str {
        "https://developers.google.com/search/docs/appearance/structured-data/article"
    }

    fn supported_types(&self) -> &[&str] {
        &[
            "Article",
            "NewsArticle",
            "BlogPosting",
            "TechArticle",
            "ScholarlyArticle",
        ]
    }

    fn evaluate_node(&self, node: &SchemaNode, _vocab_diagnostics: &[VD]) -> NodeProfileResult {
        let type_name = node.types.first().map_or("Article", |t| t.as_str());
        let path = type_name;
        let mut diagnostics = Vec::new();
        let mut required_missing = Vec::new();
        let mut recommended_missing = Vec::new();

        // Required fields
        for prop in &["headline", "image", "datePublished", "author"] {
            if let Some(d) = require_property(node, prop, path) {
                required_missing.push((*prop).to_string());
                diagnostics.push(d);
            }
        }

        // Recommended fields
        for prop in &["dateModified", "publisher", "mainEntityOfPage"] {
            if let Some(d) = recommend_property(node, prop, path) {
                recommended_missing.push((*prop).to_string());
                diagnostics.push(d);
            }
        }

        // Nested Author validation
        let author_diags = validate_nested(node, "author", "Person", &["name"], &["url"], path);
        diagnostics.extend(author_diags);

        // Nested Publisher validation
        let publisher_diags = validate_nested(
            node,
            "publisher",
            "Organization",
            &["name"],
            &["logo"],
            path,
        );
        diagnostics.extend(publisher_diags);

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
