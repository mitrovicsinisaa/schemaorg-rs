//! JSON-LD extractor: parses `<script type="application/ld+json">` tags.
//!
//! Implements a purpose-built Schema.org JSON-LD parser using `serde_json`
//! instead of the full `json-ld` crate. This avoids 300+ transitive dependencies
//! and async requirements while covering >99% of real-world Schema.org usage.
//!
//! ## Supported features
//!
//! - `@context: "https://schema.org"` (string or array)
//! - `@type` as string or array
//! - `@graph` arrays
//! - `@id` cross-reference resolution (within-document)
//! - Nested objects
//!
//! ## Not supported
//!
//! - Remote `@context` fetching
//! - `@context` term definitions (e.g. `{"cat": "schema:category"}`)
//! - JSON-LD framing, `@reverse`

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use indexmap::IndexMap;
use scraper::{Html, Selector};
use serde_json::Value;

use crate::error::{ExtractionError, ExtractionWarning, WarningCode};
use crate::types::{SchemaNode, SchemaValue, SourceFormat, SourceLocation};

use super::{classify_text_value, strip_schema_prefix, ExtractionOutput, Extractor};

/// Maximum nesting depth for JSON-LD objects.
const MAX_DEPTH: usize = 20;

/// Maximum depth for `@id` cross-reference resolution.
///
/// Kept lower than `MAX_DEPTH` to bound amplification when a single
/// `@id` is referenced from multiple locations in the tree.
const MAX_REF_DEPTH: usize = 10;

/// Maximum number of `@id` reference resolutions per document.
///
/// Bounds total memory amplification when many references point
/// to the same large node. Each resolution clones the target node.
const MAX_REF_RESOLUTIONS: usize = 50;

/// Extracts Schema.org structured data from JSON-LD `<script>` tags.
///
/// # Examples
///
/// ```
/// use schemaorg_rs::extraction::{Extractor, JsonLdExtractor};
///
/// let html = r#"<html><head>
/// <script type="application/ld+json">{
/// "@context": "https://schema.org",
/// "@type": "Product",
/// "name": "Widget"
/// }</script>
/// </head></html>"#;
///
/// let output = JsonLdExtractor.extract(html).unwrap();
/// assert_eq!(output.nodes[0].types, vec!["Product"]);
/// ```
pub struct JsonLdExtractor;

impl Extractor for JsonLdExtractor {
    fn extract(&self, html: &str) -> Result<ExtractionOutput, ExtractionError> {
        let document = Html::parse_document(html);
        self.extract_from_document(&document, html)
    }
}

impl JsonLdExtractor {
    /// Extracts from an already-parsed document.
    ///
    /// The raw `html` string is needed for source-location computation
    /// (finding byte offsets of `<script>` tags).
    ///
    /// # Errors
    ///
    /// Returns [`ExtractionError`] if a fatal error prevents extraction.
    /// JSON parse failures are captured as warnings, not errors.
    ///
    /// # Panics
    ///
    /// Panics if the internal CSS selector constant fails to parse.
    /// This is a compile-time-verified string and will never fail.
    pub fn extract_from_document(
        &self,
        document: &Html,
        html: &str,
    ) -> Result<ExtractionOutput, ExtractionError> {
        static SELECTOR: OnceLock<Selector> = OnceLock::new();
        let selector = SELECTOR.get_or_init(|| {
            Selector::parse("script[type=\"application/ld+json\"]")
                .expect("static JSON-LD selector must parse")
        });

        let line_index = LineIndex::new(html);
        let script_offsets = find_script_byte_offsets(html);

        let mut all_nodes = Vec::new();
        let mut warnings = Vec::new();

        for (idx, element) in document.select(selector).enumerate() {
            let json_text = element.inner_html();
            let trimmed = json_text.trim();
            let source_location = script_offsets
                .get(idx)
                .map(|&offset| line_index.location(offset));

            if trimmed.is_empty() {
                warnings.push(ExtractionWarning {
                    message: "empty JSON-LD script tag".into(),
                    source_location,
                    code: WarningCode::MalformedJsonLd,
                });
                continue;
            }

            let value: Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(e) => {
                    warnings.push(ExtractionWarning {
                        message: format!("failed to parse JSON-LD: {e}"),
                        source_location,
                        code: WarningCode::MalformedJsonLd,
                    });
                    continue;
                }
            };

            let items = extract_json_items(&value, source_location.as_ref(), &mut warnings);
            all_nodes.extend(items);
        }

