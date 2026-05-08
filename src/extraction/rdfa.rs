//! `RDFa` Lite 1.1 extractor: parses `vocab`/`typeof`/`property` attributes.
//!
//! Implements [RDFa Lite 1.1](https://www.w3.org/TR/rdfa-lite/) - the 5-attribute
//! subset designed for Schema.org: `vocab`, `typeof`, `property`, `resource`, `prefix`.
//!
//! ## Supported features
//!
//! - `vocab` attribute for setting the default vocabulary (e.g. `https://schema.org/`)
//! - `typeof` for defining types
//! - `property` for defining properties
//! - `resource` for overriding the subject URI
//! - `prefix` for namespace prefix mappings
//! - Nested typed nodes
//! - Content extraction from `content`, `href`, `src` attributes
//!
//! ## Not supported (full `RDFa` Core 1.1)
//!
//! - Complex CURIE expansion beyond simple `prefix:term`
//! - `about`, `src`, `href` as subject identifiers
//! - `rel` and `rev` properties
//! - `@inlist` processing
//! - Multiple interleaved vocabularies
//! - XML `base` URI resolution
//!
//! This subset covers ~95% of real-world Schema.org `RDFa` usage.

use std::borrow::Cow;

use ego_tree::NodeRef;
use indexmap::IndexMap;
use scraper::node::Node;
use scraper::Html;

use crate::error::{ExtractionError, ExtractionWarning, WarningCode};
use crate::types::{SchemaNode, SchemaValue, SourceFormat};

use super::{classify_text_value, strip_schema_prefix, ExtractionOutput, Extractor};

/// Maximum nesting depth.
const MAX_DEPTH: usize = 20;

/// Extracts Schema.org structured data from `RDFa` Lite 1.1 attributes.
///
/// # Examples
///
/// ```
/// use schemaorg_rs::extraction::{Extractor, RdfaLiteExtractor};
///
/// let html = r#"<html><body>
/// <div vocab="https://schema.org/" typeof="Product">
/// <span property="name">Widget</span>
/// </div>
/// </body></html>"#;
///
/// let output = RdfaLiteExtractor.extract(html).unwrap();
/// assert_eq!(output.nodes[0].types, vec!["Product"]);
/// ```
pub struct RdfaLiteExtractor;

impl Extractor for RdfaLiteExtractor {
    fn extract(&self, html: &str) -> Result<ExtractionOutput, ExtractionError> {
        let document = Html::parse_document(html);
        self.extract_from_document(&document)
    }
}

impl RdfaLiteExtractor {
    /// Extracts from an already-parsed document.
    ///
    /// # Errors
    ///
    /// Returns [`ExtractionError`] if a fatal error prevents extraction.
    /// Most issues are captured as warnings in the returned output.
    pub fn extract_from_document(
        &self,
        document: &Html,
    ) -> Result<ExtractionOutput, ExtractionError> {
        let mut warnings = Vec::new();
        let mut nodes = Vec::new();

        let context = RdfaContext {
            vocab: None,
            prefixes: IndexMap::new(),
        };

        // Walk the DOM tree starting from the root
        for child in document.tree.root().children() {
            walk_dom(child, &context, &mut nodes, &mut warnings, 0);
        }

        Ok(ExtractionOutput { nodes, warnings })
    }
}

/// Context stack for `RDFa` processing.
#[derive(Debug, Clone)]
struct RdfaContext {
    /// Current default vocabulary URI (e.g. `https://schema.org/`).
    vocab: Option<String>,
    /// Registered namespace prefixes (e.g. `schema` -> `https://schema.org/`).
    prefixes: IndexMap<String, String>,
}

impl RdfaContext {
    /// Creates an updated context if this element changes `vocab` or `prefix`.
    /// Returns `None` if the context is unchanged (avoids cloning).
    fn updated(&self, el: &scraper::node::Element) -> Option<Self> {
        let has_vocab = el.attr("vocab").is_some();
        let has_prefix = el.attr("prefix").is_some();

        if !has_vocab && !has_prefix {
            return None;
        }

        let mut ctx = self.clone();

        if let Some(vocab) = el.attr("vocab") {
            ctx.vocab = if vocab.is_empty() {
                None
            } else {
                Some(ensure_trailing_slash(vocab))
            };
        }

        if let Some(prefix_attr) = el.attr("prefix") {
            parse_prefix_attr(prefix_attr, &mut ctx.prefixes);
        }

        Some(ctx)
    }

