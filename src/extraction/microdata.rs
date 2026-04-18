//! Microdata extractor: parses `itemscope`/`itemprop` attributes.
//!
//! Implements the [W3C Microdata to RDF](https://www.w3.org/TR/microdata-rdf/)
//! extraction algorithm using `scraper` for DOM traversal.
//!
//! ## Supported features
//!
//! - `itemscope` / `itemtype` for defining nodes
//! - `itemprop` for defining properties
//! - Nested item scopes (property that is itself an item)
//! - `itemref` for non-contiguous DOM references
//! - `itemid` for global identifiers
//! - Space-separated `itemprop` values (one element -> multiple properties)
//! - Value extraction by element type (`<meta>`, `<a>`, `<img>`, `<time>`, etc.)
//!
//! ## Depth limit
//!
//! Nested scopes are limited to 20 levels to prevent stack overflow on
//! malformed markup with circular or excessively deep nesting.

use std::sync::OnceLock;

use ego_tree::NodeRef;
use indexmap::IndexMap;
use scraper::node::Node;
use scraper::{Html, Selector};

use crate::error::{ExtractionError, ExtractionWarning, WarningCode};
use crate::types::{SchemaNode, SchemaValue, SourceFormat};

use super::{classify_text_value, strip_schema_prefix, ExtractionOutput, Extractor};

/// Maximum nesting depth for Microdata scopes.
const MAX_DEPTH: usize = 20;

/// Extracts Schema.org structured data from HTML Microdata attributes.
///
/// # Examples
///
/// ```
/// use schemaorg_rs::extraction::{Extractor, MicrodataExtractor};
///
/// let html = r#"<html><body>
/// <div itemscope itemtype="https://schema.org/Product">
///   <span itemprop="name">Widget</span>
/// </div>
/// </body></html>"#;
///
/// let output = MicrodataExtractor.extract(html).unwrap();
/// assert_eq!(output.nodes[0].types, vec!["Product"]);
/// ```
pub struct MicrodataExtractor;

impl Extractor for MicrodataExtractor {
    fn extract(&self, html: &str) -> Result<ExtractionOutput, ExtractionError> {
        let document = Html::parse_document(html);
        self.extract_from_document(&document)
    }
}

impl MicrodataExtractor {
    /// Extracts from an already-parsed document.
    ///
    /// # Errors
    ///
    /// Returns [`ExtractionError`] if a fatal error prevents extraction.
    /// Most issues are captured as warnings in the returned output.
    ///
    /// # Panics
    ///
    /// Panics if the internal CSS selector constant fails to parse.
    /// This is a compile-time-verified string and will never fail.
    pub fn extract_from_document(
        &self,
        document: &Html,
    ) -> Result<ExtractionOutput, ExtractionError> {
        // Find top-level items: elements with itemscope but NOT itemprop
        // (itemprop + itemscope = nested item, handled during parent traversal)
        static SELECTOR: OnceLock<Selector> = OnceLock::new();
        let selector = SELECTOR.get_or_init(|| {
            Selector::parse("[itemscope]").expect("static selector '[itemscope]' must parse")
        });

        let mut warnings = Vec::new();
        let mut nodes = Vec::new();

        for element in document.select(selector) {
            // Skip nested items: those with itemprop are handled by their parent
            if element.value().attr("itemprop").is_some() {
                continue;
            }

            match extract_item(&element, document, &mut warnings, 0) {
                Some(node) => nodes.push(node),
                None => {
                    warnings.push(ExtractionWarning {
                        message: "failed to extract Microdata item".into(),
                        source_location: None,
                        code: WarningCode::MalformedMicrodata,
                    });
                }
            }
        }

        Ok(ExtractionOutput { nodes, warnings })
    }
}