        // Build @id -> index map (lightweight, no node cloning).
        // First definition wins: later duplicates emit a warning but
        // do not overwrite the original entry.
        let mut id_to_index: HashMap<String, usize> = HashMap::new();
        for (i, node) in all_nodes.iter().enumerate() {
            if let Some(id) = node.id() {
                match id_to_index.entry(id.to_owned()) {
                    std::collections::hash_map::Entry::Occupied(_) => {
                        warnings.push(ExtractionWarning {
                            message: format!("duplicate @id: {id}"),
                            source_location: node.source_location.clone(),
                            code: WarningCode::DuplicateId,
                        });
                    }
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        entry.insert(i);
                    }
                }
            }
        }

        // Clone only nodes that are actually referenced (lazy)
        let referenced = collect_referenced_ids(&all_nodes);
        let id_map: HashMap<String, SchemaNode> = referenced
            .iter()
            .filter_map(|id| {
                let &idx = id_to_index.get(id.as_str())?;
                Some((id.clone(), all_nodes[idx].clone()))
            })
            .collect();

        // Resolve @id cross-references
        resolve_references(&mut all_nodes, &id_map, &mut warnings);

        Ok(ExtractionOutput {
            nodes: all_nodes,
            warnings,
        })
    }
}

// JSON -> SchemaNode conversion
/// Extracts top-level Schema.org items from a parsed JSON value.
fn extract_json_items(
    value: &Value,
    source_location: Option<&SourceLocation>,
    warnings: &mut Vec<ExtractionWarning>,
) -> Vec<SchemaNode> {
    match value {
        Value::Array(items) => items
            .iter()
            .filter_map(|item| json_to_node(item, None, source_location, warnings, 0))
            .collect(),

        Value::Object(map) => {
            if let Some(Value::Array(graph_items)) = map.get("@graph") {
                let context = map.get("@context");
                graph_items
                    .iter()
                    .filter_map(|item| json_to_node(item, context, source_location, warnings, 0))
                    .collect()
            } else {
                json_to_node(value, None, source_location, warnings, 0)
                    .into_iter()
                    .collect()
            }
        }

        _ => {
            warnings.push(ExtractionWarning {
                message: "JSON-LD root must be an object or array".into(),
                source_location: source_location.cloned(),
                code: WarningCode::MalformedJsonLd,
            });
            Vec::new()
        }
    }
}

/// Converts a JSON object to a [`SchemaNode`].
///
/// `parent_context` is the `@context` inherited from a `@graph` wrapper.
fn json_to_node(
    value: &Value,
    parent_context: Option<&Value>,
    source_location: Option<&SourceLocation>,
    warnings: &mut Vec<ExtractionWarning>,
    depth: usize,
) -> Option<SchemaNode> {
    if depth > MAX_DEPTH {
        warnings.push(ExtractionWarning {
            message: format!("JSON-LD nesting depth exceeds {MAX_DEPTH}, skipping"),
            source_location: source_location.cloned(),
            code: WarningCode::MalformedJsonLd,
        });
        return None;
    }
    let obj = value.as_object()?;

    // Resolve @context: local overrides parent
    let context = obj.get("@context").or(parent_context);

    // Extract @type
    let types = extract_types(obj);

    // Warn if no @type and this isn't a pure @id reference
    if types.is_empty() {
        let non_meta_keys = obj.keys().filter(|k| !k.starts_with('@')).count();
        let is_reference = obj.contains_key("@id") && non_meta_keys == 0;
        if !is_reference && !obj.is_empty() {
            warnings.push(ExtractionWarning {
                message: "JSON-LD object has no @type".into(),
                source_location: source_location.cloned(),
                code: WarningCode::EmptyType,
            });
        }
    }

    // Build properties
    let mut properties: IndexMap<String, Vec<SchemaValue>> = IndexMap::new();

    for (key, val) in obj {
        if key == "@context" || key == "@type" {
            continue;
        }

        if key == "@id" {
            if let Value::String(id) = val {
                properties
                    .entry(key.clone())
                    .or_default()
                    .push(classify_text_value(id));
            }
            continue;
        }

        let values = json_to_schema_values(val, context, source_location, warnings, depth);
        if !values.is_empty() {
            properties.entry(key.clone()).or_default().extend(values);
        }
    }

    Some(SchemaNode {
        types,
        properties,
        source_format: SourceFormat::JsonLd,
        source_location: source_location.cloned(),
    })
}

