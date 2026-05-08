#![cfg(feature = "extraction")]
//! Cross-format integration tests.
//!
//! Verifies that the same structured data expressed in JSON-LD, Microdata, and
//! RDFa Lite produces equivalent `StructuredDataGraph` output.

use pretty_assertions::assert_eq;
use schemaorg_rs::types::{SchemaValue, SourceFormat};
use schemaorg_rs::{
    extract_all, Extractor, JsonLdExtractor, MicrodataExtractor, RdfaLiteExtractor,
};

/// Helper: extract nodes using a specific extractor and return the first node.
fn extract_first(extractor: &dyn Extractor, html: &str) -> schemaorg_rs::types::SchemaNode {
    let out = extractor.extract(html).expect("extraction failed");
    assert!(!out.nodes.is_empty(), "expected at least one node");
    out.nodes.into_iter().next().unwrap()
}

// Simple Product: same data in all three formats
const PRODUCT_JSONLD: &str = r#"<html><head>
<script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "Product",
  "name": "Widget",
  "description": "A great widget",
  "url": "https://example.com/widget"
}</script>
</head></html>"#;

const PRODUCT_MICRODATA: &str = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Widget</span>
  <span itemprop="description">A great widget</span>
  <a itemprop="url" href="https://example.com/widget">Widget</a>
</div>
</body></html>"#;

const PRODUCT_RDFA: &str = r#"<html><body>
<div vocab="https://schema.org/" typeof="Product">
  <span property="name">Widget</span>
  <span property="description">A great widget</span>
  <a property="url" href="https://example.com/widget">Widget</a>
</div>
</body></html>"#;

#[test]
fn cross_format_product_types() {
    let jld = extract_first(&JsonLdExtractor, PRODUCT_JSONLD);
    let md = extract_first(&MicrodataExtractor, PRODUCT_MICRODATA);
    let rdfa = extract_first(&RdfaLiteExtractor, PRODUCT_RDFA);

    assert_eq!(jld.types, vec!["Product"]);
    assert_eq!(md.types, vec!["Product"]);
    assert_eq!(rdfa.types, vec!["Product"]);
}

#[test]
fn cross_format_product_name() {
    let jld = extract_first(&JsonLdExtractor, PRODUCT_JSONLD);
    let md = extract_first(&MicrodataExtractor, PRODUCT_MICRODATA);
    let rdfa = extract_first(&RdfaLiteExtractor, PRODUCT_RDFA);

    let expected = vec![SchemaValue::Text("Widget".into())];
    assert_eq!(jld.properties["name"], expected);
    assert_eq!(md.properties["name"], expected);
    assert_eq!(rdfa.properties["name"], expected);
}

#[test]
fn cross_format_product_url() {
    let jld = extract_first(&JsonLdExtractor, PRODUCT_JSONLD);
    let md = extract_first(&MicrodataExtractor, PRODUCT_MICRODATA);
    let rdfa = extract_first(&RdfaLiteExtractor, PRODUCT_RDFA);

    let expected = vec![SchemaValue::Url("https://example.com/widget".into())];
    assert_eq!(jld.properties["url"], expected);
    assert_eq!(md.properties["url"], expected);
    assert_eq!(rdfa.properties["url"], expected);
}

#[test]
fn cross_format_source_format_differs() {
    let jld = extract_first(&JsonLdExtractor, PRODUCT_JSONLD);
    let md = extract_first(&MicrodataExtractor, PRODUCT_MICRODATA);
    let rdfa = extract_first(&RdfaLiteExtractor, PRODUCT_RDFA);

    assert_eq!(jld.source_format, SourceFormat::JsonLd);
    assert_eq!(md.source_format, SourceFormat::Microdata);
    assert_eq!(rdfa.source_format, SourceFormat::RdfaLite);
}

// Nested Product with Offer
const NESTED_JSONLD: &str = r#"<html><head>
<script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "Product",
  "name": "Widget",
  "offers": {
    "@type": "Offer",
    "priceCurrency": "USD"
  }
}</script>
</head></html>"#;