/// Extract a single Microdata item from an element with `itemscope`.
fn extract_item(
    element: &scraper::ElementRef<'_>,
    document: &Html,
    warnings: &mut Vec<ExtractionWarning>,
    depth: usize,
) -> Option<SchemaNode> {
    if depth > MAX_DEPTH {
        warnings.push(ExtractionWarning {
            message: format!("Microdata nesting depth exceeds {MAX_DEPTH}, skipping"),
            source_location: None,
            code: WarningCode::MalformedMicrodata,
        });
        return None;
    }

    let el = element.value();

    // Extract itemtype -> types
    let types = extract_itemtypes(el);

    // Build properties from itemprop descendants + itemref targets
    let mut properties: IndexMap<String, Vec<SchemaValue>> = IndexMap::new();

    // Store itemid as @id
    if let Some(item_id) = el.attr("itemid") {
        properties
            .entry("@id".into())
            .or_default()
            .push(classify_text_value(item_id));
    }

    // Collect itemprop elements from the subtree
    collect_properties(element, document, warnings, &mut properties, depth);

    // Handle itemref: collect properties from referenced elements
    if let Some(refs) = el.attr("itemref") {
        for ref_id in refs.split_whitespace() {
            // Use direct DOM traversal instead of CSS selectors to avoid
            // selector injection with special characters in IDs.
            match find_element_by_id(document, ref_id) {
                Some(ref_element) => {
                    if ref_element.value().attr("itemprop").is_some() {
                        extract_prop_value(
                            &ref_element,
                            document,
                            warnings,
                            &mut properties,
                            depth,
                        );
                    } else {
                        collect_properties(
                            &ref_element,
                            document,
                            warnings,
                            &mut properties,
                            depth,
                        );
                    }
                }
                None => {
                    warnings.push(ExtractionWarning {
                        message: format!("itemref target not found: #{ref_id}"),
                        source_location: None,
                        code: WarningCode::UnresolvableReference,
                    });
                }
            }
        }
    }

    if types.is_empty() && properties.is_empty() {
        return None;
    }

    if types.is_empty() {
        warnings.push(ExtractionWarning {
            message: "Microdata item has itemscope but no itemtype".into(),
            source_location: None,
            code: WarningCode::EmptyType,
        });
    }

    Some(SchemaNode {
        types,
        properties,
        source_format: SourceFormat::Microdata,
        source_location: None,
    })
}

/// Collects `itemprop` elements from the subtree of a given element.
///
/// Walks the immediate children; for each child with `itemprop`, extracts
/// the property. For children without `itemprop` that are NOT a new
/// `itemscope`, recurse into their subtree to find deeper `itemprop` elements.
fn collect_properties(
    element: &scraper::ElementRef<'_>,
    document: &Html,
    warnings: &mut Vec<ExtractionWarning>,
    properties: &mut IndexMap<String, Vec<SchemaValue>>,
    depth: usize,
) {
    for child in element.children() {
        visit_for_properties(child, document, warnings, properties, depth);
    }
}

/// Recursively visits a node looking for `itemprop` elements.
fn visit_for_properties(
    node: NodeRef<'_, Node>,
    document: &Html,
    warnings: &mut Vec<ExtractionWarning>,
    properties: &mut IndexMap<String, Vec<SchemaValue>>,
    depth: usize,
) {
    if let Some(el) = node.value().as_element() {
        let Some(elem_ref) = scraper::ElementRef::wrap(node) else {
            return;
        };

        if el.attr("itemprop").is_some() {
            // This is a property - extract its value
            extract_prop_value(&elem_ref, document, warnings, properties, depth);
            return; // Don't recurse further - the property owns its subtree
        }

        // If this is a new itemscope WITHOUT itemprop, it's a separate top-level item.
        // Don't traverse into it - it's handled by the top-level loop.
        if el.attr("itemscope").is_some() {
            return;
        }
    }

    // Not an itemprop, not a new itemscope - recurse into children
    for child in node.children() {
        visit_for_properties(child, document, warnings, properties, depth);
    }
}

