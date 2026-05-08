//! Rich Results profile system -- platform-specific validation beyond Schema.org.
//!
//! After M2's vocabulary validation answers *"is this valid Schema.org?"*, the
//! profile system answers *"will Google actually show a rich result for this?"*.
//!
//! # Architecture
//!
//! ```text
//! StructuredDataGraph
//!     +---- ProfileRegistry
//!         +---- Google Product profile
//!         +---- Google Article profile
//!         +---- Google `FAQPage` profile
//!         +---- Google `BreadcrumbList` profile
//!         +---- Google `LocalBusiness` profile
//!         +---- Google Event profile
//!         +---- Google Recipe profile
//!         +---- Baseline profile
//! ```
//!
//! # Examples
//!
//! ```no_run
//! # #[cfg(feature = "profiles")]
//! # {
//! use schemaorg_rs::{extract_all, validation};
//! use schemaorg_rs::profiles::{ProfileRegistry, Eligibility};
//!
//! let html = r#"<script type="application/ld+json">{
//!   "@context": "https://schema.org",
//!   "@type": "Product",
//!   "name": "Widget"
//! }</script>"#;
//!
//! let graph = extract_all(html).unwrap();
//! let vocab_result = validation::validate(&graph);
//! let registry = ProfileRegistry::with_google();
//! let result = registry.evaluate("google", &graph, &vocab_result.diagnostics).unwrap();
//!
//! match result.eligibility {
//!     Eligibility::Eligible => println!("Rich result eligible!"),
//!     Eligibility::WarningsOnly => println!("Eligible with warnings"),
//!     Eligibility::NotEligible => println!("Not eligible"),
//!     Eligibility::Restricted => println!("Restricted eligibility"),
//! }
//! # }
//! ```

pub mod baseline;
pub mod engine;
pub mod google;

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::graph::StructuredDataGraph;
use crate::types::SchemaNode;
use crate::validation::ValidationDiagnostic;

/// A deployment profile adds platform-specific rules beyond Schema.org
/// vocabulary validation.
///
/// Each profile defines which types it covers and how to evaluate nodes
/// against its requirements (required fields, recommended fields, nested
/// sub-requirements, etc.).
pub trait Profile: Send + Sync {
    /// Profile identifier (e.g., `"google"`).
    fn name(&self) -> &'static str;

    /// Profile version -- date-based to track when rules were last verified
    /// against the platform's documentation.
    fn version(&self) -> &'static str;

    /// Source documentation URL.
    fn source_url(&self) -> &'static str;

    /// Which Schema.org types does this profile cover?
    ///
    /// The engine checks each node's types against this list to decide
    /// whether to evaluate it.
    fn supported_types(&self) -> &[&str];

    /// Evaluate a single node against this profile's rules.
    ///
    /// Called by the engine for each node whose type matches
    /// [`supported_types()`](Self::supported_types).
    fn evaluate_node(
        &self,
        node: &SchemaNode,
        vocab_diagnostics: &[ValidationDiagnostic],
    ) -> NodeProfileResult;
}

/// Overall result of evaluating a graph against a profile.
///
/// Contains the aggregate eligibility, per-type breakdowns, and any
/// profile-specific diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[must_use]
pub struct ProfileResult {
    /// Overall eligibility across all evaluated nodes.
    pub eligibility: Eligibility,
    /// Per-type eligibility breakdown.
    pub type_results: Vec<TypeEligibility>,
    /// Profile-specific diagnostics (on top of vocabulary diagnostics).
    pub diagnostics: Vec<ValidationDiagnostic>,
}

/// Eligibility verdict for rich result display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Eligibility {
    /// All requirements met -- rich result should display.
    Eligible,
    /// Requirements met but warnings present.
    WarningsOnly,
    /// Missing required fields or structural issues -- no rich result.
    NotEligible,
    /// Structurally valid but eligibility depends on external factors
    /// (e.g., `FAQPage` requires site authority since 2024).
    Restricted,
}

/// Per-type eligibility breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeEligibility {
    /// The Schema.org type evaluated (e.g., `"Product"`).
    pub schema_type: String,
    /// Whether this type passes all required checks.
    pub eligible: bool,
    /// Required fields that are missing.
    pub required_missing: Vec<String>,
    /// Recommended fields that are missing.
    pub recommended_missing: Vec<String>,
    /// Per-field diagnostics specific to this type evaluation.
    pub field_diagnostics: Vec<ValidationDiagnostic>,
}

/// Result of evaluating a single node against a profile.
pub struct NodeProfileResult {
    /// Type-level eligibility for this node.
    pub type_eligibility: TypeEligibility,
}

/// Errors that can occur during profile evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProfileError {
    /// The requested profile name was not found in the registry.
    #[error("unknown profile: '{0}'")]
    UnknownProfile(String),
    /// No nodes in the graph matched any of the profile's supported types.
    #[error("no nodes matched the profile's supported types")]
    NoMatchingTypes,
}

impl fmt::Display for Eligibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eligible => write!(f, "Eligible"),
            Self::WarningsOnly => write!(f, "WarningsOnly"),
            Self::NotEligible => write!(f, "NotEligible"),
            Self::Restricted => write!(f, "Restricted"),
        }
    }
}

/// A registry of available profiles for evaluation.
///
/// Create one with [`with_google()`](Self::with_google) for Google Rich Results
/// profiles, or build a custom registry with [`new()`](Self::new) and
/// [`register()`](Self::register).
pub struct ProfileRegistry {
    profiles: Vec<Box<dyn Profile>>,
}

impl ProfileRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            profiles: Vec::new(),
        }
    }

    /// Creates a registry with all built-in Google Rich Results profiles.
    #[must_use]
    pub fn with_google() -> Self {
        let mut registry = Self::new();
        google::register_all(&mut registry);
        registry
    }

    /// Creates a registry with the baseline Schema.org profile.
    #[must_use]
    pub fn with_baseline() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(baseline::BaselineProfile));
        registry
    }

    /// Registers a profile in the registry.
    pub fn register(&mut self, profile: Box<dyn Profile>) {
        self.profiles.push(profile);
    }

    /// Evaluates a graph against all profiles with the given name.
    ///
    /// Multiple profiles may share the same name (e.g., all Google profiles
    /// are named `"google"`). This method runs all of them and merges results.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::UnknownProfile`] if no profile with the given
    /// name is registered.
    pub fn evaluate(
        &self,
        profile_name: &str,
        graph: &StructuredDataGraph,
        vocab_diagnostics: &[ValidationDiagnostic],
    ) -> Result<ProfileResult, ProfileError> {
        let matching: Vec<_> = self
            .profiles
            .iter()
            .filter(|p| p.name() == profile_name)
            .collect();

        if matching.is_empty() {
            return Err(ProfileError::UnknownProfile(profile_name.to_string()));
        }

        let mut all_type_results = Vec::new();
        let mut all_diagnostics = Vec::new();

        for profile in &matching {
            let result = engine::evaluate_graph(profile.as_ref(), graph, vocab_diagnostics);
            all_type_results.extend(result.type_results);
            all_diagnostics.extend(result.diagnostics);
        }

        let eligibility = engine::aggregate_eligibility(&all_type_results, &all_diagnostics);

        Ok(ProfileResult {
            eligibility,
            type_results: all_type_results,
            diagnostics: all_diagnostics,
        })
    }

    /// Returns the names of all registered profiles.
    #[must_use]
    pub fn profile_names(&self) -> Vec<&str> {
        self.profiles.iter().map(|p| p.name()).collect()
    }
}

impl Default for ProfileRegistry {
    fn default() -> Self {
        Self::new()
    }
}