const NESTED_MICRODATA: &str = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Widget</span>
  <div itemprop="offers" itemscope itemtype="https://schema.org/Offer">
    <span itemprop="priceCurrency">USD</span>
  </div>
</div>
</body></html>"#;

const NESTED_RDFA: &str = r#"<html><body>
<div vocab="https://schema.org/" typeof="Product">
  <span property="name">Widget</span>
  <div property="offers" typeof="Offer">
    <span property="priceCurrency">USD</span>
  </div>
</div>
</body></html>"#;

#[test]
fn cross_format_nested_types() {
    let jld = extract_first(&JsonLdExtractor, NESTED_JSONLD);
    let md = extract_first(&MicrodataExtractor, NESTED_MICRODATA);
    let rdfa = extract_first(&RdfaLiteExtractor, NESTED_RDFA);

    assert_eq!(jld.types, vec!["Product"]);
    assert_eq!(md.types, vec!["Product"]);
    assert_eq!(rdfa.types, vec!["Product"]);

    for node in &[&jld, &md, &rdfa] {
        let offers = &node.properties["offers"];
        assert_eq!(offers.len(), 1, "expected 1 offer");
        if let SchemaValue::Node(offer) = &offers[0] {
            assert_eq!(offer.types, vec!["Offer"]);
            assert_eq!(
                offer.properties["priceCurrency"],
                vec![SchemaValue::Text("USD".into())]
            );
        } else {
            panic!(
                "Expected nested Node for offers in {:?}",
                node.source_format
            );
        }
    }
}

// Event with DateTime
const EVENT_JSONLD: &str = r#"<html><head>
<script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "Event",
  "name": "Concert"
}</script>
</head></html>"#;

const EVENT_MICRODATA: &str = r#"<html><body>
<div itemscope itemtype="https://schema.org/Event">
  <span itemprop="name">Concert</span>
</div>
</body></html>"#;

const EVENT_RDFA: &str = r#"<html><body>
<div vocab="https://schema.org/" typeof="Event">
  <span property="name">Concert</span>
</div>
</body></html>"#;

#[test]
fn cross_format_event_name() {
    let jld = extract_first(&JsonLdExtractor, EVENT_JSONLD);
    let md = extract_first(&MicrodataExtractor, EVENT_MICRODATA);
    let rdfa = extract_first(&RdfaLiteExtractor, EVENT_RDFA);

    let expected = vec![SchemaValue::Text("Concert".into())];
    assert_eq!(jld.properties["name"], expected);
    assert_eq!(md.properties["name"], expected);
    assert_eq!(rdfa.properties["name"], expected);
}

// extract_all with multiple formats in one document
#[test]
fn extract_all_combined_document() {
    let html = r#"<html><head>
<script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "Organization",
  "name": "Acme"
}</script>
</head><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Widget</span>
</div>
<div vocab="https://schema.org/" typeof="Article">
  <span property="name">News</span>
</div>
</body></html>"#;

    let graph = extract_all(html).expect("extraction failed");
    assert_eq!(graph.nodes.len(), 3);

    let formats: Vec<SourceFormat> = graph.nodes.iter().map(|n| n.source_format).collect();
    assert!(formats.contains(&SourceFormat::JsonLd));
    assert!(formats.contains(&SourceFormat::Microdata));
    assert!(formats.contains(&SourceFormat::RdfaLite));

    let types: Vec<&str> = graph.nodes.iter().map(|n| n.types[0].as_str()).collect();
    assert!(types.contains(&"Organization"));
    assert!(types.contains(&"Product"));
    assert!(types.contains(&"Article"));
}

#[test]
fn extract_all_no_structured_data() {
    let html = "<html><body><p>Nothing here</p></body></html>";
    let graph = extract_all(html).expect("extraction failed");
    assert!(graph.nodes.is_empty());
}

// Malformed and edge-case inputs
#[test]
fn malformed_html_still_extracts() {
    // Unclosed tags, missing attributes - html5ever handles this
    let html = r#"<html><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">Widget
  <img itemprop="image" src="https://example.com/img.jpg">
</div>
<div vocab="https://schema.org/" typeof="Article">
  <span property="headline">News
</div>
</body></html>"#;

    let graph = extract_all(html).expect("extraction failed");
    assert!(
        graph.nodes.len() >= 2,
        "expected at least 2 nodes from malformed HTML"
    );
}