/// Extracts one or more property values from an `itemprop` element.
///
/// Handles space-separated `itemprop` names (one element -> multiple properties).
fn extract_prop_value(
    element: &scraper::ElementRef<'_>,
    document: &Html,
    warnings: &mut Vec<ExtractionWarning>,
    properties: &mut IndexMap<String, Vec<SchemaValue>>,
    depth: usize,
) {
    let el = element.value();
    let prop_names: Vec<&str> = el
        .attr("itemprop")
        .unwrap_or("")
        .split_whitespace()
        .collect();

    if prop_names.is_empty() {
        return;
    }

    let value = extract_element_value(element, document, warnings, depth);

    for name in prop_names {
        properties
            .entry(name.to_string())
            .or_default()
            .push(value.clone());
    }
}

/// Extracts the value from an element based on its tag name.
///
/// Follows the W3C Microdata extraction rules:
/// - `<meta>` -> `content` attribute
/// - `<a>`, `<link>`, `<area>` -> `href` attribute (URL)
/// - `<img>`, `<audio>`, `<video>`, `<source>` -> `src` attribute (URL)
/// - `<time>` -> `datetime` attribute
/// - `<data>` -> `value` attribute
/// - `<meter>` -> `value` attribute (Number)
/// - Element with `itemscope` -> nested node
/// - Everything else -> text content
fn extract_element_value(
    element: &scraper::ElementRef<'_>,
    document: &Html,
    warnings: &mut Vec<ExtractionWarning>,
    depth: usize,
) -> SchemaValue {
    let el = element.value();
    let tag = el.name();

    // Nested item scope
    if el.attr("itemscope").is_some() {
        return match extract_item(element, document, warnings, depth + 1) {
            Some(node) => SchemaValue::Node(Box::new(node)),
            None => SchemaValue::Text(String::new()),
        };
    }

    match tag {
        "meta" => {
            let content = el.attr("content").unwrap_or("");
            classify_text_value(content)
        }
        "a" | "link" | "area" => {
            let href = el.attr("href").unwrap_or("");
            if href.is_empty() {
                SchemaValue::Text(element.text().collect::<String>().trim().to_string())
            } else {
                SchemaValue::Url(href.to_string())
            }
        }
        "img" | "audio" | "video" | "source" | "embed" => {
            let src = el.attr("src").unwrap_or("");
            if src.is_empty() {
                SchemaValue::Text(String::new())
            } else {
                SchemaValue::Url(src.to_string())
            }
        }
        "object" => {
            let data = el.attr("data").unwrap_or("");
            if data.is_empty() {
                SchemaValue::Text(String::new())
            } else {
                SchemaValue::Url(data.to_string())
            }
        }
        "time" => {
            let datetime = el.attr("datetime").unwrap_or("");
            if datetime.is_empty() {
                SchemaValue::Text(element.text().collect::<String>().trim().to_string())
            } else {
                SchemaValue::DateTime(datetime.to_string())
            }
        }
        "data" => {
            let val = el.attr("value").unwrap_or("");
            if val.is_empty() {
                SchemaValue::Text(element.text().collect::<String>().trim().to_string())
            } else {
                classify_text_value(val)
            }
        }
        "meter" => {
            let val = el.attr("value").unwrap_or("");
            match val.parse::<f64>() {
                Ok(n) => SchemaValue::Number(n),
                Err(_) => SchemaValue::Text(val.to_string()),
            }
        }
        _ => {
            let text = element.text().collect::<String>();
            let trimmed = text.trim().to_string();
            classify_text_value(&trimmed)
        }
    }
}