/// Extract `@type` from a JSON-LD object, stripping Schema.org prefixes.
fn extract_types(obj: &serde_json::Map<String, Value>) -> Vec<String> {
    match obj.get("@type") {
        Some(Value::String(t)) => vec![strip_schema_prefix(t).into_owned()],
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str())
            .map(|s| strip_schema_prefix(s).into_owned())
            .collect(),
        _ => Vec::new(),
    }
}

/// Converts a JSON value into [`SchemaValue`]s.
fn json_to_schema_values(
    value: &Value,
    context: Option<&Value>,
    source_location: Option<&SourceLocation>,
    warnings: &mut Vec<ExtractionWarning>,
    depth: usize,
) -> Vec<SchemaValue> {
    match value {
        Value::Null => Vec::new(),
        Value::Bool(b) => vec![SchemaValue::Boolean(*b)],
        Value::Number(n) => n
            .as_f64()
            .map(|f| vec![SchemaValue::Number(f)])
            .unwrap_or_default(),
        Value::String(s) => vec![classify_text_value(s)],
        Value::Array(arr) => arr
            .iter()
            .flat_map(|v| json_to_schema_values(v, context, source_location, warnings, depth))
            .collect(),
        Value::Object(_) => json_to_node(value, context, source_location, warnings, depth + 1)
            .map(|node| vec![SchemaValue::Node(Box::new(node))])
            .unwrap_or_default(),
    }
}

// @id cross-reference resolution
/// Resolves `{"@id": "..."}` references throughout the node tree.
///
/// Total resolutions are capped at [`MAX_REF_RESOLUTIONS`] to prevent
/// memory amplification from many references to the same large node.
fn resolve_references(
    nodes: &mut [SchemaNode],
    id_map: &HashMap<String, SchemaNode>,
    warnings: &mut Vec<ExtractionWarning>,
) {
    let mut resolution_count: usize = 0;
    for node in nodes.iter_mut() {
        resolve_node_refs(node, id_map, warnings, 0, &mut resolution_count);
    }
}

/// Recursively resolves references within a single node.
///
/// Depth is limited to [`MAX_REF_DEPTH`] and total resolutions to
/// [`MAX_REF_RESOLUTIONS`] to prevent unbounded amplification.
fn resolve_node_refs(
    node: &mut SchemaNode,
    id_map: &HashMap<String, SchemaNode>,
    warnings: &mut Vec<ExtractionWarning>,
    depth: usize,
    resolution_count: &mut usize,
) {
    if depth > MAX_REF_DEPTH {
        return;
    }

    for values in node.properties.values_mut() {
        for value in values.iter_mut() {
            if let SchemaValue::Node(inner) = value {
                // Is this a pure @id reference? (no types, only @-prefixed keys)
                if inner.types.is_empty() {
                    if let Some(id_values) = inner.properties.get("@id") {
                        if let Some(SchemaValue::Text(id)) = id_values.first() {
                            if *resolution_count >= MAX_REF_RESOLUTIONS {
                                continue;
                            }
                            if let Some(resolved) = id_map.get(id.as_str()) {
                                let has_content =
                                    !resolved.types.is_empty() || resolved.properties.len() > 1;
                                if has_content {
                                    *resolution_count += 1;
                                    *value = SchemaValue::Node(Box::new(resolved.clone()));
                                    if let SchemaValue::Node(ref mut n) = value {
                                        resolve_node_refs(
                                            n,
                                            id_map,
                                            warnings,
                                            depth + 1,
                                            resolution_count,
                                        );
                                    }
                                    continue;
                                }
                            }
                            // Only warn for fragment references (e.g. "#foo").
                            // External @id URIs (e.g. "https://example.com/org/1")
                            // are valid and should not trigger warnings.
                            if id.starts_with('#') {
                                warnings.push(ExtractionWarning {
                                    message: format!("unresolvable @id reference: {id}"),
                                    source_location: inner.source_location.clone(),
                                    code: WarningCode::UnresolvableReference,
                                });
                            }
                            continue;
                        }
                    }
                }
                // Recurse into non-reference nested nodes
                resolve_node_refs(inner, id_map, warnings, depth + 1, resolution_count);
            }
        }
    }
}

