//! Profile evaluation engine -- orchestrates profile checks across graph nodes.

use crate::graph::StructuredDataGraph;
use crate::validation::ValidationDiagnostic;
use crate::vocabulary;

use super::{Eligibility, Profile, ProfileResult, TypeEligibility};

/// Evaluates a graph against a single profile.
///
/// For each node in the graph, checks if its types match the profile's
/// `supported_types()` (including subtypes via inheritance). If so,
/// evaluates the node and collects results.
pub fn evaluate_graph(
    profile: &dyn Profile,
    graph: &StructuredDataGraph,
    vocab_diagnostics: &[ValidationDiagnostic],
) -> ProfileResult {
    let mut type_results = Vec::new();
    let mut all_diagnostics = Vec::new();

    for node in &graph.nodes {
        // Check if this node's types match any supported type (including subtypes)
        let dominated = node.types.iter().any(|t| {
            profile
                .supported_types()
                .iter()
                .any(|supported| t == supported || vocabulary::is_subtype(t, supported))
        });

        if !dominated {
            continue;
        }

        let result = profile.evaluate_node(node, vocab_diagnostics);
        all_diagnostics.extend(result.type_eligibility.field_diagnostics.iter().cloned());
        type_results.push(result.type_eligibility);
    }

    let eligibility = aggregate_eligibility(&type_results, &all_diagnostics);

    ProfileResult {
        eligibility,
        type_results,
        diagnostics: all_diagnostics,
    }
}

/// Aggregates individual type eligibilities into an overall eligibility verdict.
pub(crate) fn aggregate_eligibility(
    type_results: &[TypeEligibility],
    diagnostics: &[ValidationDiagnostic],
) -> Eligibility {
    if type_results.is_empty() {
        return Eligibility::NotEligible;
    }

    // Check if any type result has restricted eligibility (from diagnostics)
    let has_restricted = diagnostics
        .iter()
        .any(|d| d.code == crate::validation::DiagnosticCode::EligibilityRestricted);

    let all_eligible = type_results.iter().all(|r| r.eligible);
    let has_warnings = !diagnostics.is_empty();

    if has_restricted {
        Eligibility::Restricted
    } else if all_eligible && !has_warnings {
        Eligibility::Eligible
    } else if all_eligible {
        Eligibility::WarningsOnly
    } else {
        Eligibility::NotEligible
    }
}