/// Extracts `itemtype` values, stripping `schema.org` prefixes.
fn extract_itemtypes(el: &scraper::node::Element) -> Vec<String> {
    el.attr("itemtype")
        .map(|types| {
            types
                .split_whitespace()
                .map(|s| strip_schema_prefix(s).into_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// Finds an element by its `id` attribute using direct DOM traversal.
///
/// Uses attribute comparison instead of CSS selectors to correctly handle
/// IDs containing special characters (dots, colons, brackets).
fn find_element_by_id<'a>(document: &'a Html, id: &str) -> Option<scraper::ElementRef<'a>> {
    document
        .tree
        .root()
        .descendants()
        .filter_map(scraper::ElementRef::wrap)
        .find(|el| el.value().id() == Some(id))
}

/////////////////////////////////////////////////////////////////////////////
// Unit tests
/////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn basic_product() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Widget</span>
  <span itemprop="description">A great widget</span>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert_eq!(out.nodes[0].types, vec!["Product"]);
        assert_eq!(out.nodes[0].source_format, SourceFormat::Microdata);
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
    fn nested_offer() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Widget</span>
  <div itemprop="offers" itemscope itemtype="https://schema.org/Offer">
    <span itemprop="priceCurrency">USD</span>
    <meta itemprop="price" content="29.99">
  </div>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
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
    fn meta_content() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <meta itemprop="name" content="Invisible Widget">
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["name"],
            vec![SchemaValue::Text("Invisible Widget".into())]
        );
    }

    #[test]
    fn link_href_as_url() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Widget</span>
  <a itemprop="url" href="https://example.com/widget">Link</a>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["url"],
            vec![SchemaValue::Url("https://example.com/widget".into())]
        );
    }

    #[test]
    fn img_src_as_url() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Widget</span>
  <img itemprop="image" src="https://example.com/img.jpg">
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["image"],
            vec![SchemaValue::Url("https://example.com/img.jpg".into())]
        );
    }

    #[test]
    fn time_datetime() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Event">
  <span itemprop="name">Concert</span>
  <time itemprop="startDate" datetime="2024-06-15T19:00:00">June 15</time>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["startDate"],
            vec![SchemaValue::DateTime("2024-06-15T19:00:00".into())]
        );
    }

    #[test]
    fn meter_value_as_number() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Widget</span>
  <meter itemprop="ratingValue" value="4.5" min="0" max="5">4.5 stars</meter>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["ratingValue"],
            vec![SchemaValue::Number(4.5)]
        );
    }

    #[test]
    fn data_value_attribute() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <data itemprop="sku" value="12345">Product SKU</data>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["sku"],
            vec![SchemaValue::Text("12345".into())]
        );
    }

    #[test]
    fn space_separated_itemprop() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name alternateName">Widget</span>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["name"],
            vec![SchemaValue::Text("Widget".into())]
        );
        assert_eq!(
            out.nodes[0].properties["alternateName"],
            vec![SchemaValue::Text("Widget".into())]
        );
    }

    #[test]
    fn multiple_values_same_property() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Widget</span>
  <img itemprop="image" src="https://example.com/img1.jpg">
  <img itemprop="image" src="https://example.com/img2.jpg">
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes[0].properties["image"].len(), 2);
    }

    #[test]
    fn itemid_becomes_at_id() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product" itemid="https://example.com/product/123">
  <span itemprop="name">Widget</span>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["@id"],
            vec![SchemaValue::Url("https://example.com/product/123".into())]
        );
    }

    #[test]
    fn itemref_collects_external_properties() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product" itemref="desc-block">
  <span itemprop="name">Widget</span>