// Lazy @id reference collection
/// Collects all `@id` values that appear as references (not definitions) in the node tree.
///
/// A reference is a `SchemaValue::Node` with no types and only an `@id` property.
/// This is used to determine which nodes need to be cloned for resolution.
fn collect_referenced_ids(nodes: &[SchemaNode]) -> HashSet<String> {
    let mut refs = HashSet::new();
    for node in nodes {
        collect_refs_in_node(node, &mut refs, 0);
    }
    refs
}

/// Recursively collects `@id` reference strings from a node's properties.
///
/// Depth is limited to [`MAX_DEPTH`] to prevent unbounded recursion
/// on pathological input.
fn collect_refs_in_node(node: &SchemaNode, refs: &mut HashSet<String>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    for values in node.properties.values() {
        for value in values {
            if let SchemaValue::Node(inner) = value {
                if inner.types.is_empty() {
                    if let Some(id_values) = inner.properties.get("@id") {
                        if let Some(SchemaValue::Text(id)) = id_values.first() {
                            refs.insert(id.clone());
                            continue;
                        }
                    }
                }
                collect_refs_in_node(inner, refs, depth + 1);
            }
        }
    }
}

// Source-location utilities
/// Maps byte offsets to line/column positions.
struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        for (i, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self { line_starts }
    }

    fn location(&self, byte_offset: usize) -> SourceLocation {
        let line = self
            .line_starts
            .partition_point(|&start| start <= byte_offset)
            .saturating_sub(1);
        let column = byte_offset.saturating_sub(self.line_starts[line]);
        SourceLocation {
            line: line + 1,
            column: column + 1,
            byte_offset,
        }
    }
}