#[test]
fn empty_document() {
    let graph = extract_all("").expect("extraction failed");
    assert!(graph.nodes.is_empty());
}

#[test]
fn multiple_formats_same_type() {
    // Same type in JSON-LD and Microdata - both should be extracted
    let html = r#"<html><head>
<script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "Product",
  "name": "JSON Product"
}</script>
</head><body>
<div itemscope itemtype="https://schema.org/Product">
  <span itemprop="name">HTML Product</span>
</div>
</body></html>"#;

    let graph = extract_all(html).expect("extraction failed");
    assert_eq!(graph.nodes.len(), 2);

    let names: Vec<&SchemaValue> = graph
        .nodes
        .iter()
        .filter_map(|n| n.properties.get("name")?.first())
        .collect();
    assert_eq!(names.len(), 2);
}

#[test]
fn thread_safety_send_sync() {
    // Verify that extractors implement Send + Sync so they can be
    // used from multiple threads (e.g. in a web server context).
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<JsonLdExtractor>();
    assert_send_sync::<MicrodataExtractor>();
    assert_send_sync::<RdfaLiteExtractor>();
}

#[test]
fn schema_value_display() {
    use schemaorg_rs::types::{SchemaNode, SchemaValue, SourceFormat};

    assert_eq!(SchemaValue::Text("hello".into()).to_string(), "hello");
    assert_eq!(
        SchemaValue::Url("https://example.com".into()).to_string(),
        "https://example.com"
    );
    assert_eq!(SchemaValue::Boolean(true).to_string(), "true");
    assert_eq!(SchemaValue::Number(42.5).to_string(), "42.5");
    assert_eq!(
        SchemaValue::DateTime("2024-01-15".into()).to_string(),
        "2024-01-15"
    );

    let node = SchemaNode {
        types: vec!["Product".into()],
        properties: indexmap::IndexMap::new(),
        source_format: SourceFormat::JsonLd,
        source_location: None,
    };
    assert_eq!(
        SchemaValue::Node(Box::new(node)).to_string(),
        "[Product node]"
    );
}

#[test]
fn source_format_display() {
    assert_eq!(SourceFormat::JsonLd.to_string(), "JSON-LD");
    assert_eq!(SourceFormat::Microdata.to_string(), "Microdata");
    assert_eq!(SourceFormat::RdfaLite.to_string(), "RDFa Lite");
}

#[test]
fn warning_code_display_all_variants() {
    use schemaorg_rs::error::WarningCode;

    assert_eq!(
        WarningCode::MalformedJsonLd.to_string(),
        "malformed-json-ld"
    );
    assert_eq!(
        WarningCode::MalformedMicrodata.to_string(),
        "malformed-microdata"
    );
    assert_eq!(WarningCode::MalformedRdfa.to_string(), "malformed-rdfa");
    assert_eq!(
        WarningCode::UnresolvableReference.to_string(),
        "unresolvable-reference"
    );
    assert_eq!(WarningCode::EmptyType.to_string(), "empty-type");
    assert_eq!(WarningCode::DuplicateId.to_string(), "duplicate-id");
    assert_eq!(WarningCode::ExtractorFailed.to_string(), "extractor-failed");
}

#[test]
fn extraction_error_display() {
    use schemaorg_rs::error::ExtractionError;

    let err = ExtractionError::Internal("test failure".into());
    assert_eq!(err.to_string(), "extraction failed: test failure");
}

#[test]
fn unicode_emoji_in_json_ld() {
    let html = r#"<html><head><script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "Product",
  "name": "Super Widget \u2728\uD83D\uDE80"
}</script></head></html>"#;

    let graph = extract_all(html).expect("extraction failed");
    assert_eq!(graph.nodes.len(), 1);
    // The name should contain unicode characters
    let name = &graph.nodes[0].properties["name"][0];
    if let SchemaValue::Text(s) = name {
        assert!(s.contains("Super Widget"));
    } else {
        panic!("Expected Text value");
    }
}
