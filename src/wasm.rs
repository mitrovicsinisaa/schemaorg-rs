//! WASM bindings for `schemaorg-rs`.
//!
//! Exposes three functions to JavaScript:
//!
//! - [`extract`] -- parse HTML and return structured data as JSON
//! - [`validate_html`] -- full pipeline: extract -> validate -> profile evaluate
//! - [`schema_version`] -- returns the Schema.org vocabulary version
//!
//! All functions return JSON strings. The JS wrapper in `wasm/index.js`
//! calls `JSON.parse()` on the results.

use wasm_bindgen::prelude::*;

use crate::graph;
use crate::profiles::ProfileRegistry;
use crate::validation;

/// Initializes the WASM module. Call once before other functions.
/// Sets up a panic hook that logs to `console.error`.
#[wasm_bindgen(start)]
pub fn wasm_start() {
    std::panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        log_error(&msg);
    }));
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn log_error(s: &str);
}

/// Extracts structured data from HTML and returns it as a JSON string.
///
/// The returned JSON contains an array of `SchemaNode` objects with their
/// types, properties, source format, and source location.
///
/// # Returns
///
/// JSON string: `{ "nodes": [...], "warnings": [...] }` on success,
/// or `{ "error": "..." }` on failure.
#[wasm_bindgen]
pub fn extract(html: &str) -> String {
    match graph::extract_all(html) {
        Ok(graph) => {
            let result = serde_json::json!({
                "nodes": graph.nodes,
                "warnings": graph.warnings,
            });
            serde_json::to_string(&result)
                .unwrap_or_else(|e| serde_json::json!({ "error": e.to_string() }).to_string())
        }
        Err(e) => serde_json::json!({ "error": e.to_string() }).to_string(),
    }
}

/// Full validation pipeline: extract -> vocab validate -> profile evaluate.
///
/// Runs extraction, Schema.org vocabulary validation, and profile evaluation
/// in a single call. Returns a combined JSON result.
///
/// # Arguments
///
/// - `html` -- the HTML document to analyze
/// - `profile` -- the profile name to evaluate against (`"google"` or `"baseline"`)
///
/// # Returns
///
/// JSON string containing:
/// ```json
/// {
/// "extraction": { "nodes": [...], "warnings": [...] },
/// "validation": { "diagnostics": [...], "has_errors": bool },
/// "profile": { "eligibility": "...", "type_results": [...], "diagnostics": [...] }
/// }
/// ```
///
/// Or `{ "error": "..." }` on extraction failure.
#[wasm_bindgen]
pub fn validate_html(html: &str, profile: &str) -> String {
    // Step 1: Extract
    let graph = match graph::extract_all(html) {
        Ok(g) => g,
        Err(e) => return serde_json::json!({ "error": e.to_string() }).to_string(),
    };

    // Step 2: Vocabulary validation
    let vocab_result = validation::validate(&graph);

    // Step 3: Profile evaluation
    let registry = match profile {
        "baseline" => ProfileRegistry::with_baseline(),
        _ => ProfileRegistry::with_google(),
    };

    let profile_result = registry.evaluate(profile, &graph, &vocab_result.diagnostics);

    let profile_json = match profile_result {
        Ok(r) => serde_json::json!({
            "eligibility": r.eligibility.to_string(),
            "type_results": r.type_results,
            "diagnostics": r.diagnostics,
        }),
        Err(e) => serde_json::json!({
            "eligibility": "NotEligible",
            "type_results": [],
            "diagnostics": [],
            "note": e.to_string(),
        }),
    };

    let result = serde_json::json!({
        "extraction": {
            "nodes": graph.nodes,
            "warnings": graph.warnings,
        },
        "validation": {
            "diagnostics": vocab_result.diagnostics,
            "has_errors": vocab_result.has_errors(),
        },
        "profile": profile_json,
    });

    serde_json::to_string(&result)
        .unwrap_or_else(|e| serde_json::json!({ "error": e.to_string() }).to_string())
}

/// Returns the Schema.org vocabulary version used by this build.
#[wasm_bindgen]
pub fn schema_version() -> String {
    crate::vocabulary::schema_version().to_string()
}