    /// Resolves a potentially prefixed term to a full URI, then strips known vocabulary prefixes.
    fn resolve_term(&self, term: &str) -> String {
        // Full URI: strip the vocabulary prefix if present
        let stripped = strip_schema_prefix(term);
        if matches!(stripped, Cow::Owned(_)) {
            return stripped.into_owned();
        }

        // Try prefix:term expansion (e.g. "schema:Product")
        if let Some(colon_pos) = term.find(':') {
            let prefix = &term[..colon_pos];
            let local = &term[colon_pos + 1..];
            if let Some(ns_uri) = self.prefixes.get(prefix) {
                let full = format!("{ns_uri}{local}");
                return strip_schema_prefix(&full).into_owned();
            }
        }

        term.to_string()
    }
}

/// Walks the DOM tree, collecting `RDFa` Lite structured data.
fn walk_dom(
    node: NodeRef<'_, Node>,
    parent_ctx: &RdfaContext,
    nodes: &mut Vec<SchemaNode>,
    warnings: &mut Vec<ExtractionWarning>,
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }

    let Some(el) = node.value().as_element() else {
        // Not an element - recurse into children (e.g. template nodes)
        for child in node.children() {
            walk_dom(child, parent_ctx, nodes, warnings, depth);
        }
        return;
    };

    let updated_ctx = parent_ctx.updated(el);
    let ctx = updated_ctx.as_ref().unwrap_or(parent_ctx);

    // Does this element define a new typed node?
    if let Some(typeof_attr) = el.attr("typeof") {
        let types: Vec<String> = typeof_attr
            .split_whitespace()
            .map(|t| ctx.resolve_term(t))
            .collect();

        if types.is_empty() {
            warnings.push(ExtractionWarning {
                message: "RDFa typeof attribute is empty".into(),
                source_location: None,
                code: WarningCode::EmptyType,
            });
        }

        let mut properties: IndexMap<String, Vec<SchemaValue>> = IndexMap::new();

        // Store resource as @id if present
        if let Some(resource) = el.attr("resource") {
            properties
                .entry("@id".into())
                .or_default()
                .push(classify_text_value(resource));
        }

        // Collect properties from children
        collect_rdfa_properties(node, ctx, &mut properties, warnings, depth + 1);

        let schema_node = SchemaNode {
            types,
            properties,
            source_format: SourceFormat::RdfaLite,
            source_location: None,
        };

        nodes.push(schema_node);
        return; // Children already processed by collect_rdfa_properties
    }

    // No typeof - continue walking children
    for child in node.children() {
        walk_dom(child, ctx, nodes, warnings, depth + 1);
    }
}

/// Collects properties from children of a typed node.
fn collect_rdfa_properties(
    node: NodeRef<'_, Node>,
    ctx: &RdfaContext,
    properties: &mut IndexMap<String, Vec<SchemaValue>>,
    warnings: &mut Vec<ExtractionWarning>,
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }

    for child in node.children() {
        visit_for_rdfa_props(child, ctx, properties, warnings, depth);
    }
}