</div>
<div id="desc-block">
  <span itemprop="description">A fine product</span>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert_eq!(
            out.nodes[0].properties["description"],
            vec![SchemaValue::Text("A fine product".into())]
        );
    }

    #[test]
    fn itemref_missing_target_warns() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product" itemref="nonexistent">
  <span itemprop="name">Widget</span>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert!(out
            .warnings
            .iter()
            .any(|w| w.code == WarningCode::UnresolvableReference));
    }

    #[test]
    fn multiple_itemtypes() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product https://schema.org/IndividualProduct">
  <span itemprop="name">Widget</span>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes[0].types, vec!["Product", "IndividualProduct"]);
    }

    #[test]
    fn http_prefix_stripped() {
        let html = r#"<html><body>
<div itemscope itemtype="http://schema.org/Product">
  <span itemprop="name">Widget</span>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes[0].types, vec!["Product"]);
    }

    #[test]
    fn deeply_nested_scopes() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Widget</span>
  <div itemprop="offers" itemscope itemtype="https://schema.org/Offer">
    <meta itemprop="price" content="29.99">
    <div itemprop="seller" itemscope itemtype="https://schema.org/Organization">
      <span itemprop="name">Acme</span>
      <div itemprop="address" itemscope itemtype="https://schema.org/PostalAddress">
        <span itemprop="addressCountry">US</span>
      </div>
    </div>
  </div>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        if let SchemaValue::Node(offer) = &out.nodes[0].properties["offers"][0] {
            if let SchemaValue::Node(seller) = &offer.properties["seller"][0] {
                if let SchemaValue::Node(addr) = &seller.properties["address"][0] {
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
    fn multiple_top_level_items() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Widget A</span>
</div>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Widget B</span>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 2);
        assert_eq!(
            out.nodes[0].properties["name"],
            vec![SchemaValue::Text("Widget A".into())]
        );
        assert_eq!(
            out.nodes[1].properties["name"],
            vec![SchemaValue::Text("Widget B".into())]
        );
    }

    #[test]
    fn no_microdata() {
        let html = "<html><body><p>No microdata here</p></body></html>";
        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert!(out.nodes.is_empty());
        assert!(out.warnings.is_empty());
    }

    #[test]
    fn itemscope_without_itemtype_warns() {
        let html = r#"<html><body>
<div itemscope>
  <span itemprop="name">Something</span>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert!(out.nodes[0].types.is_empty());
        assert!(out
            .warnings
            .iter()
            .any(|w| w.code == WarningCode::EmptyType));
    }

    #[test]
    fn itemprop_in_wrapper_div() {
        // itemprop elements inside non-itemscope wrapper divs
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <div class="wrapper">
    <div class="inner">
      <span itemprop="name">Widget</span>
    </div>
  </div>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert_eq!(
            out.nodes[0].properties["name"],
            vec![SchemaValue::Text("Widget".into())]
        );
    }

    #[test]
    fn time_without_datetime_uses_text() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Event">
  <time itemprop="startDate">June 15, 2024</time>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["startDate"],
            vec![SchemaValue::Text("June 15, 2024".into())]
        );
    }

    #[test]
    fn link_without_href_uses_text() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <a itemprop="url">Click here</a>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["url"],
            vec![SchemaValue::Text("Click here".into())]
        );
    }

    #[test]
    fn circular_itemref_does_not_loop() {
        let html = r#"<html><body>
<div id="a" itemscope itemtype="https://schema.org/Product" itemref="b">
  <span itemprop="name">Product A</span>
</div>
<div id="b">
  <span itemprop="description">Desc from B</span>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("must not hang");
        assert_eq!(out.nodes.len(), 1);
        assert_eq!(
            out.nodes[0].properties["description"],
            vec![SchemaValue::Text("Desc from B".into())]
        );
    }

    #[test]
    fn self_referencing_itemref() {
        // An item references its own id
        let html = r#"<html><body>
<div id="self" itemscope itemtype="https://schema.org/Product" itemref="self">
  <span itemprop="name">Widget</span>
</div>
</body></html>"#;

        // Should not infinite-loop. The element itself already has its
        // properties collected, so re-visiting should not duplicate.
        let out = MicrodataExtractor.extract(html).expect("must not hang");
        assert_eq!(out.nodes.len(), 1);
    }

    #[test]
    fn itemref_multiple_ids() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product" itemref="desc-block price-block">
  <span itemprop="name">Widget</span>
</div>
<div id="desc-block">
  <span itemprop="description">A fine widget</span>
