//! JSON-LD extractor — parses `<script type="application/ld+json">` tags.
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

use std::collections::HashMap;

use indexmap::IndexMap;
use scraper::{Html, Selector};
use serde_json::Value;

use crate::error::{ExtractionError, ExtractionWarning, WarningCode};
use crate::types::{SchemaNode, SchemaValue, SourceFormat, SourceLocation};

use super::{ExtractionOutput, Extractor};

/// Extracts Schema.org structured data from JSON-LD `<script>` tags.
pub struct JsonLdExtractor;

impl Extractor for JsonLdExtractor {
    fn extract(&self, html: &str) -> Result<ExtractionOutput, ExtractionError> {
        let document = Html::parse_document(html);
        let selector = Selector::parse("script[type=\"application/ld+json\"]")
            .map_err(|_| ExtractionError::Internal("failed to parse CSS selector".into()))?;

        let line_index = LineIndex::new(html);
        let script_offsets = find_script_byte_offsets(html);

        let mut all_nodes = Vec::new();
        let mut warnings = Vec::new();
        let mut id_map: HashMap<String, SchemaNode> = HashMap::new();

        for (idx, element) in document.select(&selector).enumerate() {
            let json_text = element.inner_html();
            let trimmed = json_text.trim();
            let source_location = script_offsets
                .get(idx)
                .map(|&offset| line_index.location(offset));

            if trimmed.is_empty() {
                warnings.push(ExtractionWarning {
                    message: "Empty JSON-LD script tag".into(),
                    source_location,
                    code: WarningCode::MalformedJsonLd,
                });
                continue;
            }

            let value: Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(e) => {
                    warnings.push(ExtractionWarning {
                        message: format!("Failed to parse JSON-LD: {e}"),
                        source_location,
                        code: WarningCode::MalformedJsonLd,
                    });
                    continue;
                }
            };

            let items = extract_json_items(&value, &source_location, &mut warnings);

            for node in items {
                if let Some(id) = node.id() {
                    if id_map.contains_key(id) {
                        warnings.push(ExtractionWarning {
                            message: format!("Duplicate @id: {id}"),
                            source_location: source_location.clone(),
                            code: WarningCode::DuplicateId,
                        });
                    }
                    id_map.insert(id.to_owned(), node.clone());
                }
                all_nodes.push(node);
            }
        }

        // Second pass: resolve @id cross-references
        resolve_references(&mut all_nodes, &id_map, &mut warnings);

        Ok(ExtractionOutput {
            nodes: all_nodes,
            warnings,
        })
    }
}

// ---------------------------------------------------------------------------
// JSON → SchemaNode conversion
// ---------------------------------------------------------------------------

/// Extract top-level Schema.org items from a parsed JSON value.
fn extract_json_items(
    value: &Value,
    source_location: &Option<SourceLocation>,
    warnings: &mut Vec<ExtractionWarning>,
) -> Vec<SchemaNode> {
    match value {
        Value::Array(items) => items
            .iter()
            .filter_map(|item| json_to_node(item, None, source_location, warnings))
            .collect(),

        Value::Object(map) => {
            if let Some(Value::Array(graph_items)) = map.get("@graph") {
                let context = map.get("@context");
                graph_items
                    .iter()
                    .filter_map(|item| json_to_node(item, context, source_location, warnings))
                    .collect()
            } else {
                json_to_node(value, None, source_location, warnings)
                    .into_iter()
                    .collect()
            }
        }

        _ => {
            warnings.push(ExtractionWarning {
                message: "JSON-LD root must be an object or array".into(),
                source_location: source_location.clone(),
                code: WarningCode::MalformedJsonLd,
            });
            Vec::new()
        }
    }
}

/// Convert a JSON object to a [`SchemaNode`].
///
/// `parent_context` is the `@context` inherited from a `@graph` wrapper.
fn json_to_node(
    value: &Value,
    parent_context: Option<&Value>,
    source_location: &Option<SourceLocation>,
    warnings: &mut Vec<ExtractionWarning>,
) -> Option<SchemaNode> {
    let obj = value.as_object()?;

    // Resolve @context: local overrides parent
    let context = obj.get("@context").or(parent_context);

    // Extract @type
    let types = extract_types(obj, context);

    // Warn if no @type and this isn't a pure @id reference
    if types.is_empty() {
        let non_meta_keys = obj.keys().filter(|k| !k.starts_with('@')).count();
        let is_reference = obj.contains_key("@id") && non_meta_keys == 0;
        if !is_reference && !obj.is_empty() {
            warnings.push(ExtractionWarning {
                message: "JSON-LD object has no @type".into(),
                source_location: source_location.clone(),
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
                    .push(SchemaValue::Text(id.clone()));
            }
            continue;
        }

        let values = json_to_schema_values(val, context, source_location, warnings);
        if !values.is_empty() {
            properties.entry(key.clone()).or_default().extend(values);
        }
    }

    Some(SchemaNode {
        types,
        properties,
        source_format: SourceFormat::JsonLd,
        source_location: source_location.clone(),
    })
}

/// Extract `@type` from a JSON-LD object, stripping Schema.org prefixes.
fn extract_types(obj: &serde_json::Map<String, Value>, context: Option<&Value>) -> Vec<String> {
    match obj.get("@type") {
        Some(Value::String(t)) => vec![strip_schema_prefix(t, context)],
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str())
            .map(|t| strip_schema_prefix(t, context))
            .collect(),
        _ => Vec::new(),
    }
}