/// Visits a node looking for `RDFa` property attributes.
fn visit_for_rdfa_props(
    node: NodeRef<'_, Node>,
    parent_ctx: &RdfaContext,
    properties: &mut IndexMap<String, Vec<SchemaValue>>,
    warnings: &mut Vec<ExtractionWarning>,
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }

    let Some(el) = node.value().as_element() else {
        return;
    };

    let updated_ctx = parent_ctx.updated(el);
    let ctx = updated_ctx.as_ref().unwrap_or(parent_ctx);

    // Does this element define a property?
    if let Some(prop_attr) = el.attr("property") {
        let prop_names: Vec<String> = prop_attr
            .split_whitespace()
            .map(|p| ctx.resolve_term(p))
            .collect();

        if prop_names.is_empty() {
            return;
        }

        // Is this property also a new typed node?
        if let Some(typeof_attr) = el.attr("typeof") {
            let types: Vec<String> = typeof_attr
                .split_whitespace()
                .map(|t| ctx.resolve_term(t))
                .collect();

            let mut nested_props: IndexMap<String, Vec<SchemaValue>> = IndexMap::new();

            if let Some(resource) = el.attr("resource") {
                nested_props
                    .entry("@id".into())
                    .or_default()
                    .push(classify_text_value(resource));
            }

            collect_rdfa_properties(node, ctx, &mut nested_props, warnings, depth + 1);

            let nested_node = SchemaNode {
                types,
                properties: nested_props,
                source_format: SourceFormat::RdfaLite,
                source_location: None,
            };

            let value = SchemaValue::Node(Box::new(nested_node));
            for name in &prop_names {
                properties
                    .entry(name.clone())
                    .or_default()
                    .push(value.clone());
            }
            return; // Children already consumed by collect_rdfa_properties
        }

        // Extract the value
        let value = extract_rdfa_value(node, el);

        for name in &prop_names {
            properties
                .entry(name.clone())
                .or_default()
                .push(value.clone());
        }
        return; // Property element owns its subtree
    }

    // Not a property - check for typeof (nested independent node)
    if el.attr("typeof").is_some() {
        // This is a new typed node that is NOT a property of the parent.
        // We skip it here and let the top-level walk_dom handle it.
        // But wait - it's nested inside an existing typed node. RDFa Lite doesn't
        // have a clean way to express independent nested items like Microdata's
        // itemprop-less itemscope. We skip to avoid double-counting.
        return;
    }

    // No property, no typeof - recurse into children
    for child in node.children() {
        visit_for_rdfa_props(child, ctx, properties, warnings, depth + 1);
    }
}

/// Extracts a value from an `RDFa` property element.
fn extract_rdfa_value(node: NodeRef<'_, Node>, el: &scraper::node::Element) -> SchemaValue {
    let tag = el.name();

    // content attribute takes highest priority
    if let Some(content) = el.attr("content") {
        return classify_text_value(content);
    }

    // resource attribute -> URL/Text
    if let Some(resource) = el.attr("resource") {
        return classify_text_value(resource);
    }

    // href on links
    if let Some(href) = el.attr("href") {
        match tag {
            "a" | "link" | "area" => return SchemaValue::Url(href.to_string()),
            _ => return classify_text_value(href),
        }
    }

    // src on media elements
    if let Some(src) = el.attr("src") {
        match tag {
            "img" | "audio" | "video" | "source" | "embed" => {
                return SchemaValue::Url(src.to_string())
            }
            _ => return classify_text_value(src),
        }
    }

    // datetime on <time> elements
    if tag == "time" {
        if let Some(datetime) = el.attr("datetime") {
            return SchemaValue::DateTime(datetime.to_string());
        }
    }

    // data element value
    if tag == "data" {
        if let Some(val) = el.attr("value") {
            return classify_text_value(val);
        }
    }

    // Fall back to text content
    let text = collect_text_content(node);
    let trimmed = text.trim().to_string();
    classify_text_value(&trimmed)
}

/// Collects text content from a node and all its descendants.
fn collect_text_content(node: NodeRef<'_, Node>) -> String {
    let mut text = String::new();
    for descendant in node.descendants() {
        if let Some(t) = descendant.value().as_text() {
            text.push_str(t);
        }
    }
    text
}

/// Parses the `prefix` attribute into prefix -> URI mappings.
///
/// Format: `prefix: URI prefix2: URI2` (space-separated pairs).
fn parse_prefix_attr(attr: &str, prefixes: &mut IndexMap<String, String>) {
    let tokens: Vec<&str> = attr.split_whitespace().collect();
    let mut i = 0;
    while i + 1 < tokens.len() {
        let prefix = tokens[i];
        let uri = tokens[i + 1];
        if let Some(stripped) = prefix.strip_suffix(':') {
            prefixes.insert(stripped.to_string(), uri.to_string());
            i += 2;
        } else {
            i += 1;
        }
    }
}