</div>
<div id="price-block">
  <meta itemprop="price" content="29.99">
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert_eq!(
            out.nodes[0].properties["description"],
            vec![SchemaValue::Text("A fine widget".into())]
        );
        assert_eq!(
            out.nodes[0].properties["price"],
            vec![SchemaValue::Text("29.99".into())]
        );
    }

    #[test]
    fn empty_itemprop_attribute_skipped() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="">should be skipped</span>
  <span itemprop="name">Widget</span>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        // Empty itemprop should not create a property with empty-string key
        assert!(!out.nodes[0].properties.contains_key(""));
        assert_eq!(
            out.nodes[0].properties["name"],
            vec![SchemaValue::Text("Widget".into())]
        );
    }

    #[test]
    fn object_element_data_attribute() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Widget</span>
  <object itemprop="image" data="https://example.com/widget.swf">fallback</object>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["image"],
            vec![SchemaValue::Url("https://example.com/widget.swf".into())]
        );
    }

    #[test]
    fn embed_element_src_attribute() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Widget</span>
  <embed itemprop="video" src="https://example.com/demo.mp4">
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["video"],
            vec![SchemaValue::Url("https://example.com/demo.mp4".into())]
        );
    }

    #[test]
    fn source_element_src_attribute() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Widget</span>
  <source itemprop="audio" src="https://example.com/sound.mp3">
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["audio"],
            vec![SchemaValue::Url("https://example.com/sound.mp3".into())]
        );
    }

    #[test]
    fn depth_exceeding_max_warns() {
        // Build HTML with MAX_DEPTH + 2 nested itemscopes
        let mut html = String::from("<html><body>");
        let target = MAX_DEPTH + 2;
        for i in 0..target {
            html.push_str(&format!(
                r#"<div itemprop="child" itemscope itemtype="https://schema.org/Thing"><span itemprop="name">L{i}</span>"#
            ));
        }
        for _ in 0..target {
            html.push_str("</div>");
        }
        html.push_str("</body></html>");

        // Remove itemprop from the outermost to make it a top-level item
        let html = html.replacen(r#"itemprop="child" "#, "", 1);

        let out = MicrodataExtractor
            .extract(&html)
            .expect("extraction failed");
        assert!(
            out.warnings
                .iter()
                .any(|w| w.message.contains("depth") || w.message.contains("Microdata")),
            "should warn when exceeding MAX_DEPTH"
        );
    }

    #[test]
    fn empty_itemtype_attribute() {
        let html = r#"<html><body>
<div itemscope itemtype="">
  <span itemprop="name">Something</span>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert!(out.nodes[0].types.is_empty());
        assert!(out
            .warnings
            .iter()
            .any(|w| w.code == WarningCode::EmptyType));
    }

    #[test]
    fn meter_non_numeric_value_fallback() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Widget</span>
  <meter itemprop="score" value="not-a-number">High</meter>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["score"],
            vec![SchemaValue::Text("not-a-number".into())]
        );
    }

    #[test]
    fn img_empty_src_gives_empty_text() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Widget</span>
  <img itemprop="image" src="">
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["image"],
            vec![SchemaValue::Text(String::new())]
        );
    }

    #[test]
    fn itemref_to_element_with_itemprop() {
        // The referenced element itself has itemprop, so it should be
        // extracted as a property directly
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product" itemref="ext-name">
  <span itemprop="description">A fine widget</span>
</div>
<span id="ext-name" itemprop="name">Widget</span>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert_eq!(
            out.nodes[0].properties["name"],
            vec![SchemaValue::Text("Widget".into())]
        );
    }

    #[test]
    fn unicode_preserved_in_values() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Gerät für Ökologie</span>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(out.nodes.len(), 1);
        assert_eq!(
            out.nodes[0].properties["name"],
            vec![SchemaValue::Text("Gerät für Ökologie".into())]
        );
    }

    #[test]
    fn object_empty_data_gives_empty_text() {
        let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Widget</span>
  <object itemprop="image" data="">fallback</object>
</div>
</body></html>"#;

        let out = MicrodataExtractor.extract(html).expect("extraction failed");
        assert_eq!(
            out.nodes[0].properties["image"],
            vec![SchemaValue::Text(String::new())]
        );
    }
}