/// Finds byte offsets of `<script type="application/ld+json">` tags.
fn find_script_byte_offsets(html: &str) -> Vec<usize> {
    let mut offsets = Vec::new();
    let mut search_from = 0;
    let pattern = "application/ld+json";

    while let Some(pos) = html[search_from..].find(pattern) {
        let abs_pos = search_from + pos;
        if let Some(tag_start) = html[..abs_pos].rfind('<') {
            if html[tag_start..abs_pos].contains("script") {
                offsets.push(tag_start);
            }
        }
        search_from = abs_pos + pattern.len();
    }

    offsets
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn line_index_positions() {
        let idx = LineIndex::new("line1\nline2\nline3");
        let loc = idx.location(0);
        assert_eq!((loc.line, loc.column), (1, 1));
        let loc = idx.location(6);
        assert_eq!((loc.line, loc.column), (2, 1));
        let loc = idx.location(8);
        assert_eq!((loc.line, loc.column), (2, 3));
    }

    #[test]
    fn find_script_offsets() {
        let html =
            r#"<html><script type="application/ld+json">{"@type":"Product"}</script></html>"#;
        let offsets = find_script_byte_offsets(html);
        assert_eq!(offsets.len(), 1);
        assert!(html[offsets[0]..].starts_with("<script"));
    }

    #[test]
    fn basic_product() {
        let html = r#"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "Product",
  "name": "Example Product",
  "url": "https://example.com/product"
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert_eq!(out.nodes[0].types, vec!["Product"]);
        assert_eq!(out.nodes[0].source_format, SourceFormat::JsonLd);
        assert_eq!(
            out.nodes[0].properties["name"],
            vec![SchemaValue::Text("Example Product".into())]
        );
        assert_eq!(
            out.nodes[0].properties["url"],
            vec![SchemaValue::Url("https://example.com/product".into())]
        );
    }

    #[test]
    fn graph_extraction() {
        let html = r#"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@graph": [
    {"@type": "Organization", "name": "Acme"},
    {"@type": "WebSite", "name": "Acme Site"}
  ]
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 2);
        assert_eq!(out.nodes[0].types, vec!["Organization"]);
        assert_eq!(out.nodes[1].types, vec!["WebSite"]);
    }

    #[test]
    fn array_type() {
        let html = r#"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": ["Product", "IndividualProduct"],
  "name": "Widget"
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes[0].types, vec!["Product", "IndividualProduct"]);
    }

    #[test]
    fn nested_object() {
        let html = r#"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "Product",
  "name": "Widget",
  "offers": {
    "@type": "Offer",
    "price": 19.99,
    "priceCurrency": "USD"
  }
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        let offers = &out.nodes[0].properties["offers"];
        assert_eq!(offers.len(), 1);
        if let SchemaValue::Node(offer) = &offers[0] {
            assert_eq!(offer.types, vec!["Offer"]);
            assert_eq!(offer.properties["price"], vec![SchemaValue::Number(19.99)]);
            assert_eq!(
                offer.properties["priceCurrency"],
                vec![SchemaValue::Text("USD".into())]
            );
        } else {
            panic!("Expected nested Node");
        }
    }

    #[test]
    fn id_cross_reference() {
        let html = r##"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@graph": [
    {"@type": "Product", "name": "Widget", "offers": {"@id": "#offer1"}},
    {"@id": "#offer1", "@type": "Offer", "price": 29.99}
  ]
}</script></head></html>"##;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 2);
        let offers = &out.nodes[0].properties["offers"];
        if let SchemaValue::Node(offer) = &offers[0] {
            assert_eq!(offer.types, vec!["Offer"]);
            assert_eq!(offer.properties["price"], vec![SchemaValue::Number(29.99)]);
        } else {
            panic!("Expected resolved Node, got {:?}", offers[0]);
        }
    }

    #[test]
    fn malformed_json_is_warning() {
        let html =
            r#"<html><head><script type="application/ld+json">{ invalid }</script></head></html>"#;
        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert!(out.nodes.is_empty());
        assert_eq!(out.warnings.len(), 1);
        assert_eq!(out.warnings[0].code, WarningCode::MalformedJsonLd);
    }

    #[test]
    fn empty_script_tag() {
        let html = r#"<html><head><script type="application/ld+json"></script></head></html>"#;
        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert!(out.nodes.is_empty());
        assert_eq!(out.warnings[0].code, WarningCode::MalformedJsonLd);
    }

    #[test]
    fn multiple_script_tags() {
        let html = r#"<html><head>
<script type="application/ld+json">{"@context":"https://schema.org","@type":"Product","name":"A"}</script>
<script type="application/ld+json">{"@context":"https://schema.org","@type":"Article","name":"B"}</script>
</head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 2);
        assert_eq!(out.nodes[0].types, vec!["Product"]);
        assert_eq!(out.nodes[1].types, vec!["Article"]);
    }

    #[test]
    fn top_level_array() {
        let html = r#"<html><head><script type="application/ld+json">[
  {"@context":"https://schema.org","@type":"Product","name":"A"},
  {"@context":"https://schema.org","@type":"Article","name":"B"}
]</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 2);
        assert_eq!(out.nodes[0].types, vec!["Product"]);
        assert_eq!(out.nodes[1].types, vec!["Article"]);
    }

    #[test]
    fn boolean_and_number_values() {
        let html = r#"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "Product",
  "isFamilyFriendly": true,
  "weight": 1.5
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["isFamilyFriendly"],
            vec![SchemaValue::Boolean(true)]
        );
        assert_eq!(
            out.nodes[0].properties["weight"],
            vec![SchemaValue::Number(1.5)]
        );
    }

    #[test]
    fn unresolvable_reference_warns() {
        let html = r##"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "Product",
  "offers": {"@id": "#nonexistent"}
}</script></head></html>"##;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert!(out
            .warnings
            .iter()
            .any(|w| w.code == WarningCode::UnresolvableReference));
    }

    #[test]
    fn no_context_with_full_uri_type() {
        let html = r#"<html><head><script type="application/ld+json">{
  "@type": "https://schema.org/Product",
  "name": "Widget"
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert_eq!(out.nodes[0].types, vec!["Product"]);
    }

    #[test]
    fn array_context() {
        let html = r#"<html><head><script type="application/ld+json">{
  "@context": ["https://schema.org", {"custom": "https://example.com/"}],
  "@type": "Product",
  "name": "Widget"
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes[0].types, vec!["Product"]);
    }

    #[test]
    fn array_property_values() {
        let html = r#"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "Product",
  "name": "Widget",
  "image": [
    "https://example.com/img1.jpg",
    "https://example.com/img2.jpg"
  ]
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes[0].properties["image"].len(), 2);
        assert_eq!(
            out.nodes[0].properties["image"][0],
            SchemaValue::Url("https://example.com/img1.jpg".into())
        );
    }

    #[test]
    fn null_values_are_skipped() {
        let html = r#"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "Product",
  "name": "Widget",
  "description": null
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        // null values should be skipped entirely
        assert!(!out.nodes[0].properties.contains_key("description"));
    }

    #[test]
    fn integer_numbers() {
        let html = r#"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "Product",
  "name": "Widget",
  "ratingCount": 42
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["ratingCount"],
            vec![SchemaValue::Number(42.0)]
        );
    }

    #[test]
    fn graph_context_inherited_by_children() {
        let html = r#"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@graph": [
    {"@type": "Product", "name": "A"},
    {"@type": "https://schema.org/Article", "name": "B"}
  ]
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 2);
        assert_eq!(out.nodes[0].types, vec!["Product"]);
        assert_eq!(out.nodes[1].types, vec!["Article"]);
    }

    #[test]
    fn duplicate_id_warns() {
        let html = r##"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@graph": [
    {"@id": "#thing", "@type": "Product", "name": "First"},
    {"@id": "#thing", "@type": "Article", "name": "Second"}
  ]
}</script></head></html>"##;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert!(out
            .warnings
            .iter()
            .any(|w| w.code == WarningCode::DuplicateId));
    }

    #[test]
    fn deeply_nested_objects() {
        let html = r#"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "Product",
  "name": "Widget",
  "offers": {
    "@type": "Offer",
    "seller": {
      "@type": "Organization",
      "address": {
        "@type": "PostalAddress",
        "addressCountry": "US"
      }
    }
  }
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        let offers = &out.nodes[0].properties["offers"];
        if let SchemaValue::Node(offer) = &offers[0] {
            let seller = &offer.properties["seller"];
            if let SchemaValue::Node(org) = &seller[0] {
                let address = &org.properties["address"];
                if let SchemaValue::Node(addr) = &address[0] {
                    assert_eq!(addr.types, vec!["PostalAddress"]);
                    assert_eq!(
                        addr.properties["addressCountry"],
                        vec![SchemaValue::Text("US".into())]
                    );
                } else {
                    panic!("Expected PostalAddress node");
                }
            } else {
                panic!("Expected Organization node");
            }
        } else {
            panic!("Expected Offer node");
        }
    }

    #[test]
    fn whitespace_only_script() {
        let html = r#"<html><head><script type="application/ld+json">   
  
  </script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert!(out.nodes.is_empty());
        assert_eq!(out.warnings.len(), 1);
        assert_eq!(out.warnings[0].code, WarningCode::MalformedJsonLd);
    }

    #[test]
    fn source_location_is_set() {
        let html = "<html><head>\n<script type=\"application/ld+json\">\n{\"@type\":\"Product\",\"name\":\"A\"}\n</script>\n</head></html>";

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        let loc = out.nodes[0]
            .source_location
            .as_ref()
            .expect("missing source location");
        // The <script> tag starts on line 2
        assert_eq!(loc.line, 2);
    }

    #[test]
    fn multiple_types_with_uri_prefix() {
        let html = r#"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": ["https://schema.org/Product", "http://schema.org/IndividualProduct"],
  "name": "Widget"
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes[0].types, vec!["Product", "IndividualProduct"]);
    }

    #[test]
    fn schema_node_id_accessor() {
        let html = r##"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@id": "#product1",
  "@type": "Product",
  "name": "Widget"
}</script></head></html>"##;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes[0].id(), Some("#product1"));
    }

    #[test]
    fn no_structured_data() {
        let html = r#"<html><head><title>No structured data</title></head>
<body><p>Hello world</p></body></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert!(out.nodes.is_empty());
        assert!(out.warnings.is_empty());
    }

    #[test]
    fn json_ld_with_trailing_comma() {
        // Many real-world sites have trailing commas in JSON-LD (invalid JSON)
        let html = r#"<html><head><script type="application/ld+json">{
  "@type": "Product",
  "name": "Widget",
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert!(out.nodes.is_empty());
        assert_eq!(out.warnings[0].code, WarningCode::MalformedJsonLd);
    }

    #[test]
    fn circular_id_references_do_not_loop() {
        // A references B, B references A -- must terminate
        let html = r##"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@graph": [
    {"@id": "#a", "@type": "Product", "name": "A", "isRelatedTo": {"@id": "#b"}},
    {"@id": "#b", "@type": "Article", "name": "B", "isRelatedTo": {"@id": "#a"}}
  ]
}</script></head></html>"##;

        let out = JsonLdExtractor.extract(html).expect("must not hang");
        assert_eq!(out.nodes.len(), 2);
    }

    #[test]
    fn self_referencing_id_does_not_loop() {
        let html = r##"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@graph": [
    {"@id": "#self", "@type": "Product", "name": "Me", "isRelatedTo": {"@id": "#self"}}
  ]
}</script></head></html>"##;

        let out = JsonLdExtractor.extract(html).expect("must not hang");
        assert_eq!(out.nodes.len(), 1);
    }

    #[test]
    fn empty_id_string() {
        let html = r##"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@id": "",
  "@type": "Product",
  "name": "Widget"
}</script></head></html>"##;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        // Empty @id is stored but should not cause issues
        assert_eq!(out.nodes[0].id(), Some(""));
    }

    #[test]
    fn nesting_at_exactly_max_depth_succeeds() {
        // Build JSON with exactly MAX_DEPTH (20) levels of nesting
        let mut json =
            String::from(r#"{"@context":"https://schema.org","@type":"Thing","name":"L0""#);
        for i in 1..MAX_DEPTH {
            json.push_str(&format!(r#","p{i}":{{"@type":"Thing","name":"L{i}""#));
        }
        // Close nested objects (one } per level) plus the root
        for _ in 0..MAX_DEPTH {
            json.push('}');
        }

        let html = format!(
            r#"<html><head><script type="application/ld+json">{json}</script></head></html>"#
        );

        let out = JsonLdExtractor.extract(&html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        // No depth warning should fire at exactly MAX_DEPTH
        assert!(
            !out.warnings.iter().any(|w| w.message.contains("depth")),
            "should not warn at MAX_DEPTH"
        );
    }

    #[test]
    fn nesting_beyond_max_depth_warns() {
        // Build JSON with MAX_DEPTH + 2 levels of nesting
        let target = MAX_DEPTH + 2;
        let mut json =
            String::from(r#"{"@context":"https://schema.org","@type":"Thing","name":"L0""#);
        for i in 1..target {
            json.push_str(&format!(r#","p{i}":{{"@type":"Thing","name":"L{i}""#));
        }
        // Close nested objects plus the root
        for _ in 0..target {
            json.push('}');
        }

        let html = format!(
            r#"<html><head><script type="application/ld+json">{json}</script></head></html>"#
        );

        let out = JsonLdExtractor.extract(&html).expect("extraction failed");
        assert!(
            out.warnings.iter().any(|w| w.message.contains("depth")),
            "should warn when exceeding MAX_DEPTH"
        );
    }

    #[test]
    fn type_is_number_ignored() {
        let html = r#"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": 42,
  "name": "Widget"
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert!(out.nodes[0].types.is_empty());
        assert!(out
            .warnings
            .iter()
            .any(|w| w.code == WarningCode::EmptyType));
    }

    #[test]
    fn type_is_object_ignored() {
        let html = r#"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": {"invalid": true},
  "name": "Widget"
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert!(out.nodes[0].types.is_empty());
    }

    #[test]
    fn type_empty_array() {
        let html = r#"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": [],
  "name": "Widget"
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert!(out.nodes[0].types.is_empty());
        assert!(out
            .warnings
            .iter()
            .any(|w| w.code == WarningCode::EmptyType));
    }

    #[test]
    fn type_array_with_mixed_values() {
        // Non-string values in @type array should be filtered out
        let html = r#"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": [42, "Product", null, "IndividualProduct"],
  "name": "Widget"
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes[0].types, vec!["Product", "IndividualProduct"]);
    }

    #[test]
    fn non_schema_org_context_still_extracts() {
        let html = r#"<html><head><script type="application/ld+json">{
  "@context": "https://w3.org/ns/activitystreams",
  "@type": "Note",
  "content": "Hello"
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert_eq!(out.nodes[0].types, vec!["Note"]);
        assert_eq!(
            out.nodes[0].properties["content"],
            vec![SchemaValue::Text("Hello".into())]
        );
    }

    #[test]
    fn html_entities_in_script_content() {
        // html5ever decodes HTML entities in script text content
        let html = r#"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "Product",
  "name": "Widget &amp; Gadget"
}</script></head></html>"#;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        // serde_json will see the decoded "&" or the raw "&amp;" depending
        // on how html5ever handles script content. Either way, extraction
        // should succeed without error.
        assert_eq!(out.nodes.len(), 1);
    }

    #[test]
    fn multiple_references_to_same_id() {
        // Three properties all reference the same @id node
        let html = r##"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@graph": [
    {
      "@type": "Product", "name": "Widget",
      "offers": {"@id": "#offer"},
      "makesOffer": {"@id": "#offer"},
      "hasOfferCatalog": {"@id": "#offer"}
    },
    {"@id": "#offer", "@type": "Offer", "price": 9.99}
  ]
}</script></head></html>"##;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 2);
        // All three references should be resolved
        for prop in &["offers", "makesOffer", "hasOfferCatalog"] {
            let values = &out.nodes[0].properties[*prop];
            if let SchemaValue::Node(node) = &values[0] {
                assert_eq!(node.types, vec!["Offer"]);
            } else {
                panic!("Expected resolved Node for {prop}");
            }
        }
    }

    #[test]
    fn duplicate_id_first_definition_wins() {
        // Verify first-wins semantics after the bug fix
        let html = r##"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@graph": [
    {"@type": "Product", "name": "P", "offers": {"@id": "#dup"}},
    {"@id": "#dup", "@type": "Offer", "price": 10.00, "priceCurrency": "USD"},
    {"@id": "#dup", "@type": "Offer", "price": 99.99, "priceCurrency": "EUR"}
  ]
}</script></head></html>"##;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        // Should warn about duplicate
        assert!(out
            .warnings
            .iter()
            .any(|w| w.code == WarningCode::DuplicateId));
        // The FIRST definition (price=10.00, USD) should win
        let offers = &out.nodes[0].properties["offers"];
        if let SchemaValue::Node(offer) = &offers[0] {
            assert_eq!(
                offer.properties["price"],
                vec![SchemaValue::Number(10.0)],
                "first @id definition should win"
            );
            assert_eq!(
                offer.properties["priceCurrency"],
                vec![SchemaValue::Text("USD".into())],
                "first @id definition should win"
            );
        } else {
            panic!("Expected resolved Offer node");
        }
    }

    #[test]
    fn json_root_is_string_warns() {
        let html = r#"<html><head><script type="application/ld+json">"just a string"</script></head></html>"#;
        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert!(out.nodes.is_empty());
        assert_eq!(out.warnings[0].code, WarningCode::MalformedJsonLd);
    }

    #[test]
    fn json_root_is_number_warns() {
        let html = r#"<html><head><script type="application/ld+json">42</script></head></html>"#;
        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert!(out.nodes.is_empty());
        assert_eq!(out.warnings[0].code, WarningCode::MalformedJsonLd);
    }

    #[test]
    fn external_uri_id_no_warning() {
        // External @id URIs should NOT produce unresolvable-reference warnings
        let html = r##"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "Product",
  "name": "Widget",
  "manufacturer": {"@id": "https://example.com/org/1"}
}</script></head></html>"##;

        let out = JsonLdExtractor.extract(html).expect("extraction failed");
        assert!(
            !out.warnings
                .iter()
                .any(|w| w.code == WarningCode::UnresolvableReference),
            "external @id URIs should not trigger warnings"
        );
    }
}