/// Strip `https://schema.org/` and similar prefixes from a type name.
fn strip_schema_prefix(type_name: &str, _context: Option<&Value>) -> String {
    const PREFIXES: &[&str] = &["https://schema.org/", "http://schema.org/", "schema:"];

    for prefix in PREFIXES {
        if let Some(stripped) = type_name.strip_prefix(prefix) {
            return stripped.to_string();
        }
    }

    type_name.to_string()
}

/// Convert a JSON value into [`SchemaValue`]s.
fn json_to_schema_values(
    value: &Value,
    context: Option<&Value>,
    source_location: &Option<SourceLocation>,
    warnings: &mut Vec<ExtractionWarning>,
) -> Vec<SchemaValue> {
    match value {
        Value::Null => Vec::new(),
        Value::Bool(b) => vec![SchemaValue::Boolean(*b)],
        Value::Number(n) => n
            .as_f64()
            .map(|f| vec![SchemaValue::Number(f)])
            .unwrap_or_default(),
        Value::String(s) => vec![classify_string_value(s)],
        Value::Array(arr) => arr
            .iter()
            .flat_map(|v| json_to_schema_values(v, context, source_location, warnings))
            .collect(),
        Value::Object(_) => json_to_node(value, context, source_location, warnings)
            .map(|node| vec![SchemaValue::Node(Box::new(node))])
            .unwrap_or_default(),
    }
}

/// Classify a string as [`SchemaValue::Text`], [`SchemaValue::Url`], or [`SchemaValue::DateTime`].
fn classify_string_value(s: &str) -> SchemaValue {
    if s.starts_with("http://") || s.starts_with("https://") || s.starts_with("mailto:") {
        return SchemaValue::Url(s.to_string());
    }

    if is_iso_datetime(s) {
        return SchemaValue::DateTime(s.to_string());
    }

    SchemaValue::Text(s.to_string())
}

/// Check if a string matches an ISO 8601 date/datetime pattern (`YYYY-MM-DD...`).
fn is_iso_datetime(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() < 10 {
        return false;
    }
    bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
}

// ---------------------------------------------------------------------------
// @id cross-reference resolution
// ---------------------------------------------------------------------------

/// Resolve `{"@id": "..."}` references throughout the node tree.
fn resolve_references(
    nodes: &mut [SchemaNode],
    id_map: &HashMap<String, SchemaNode>,
    warnings: &mut Vec<ExtractionWarning>,
) {
    for node in nodes.iter_mut() {
        resolve_node_refs(node, id_map, warnings, 0);
    }
}

/// Recursively resolve references within a single node (depth-limited to 10).
fn resolve_node_refs(
    node: &mut SchemaNode,
    id_map: &HashMap<String, SchemaNode>,
    warnings: &mut Vec<ExtractionWarning>,
    depth: usize,
) {
    if depth > 10 {
        return;
    }

    for values in node.properties.values_mut() {
        for value in values.iter_mut() {
            if let SchemaValue::Node(inner) = value {
                // Is this a pure @id reference? (no types, only @-prefixed keys)
                if inner.types.is_empty() {
                    if let Some(id_values) = inner.properties.get("@id") {
                        if let Some(SchemaValue::Text(id)) = id_values.first() {
                            if let Some(resolved) = id_map.get(id.as_str()) {
                                let has_content =
                                    !resolved.types.is_empty() || resolved.properties.len() > 1;
                                if has_content {
                                    *value = SchemaValue::Node(Box::new(resolved.clone()));
                                    if let SchemaValue::Node(ref mut n) = value {
                                        resolve_node_refs(n, id_map, warnings, depth + 1);
                                    }
                                    continue;
                                }
                            }
                            warnings.push(ExtractionWarning {
                                message: format!("Unresolvable @id reference: {id}"),
                                source_location: inner.source_location.clone(),
                                code: WarningCode::UnresolvableReference,
                            });
                        }
                    }
                }
                // Recurse into non-reference nested nodes
                resolve_node_refs(inner, id_map, warnings, depth + 1);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Source-location utilities
// ---------------------------------------------------------------------------

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

/// Find byte offsets of `<script type="application/ld+json">` tags.
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

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
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
    fn strip_schema_prefixes() {
        assert_eq!(strip_schema_prefix("Product", None), "Product");
        assert_eq!(
            strip_schema_prefix("https://schema.org/Product", None),
            "Product"
        );
        assert_eq!(
            strip_schema_prefix("http://schema.org/Product", None),
            "Product"
        );
        assert_eq!(strip_schema_prefix("schema:Product", None), "Product");
    }

    #[test]
    fn classify_string_values() {
        assert_eq!(
            classify_string_value("hello"),
            SchemaValue::Text("hello".into())
        );
        assert_eq!(
            classify_string_value("https://example.com"),
            SchemaValue::Url("https://example.com".into())
        );
        assert_eq!(
            classify_string_value("2024-01-15"),
            SchemaValue::DateTime("2024-01-15".into())
        );
        assert_eq!(
            classify_string_value("2024-01-15T10:30:00Z"),
            SchemaValue::DateTime("2024-01-15T10:30:00Z".into())
        );
    }

    #[test]
    fn iso_datetime_detection() {
        assert!(is_iso_datetime("2024-01-15"));
        assert!(is_iso_datetime("2024-01-15T10:30:00"));
        assert!(is_iso_datetime("2024-01-15T10:30:00Z"));
        assert!(!is_iso_datetime("hello"));
        assert!(!is_iso_datetime("2024"));
        assert!(!is_iso_datetime("not-a-date"));
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
}