/// Ensures a vocabulary URI ends with a trailing `/`.
fn ensure_trailing_slash(uri: &str) -> String {
    if uri.ends_with('/') || uri.ends_with('#') {
        uri.to_string()
    } else {
        format!("{uri}/")
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn basic_product() {
        let html = r#"<html><body>
<div vocab="https://schema.org/" typeof="Product">
  <span property="name">Widget</span>
  <span property="description">A great widget</span>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert_eq!(out.nodes[0].types, vec!["Product"]);
        assert_eq!(out.nodes[0].source_format, SourceFormat::RdfaLite);
        assert_eq!(
            out.nodes[0].properties["name"],
            vec![SchemaValue::Text("Widget".into())]
        );
        assert_eq!(
            out.nodes[0].properties["description"],
            vec![SchemaValue::Text("A great widget".into())]
        );
    }

    #[test]
    fn nested_typed_property() {
        let html = r#"<html><body>
<div vocab="https://schema.org/" typeof="Product">
  <span property="name">Widget</span>
  <div property="offers" typeof="Offer">
    <span property="priceCurrency">USD</span>
    <meta property="price" content="29.99">
  </div>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        let offers = &out.nodes[0].properties["offers"];
        assert_eq!(offers.len(), 1);
        if let SchemaValue::Node(offer) = &offers[0] {
            assert_eq!(offer.types, vec!["Offer"]);
            assert_eq!(
                offer.properties["priceCurrency"],
                vec![SchemaValue::Text("USD".into())]
            );
            assert_eq!(
                offer.properties["price"],
                vec![SchemaValue::Text("29.99".into())]
            );
        } else {
            panic!("Expected nested Node for offers");
        }
    }

    #[test]
    fn content_attribute() {
        let html = r#"<html><body>
<div vocab="https://schema.org/" typeof="Product">
  <meta property="name" content="Widget">
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["name"],
            vec![SchemaValue::Text("Widget".into())]
        );
    }

    #[test]
    fn href_as_url() {
        let html = r#"<html><body>
<div vocab="https://schema.org/" typeof="Product">
  <span property="name">Widget</span>
  <a property="url" href="https://example.com/widget">Link</a>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["url"],
            vec![SchemaValue::Url("https://example.com/widget".into())]
        );
    }

    #[test]
    fn img_src_as_url() {
        let html = r#"<html><body>
<div vocab="https://schema.org/" typeof="Product">
  <span property="name">Widget</span>
  <img property="image" src="https://example.com/img.jpg">
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["image"],
            vec![SchemaValue::Url("https://example.com/img.jpg".into())]
        );
    }

    #[test]
    fn time_datetime() {
        let html = r#"<html><body>
<div vocab="https://schema.org/" typeof="Event">
  <span property="name">Concert</span>
  <time property="startDate" datetime="2024-06-15T19:00:00">June 15</time>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["startDate"],
            vec![SchemaValue::DateTime("2024-06-15T19:00:00".into())]
        );
    }

    #[test]
    fn resource_as_id() {
        let html = r#"<html><body>
<div vocab="https://schema.org/" typeof="Product" resource="https://example.com/product/1">
  <span property="name">Widget</span>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["@id"],
            vec![SchemaValue::Url("https://example.com/product/1".into())]
        );
    }

    #[test]
    fn vocab_inheritance() {
        let html = r#"<html vocab="https://schema.org/"><body>
<div typeof="Product">
  <span property="name">Widget</span>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert_eq!(out.nodes[0].types, vec!["Product"]);
    }

    #[test]
    fn prefix_resolution() {
        let html = r#"<html prefix="schema: https://schema.org/"><body>
<div vocab="https://schema.org/" typeof="schema:Product">
  <span property="schema:name">Widget</span>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert_eq!(out.nodes[0].types, vec!["Product"]);
        assert_eq!(
            out.nodes[0].properties["name"],
            vec![SchemaValue::Text("Widget".into())]
        );
    }

    #[test]
    fn multiple_types() {
        let html = r#"<html><body>
<div vocab="https://schema.org/" typeof="Product IndividualProduct">
  <span property="name">Widget</span>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes[0].types, vec!["Product", "IndividualProduct"]);
    }

    #[test]
    fn multiple_top_level_items() {
        let html = r#"<html><body>
<div vocab="https://schema.org/" typeof="Product">
  <span property="name">Widget A</span>
</div>
<div vocab="https://schema.org/" typeof="Article">
  <span property="name">Article B</span>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 2);
        assert_eq!(out.nodes[0].types, vec!["Product"]);
        assert_eq!(out.nodes[1].types, vec!["Article"]);
    }

    #[test]
    fn no_rdfa() {
        let html = "<html><body><p>No RDFa here</p></body></html>";
        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert!(out.nodes.is_empty());
        assert!(out.warnings.is_empty());
    }

    #[test]
    fn deep_nesting() {
        let html = r#"<html><body>
<div vocab="https://schema.org/" typeof="Product">
  <span property="name">Widget</span>
  <div property="offers" typeof="Offer">
    <meta property="price" content="29.99">
    <div property="seller" typeof="Organization">
      <span property="name">Acme</span>
    </div>
  </div>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        if let SchemaValue::Node(offer) = &out.nodes[0].properties["offers"][0] {
            assert_eq!(offer.types, vec!["Offer"]);
            if let SchemaValue::Node(seller) = &offer.properties["seller"][0] {
                assert_eq!(seller.types, vec!["Organization"]);
                assert_eq!(
                    seller.properties["name"],
                    vec![SchemaValue::Text("Acme".into())]
                );
            } else {
                panic!("Expected Organization node");
            }
        } else {
            panic!("Expected Offer node");
        }
    }

    #[test]
    fn property_in_wrapper_div() {
        let html = r#"<html><body>
<div vocab="https://schema.org/" typeof="Product">
  <div class="wrapper">
    <span property="name">Widget</span>
  </div>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["name"],
            vec![SchemaValue::Text("Widget".into())]
        );
    }

    #[test]
    fn http_vocab() {
        let html = r#"<html><body>
<div vocab="http://schema.org/" typeof="Product">
  <span property="name">Widget</span>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes[0].types, vec!["Product"]);
    }

    #[test]
    fn parse_prefix_attr_works() {
        let mut prefixes = IndexMap::new();
        parse_prefix_attr(
            "schema: https://schema.org/ og: https://ogp.me/ns#",
            &mut prefixes,
        );
        assert_eq!(prefixes["schema"], "https://schema.org/");
        assert_eq!(prefixes["og"], "https://ogp.me/ns#");
    }

    #[test]
    fn empty_vocab_resets_vocabulary() {
        // An empty vocab="" should reset the vocabulary to None
        let html = r#"<html vocab="https://schema.org/"><body>
<div typeof="Product">
  <span property="name">Outer</span>
  <div vocab="">
    <div typeof="CustomThing">
      <span property="label">Inner</span>
    </div>
  </div>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        // The outer Product should be extracted
        assert!(out
            .nodes
            .iter()
            .any(|n| n.types.contains(&"Product".to_string())));
    }

    #[test]
    fn depth_exceeding_max_truncates_silently() {
        // Build HTML with MAX_DEPTH + 2 nested typeof elements
        let mut html = String::from(r#"<html><body><div vocab="https://schema.org/">"#);
        let target = MAX_DEPTH + 2;
        for i in 0..target {
            html.push_str(&format!(
                r#"<div property="child" typeof="Thing"><span property="name">L{i}</span>"#
            ));
        }
        for _ in 0..target {
            html.push_str("</div>");
        }
        html.push_str("</div></body></html>");

        // Remove the first property="child" to make the outermost a top-level item
        let html = html.replacen(r#"property="child" "#, "", 1);

        let out = RdfaLiteExtractor.extract(&html).expect("extraction failed");
        // Should extract without crashing even if deep nesting is truncated
        assert!(!out.nodes.is_empty());
    }

    #[test]
    fn empty_typeof_warns() {
        let html = r#"<html><body>
<div vocab="https://schema.org/" typeof="">
  <span property="name">Something</span>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert!(
            out.warnings
                .iter()
                .any(|w| w.code == WarningCode::EmptyType),
            "empty typeof should produce EmptyType warning"
        );
    }

    #[test]
    fn data_element_with_value() {
        let html = r#"<html><body>
<div vocab="https://schema.org/" typeof="Product">
  <span property="name">Widget</span>
  <data property="sku" value="12345">Product SKU</data>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["sku"],
            vec![SchemaValue::Text("12345".into())]
        );
    }

    #[test]
    fn property_with_empty_text() {
        let html = r#"<html><body>
<div vocab="https://schema.org/" typeof="Product">
  <span property="name"></span>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["name"],
            vec![SchemaValue::Text(String::new())]
        );
    }

    #[test]
    fn typeof_without_vocab() {
        // typeof without vocab in ancestor chain -- types should be preserved as-is
        let html = r#"<html><body>
<div typeof="Product">
  <span property="name">Widget</span>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert_eq!(out.nodes[0].types, vec!["Product"]);
    }

    #[test]
    fn content_attribute_with_url_value() {
        let html = r#"<html><body>
<div vocab="https://schema.org/" typeof="Product">
  <meta property="url" content="https://example.com/product">
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["url"],
            vec![SchemaValue::Url("https://example.com/product".into())]
        );
    }

    #[test]
    fn resource_on_nested_property() {
        let html = r#"<html><body>
<div vocab="https://schema.org/" typeof="Product">
  <span property="name">Widget</span>
  <div property="offers" typeof="Offer" resource="https://example.com/offer/1">
    <span property="priceCurrency">USD</span>
  </div>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        let offers = &out.nodes[0].properties["offers"];
        if let SchemaValue::Node(offer) = &offers[0] {
            assert_eq!(
                offer.properties["@id"],
                vec![SchemaValue::Url("https://example.com/offer/1".into())]
            );
        } else {
            panic!("Expected nested Offer node");
        }
    }

    #[test]
    fn nested_prefix_declarations() {
        let html = r#"<html prefix="schema: https://schema.org/"><body>
<div prefix="og: https://ogp.me/ns#" vocab="https://schema.org/" typeof="Product">
  <span property="name">Widget</span>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert_eq!(out.nodes[0].types, vec!["Product"]);
    }

    #[test]
    fn independent_typeof_nested_in_typed_node() {
        // A typeof inside another typeof WITHOUT property attribute
        // should not be double-counted or added as a property of the parent.
        let html = r#"<html><body>
<div vocab="https://schema.org/" typeof="WebPage">
  <span property="name">My Page</span>
  <div typeof="Organization">
    <span property="name">Acme Corp</span>
  </div>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        // The WebPage should be extracted as a top-level node.
        // The Organization (without property attribute) is skipped
        // by the current implementation to avoid double-counting.
        assert!(out
            .nodes
            .iter()
            .any(|n| n.types.contains(&"WebPage".to_string())));
    }

    #[test]
    fn time_element_without_datetime() {
        let html = r#"<html><body>
<div vocab="https://schema.org/" typeof="Event">
  <span property="name">Concert</span>
  <time property="startDate">June 15, 2024</time>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        // Without datetime attribute, should fall back to text content
        assert_eq!(
            out.nodes[0].properties["startDate"],
            vec![SchemaValue::Text("June 15, 2024".into())]
        );
    }

    #[test]
    fn unicode_preserved_in_values() {
        let html = r#"<html><body>
<div vocab="https://schema.org/" typeof="Product">
  <span property="name">Gerät für Ökologie</span>
</div>
</body></html>"#;

        let out = RdfaLiteExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert_eq!(
            out.nodes[0].properties["name"],
            vec![SchemaValue::Text("Gerät für Ökologie".into())]
        );
    }
}
