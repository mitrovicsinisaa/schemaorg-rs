//! Integration tests: HTML -> extract_all() -> validate() -> assert diagnostics.

#[cfg(feature = "validation")]
mod validation_integration {
    use schemaorg_rs::extract_all;
    use schemaorg_rs::validation::{self, DiagnosticCode};

    // Valid documents (no errors)
    #[test]
    fn valid_product_no_errors() {
        let html = r#"<script type="application/ld+json">{
            "@context": "https://schema.org",
            "@type": "Product",
            "name": "Test Widget",
            "description": "A test product",
            "offers": {
                "@type": "Offer",
                "price": "10.00",
                "priceCurrency": "EUR"
            }
        }</script>"#;
        let graph = extract_all(html).unwrap();
        let result = validation::validate(&graph);
        assert!(
            !result.has_errors(),
            "Valid product should have no errors: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn valid_article_no_errors() {
        let html = r#"<script type="application/ld+json">{
            "@context": "https://schema.org",
            "@type": "Article",
            "headline": "Test Article",
            "author": {
                "@type": "Person",
                "name": "John Doe"
            }
        }</script>"#;
        let graph = extract_all(html).unwrap();
        let result = validation::validate(&graph);
        assert!(
            !result.has_errors(),
            "Valid article should have no errors: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn valid_event_no_errors() {
        let html = r#"<script type="application/ld+json">{
            "@context": "https://schema.org",
            "@type": "Event",
            "name": "Test Event",
            "startDate": "2024-06-15T19:00:00",
            "location": {
                "@type": "Place",
                "name": "Event Venue"
            }
        }</script>"#;
        let graph = extract_all(html).unwrap();
        let result = validation::validate(&graph);
        assert!(
            !result.has_errors(),
            "Valid event should have no errors: {:?}",
            result.diagnostics
        );
    }

    // Unknown type
    #[test]
    fn typo_in_type_reports_error() {
        let html = r#"<script type="application/ld+json">{
            "@context": "https://schema.org",
            "@type": "Produc",
            "name": "Test"
        }</script>"#;
        let graph = extract_all(html).unwrap();
        let result = validation::validate(&graph);
        assert!(result.has_errors());
        let unknown = result
            .errors()
            .find(|d| d.code == DiagnosticCode::UnknownType);
        assert!(unknown.is_some(), "Should detect unknown type");
        let msg = &unknown.unwrap().message;
        assert!(
            msg.contains("Produc"),
            "Message should mention the typo: {msg}"
        );
        assert!(
            msg.contains("Product"),
            "Message should suggest 'Product': {msg}"
        );
    }

    #[test]
    fn completely_invalid_type() {
        let html = r#"<script type="application/ld+json">{
            "@context": "https://schema.org",
            "@type": "XYZNotAType123",
            "name": "Test"
        }</script>"#;
        let graph = extract_all(html).unwrap();
        let result = validation::validate(&graph);
        assert!(result
            .errors()
            .any(|d| d.code == DiagnosticCode::UnknownType));
    }

    // Unknown property
    #[test]
    fn typo_in_property_reports_error() {
        let html = r#"<script type="application/ld+json">{
            "@context": "https://schema.org",
            "@type": "Product",
            "namee": "Test"
        }</script>"#;
        let graph = extract_all(html).unwrap();
        let result = validation::validate(&graph);
        assert!(result.has_errors());
        let unknown = result
            .errors()
            .find(|d| d.code == DiagnosticCode::UnknownProperty);
        assert!(unknown.is_some(), "Should detect unknown property");
        let msg = &unknown.unwrap().message;
        assert!(
            msg.contains("namee"),
            "Message should mention the typo: {msg}"
        );
        assert!(msg.contains("name"), "Message should suggest 'name': {msg}");
    }

    // Property not for type
    #[test]
    fn property_wrong_domain() {
        let html = r#"<script type="application/ld+json">{
            "@context": "https://schema.org",
            "@type": "Product",
            "name": "Widget",
            "recipeCategory": "Main Course"
        }</script>"#;
        let graph = extract_all(html).unwrap();
        let result = validation::validate(&graph);
        let wrong_domain = result
            .warnings()
            .find(|d| d.code == DiagnosticCode::PropertyNotForType);
        assert!(
            wrong_domain.is_some(),
            "Should detect property not valid for type: {:?}",
            result.diagnostics
        );
    }

    // Deprecated/superseded property
    #[test]
    fn superseded_property_warns() {
        // "episodes" is superseded by "episode"
        let html = r#"<script type="application/ld+json">{
            "@context": "https://schema.org",
            "@type": "TVSeries",
            "name": "Test Series",
            "episodes": {
                "@type": "Episode",
                "name": "Pilot"
            }
        }</script>"#;
        let graph = extract_all(html).unwrap();
        let result = validation::validate(&graph);
        let superseded = result
            .warnings()
            .find(|d| d.code == DiagnosticCode::DeprecatedProperty);
        assert!(
            superseded.is_some(),
            "Should detect superseded property: {:?}",
            result.diagnostics
        );
    }

    // Value type validation
    #[test]
    fn nested_node_wrong_type() {
        let html = r#"<script type="application/ld+json">{
            "@context": "https://schema.org",
            "@type": "Product",
            "name": "Widget",
            "offers": {
                "@type": "Person",
                "name": "Wrong Type"
            }
        }</script>"#;
        let graph = extract_all(html).unwrap();
        let result = validation::validate(&graph);
        let invalid = result
            .errors()
            .find(|d| d.code == DiagnosticCode::InvalidValueType);
        assert!(
            invalid.is_some(),
            "Should detect wrong nested type: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn text_where_url_expected_warns() {
        let html = r#"<script type="application/ld+json">{
            "@context": "https://schema.org",
            "@type": "Product",
            "name": "Widget",
            "image": "not a URL"
        }</script>"#;
        let graph = extract_all(html).unwrap();
        let result = validation::validate(&graph);
        let url_warn = result
            .warnings()
            .find(|d| d.code == DiagnosticCode::ExpectedUrlGotText);
        assert!(
            url_warn.is_some(),
            "Should warn about text where URL expected: {:?}",
            result.diagnostics
        );
    }

    // Path tracking
    #[test]
    fn diagnostic_paths_are_descriptive() {
        let html = r#"<script type="application/ld+json">{
            "@context": "https://schema.org",
            "@type": "Product",
            "name": "Widget",
            "offers": {
                "@type": "Offer",
                "pricee": "29.99"
            }
        }</script>"#;
        let graph = extract_all(html).unwrap();
        let result = validation::validate(&graph);
        let unknown = result
            .errors()
            .find(|d| d.code == DiagnosticCode::UnknownProperty);
        assert!(unknown.is_some());
        let path = &unknown.unwrap().path;
        assert!(
            path.contains("Offer"),
            "Path should contain nested type: {path}"
        );
        assert!(
            path.contains("pricee"),
            "Path should contain property: {path}"
        );
    }

    // Microdata validation
    #[test]
    fn microdata_validation() {
        let html = r#"<html><body>
        <div itemscope itemtype="https://schema.org/Product">
            <span itemprop="name">Widget</span>
            <span itemprop="unknownProp123">Bad</span>
        </div>
        </body></html>"#;
        let graph = extract_all(html).unwrap();
        let result = validation::validate(&graph);
        assert!(result
            .errors()
            .any(|d| d.code == DiagnosticCode::UnknownProperty));
    }

    // RDFa validation
    #[test]
    fn rdfa_validation() {
        let html = r#"<html><body>
        <div vocab="https://schema.org/" typeof="Produc">
            <span property="name">Widget</span>
        </div>
        </body></html>"#;
        let graph = extract_all(html).unwrap();
        let result = validation::validate(&graph);
        assert!(result
            .errors()
            .any(|d| d.code == DiagnosticCode::UnknownType));
    }

    // Empty / edge cases
    #[test]
    fn empty_graph_no_diagnostics() {
        let html = "<html></html>";
        let graph = extract_all(html).unwrap();
        let result = validation::validate(&graph);
        assert!(result.is_empty());
    }

    // Severity filtering
    #[test]
    fn validation_result_methods() {
        let html = r#"<script type="application/ld+json">{
            "@context": "https://schema.org",
            "@type": "Produc",
            "namee": "Test"
        }</script>"#;
        let graph = extract_all(html).unwrap();
        let result = validation::validate(&graph);
        assert!(result.has_errors());
        assert!(result.len() > 0);
        assert!(!result.is_empty());
        assert!(result.errors().count() > 0);
    }

    // Correct Offer nested in Product
    #[test]
    fn valid_product_with_offer_no_value_errors() {
        let html = r#"<script type="application/ld+json">{
            "@context": "https://schema.org",
            "@type": "Product",
            "name": "Widget",
            "offers": {
                "@type": "Offer",
                "price": "29.99",
                "priceCurrency": "EUR",
                "availability": "https://schema.org/InStock"
            }
        }</script>"#;
        let graph = extract_all(html).unwrap();
        let result = validation::validate(&graph);
        let errors: Vec<_> = result.errors().collect();
        assert!(
            errors.is_empty(),
            "Valid Product+Offer should have no errors: {errors:?}"
        );
    }

    // Boolean as string
    #[test]
    fn boolean_string_warns() {
        let html = r#"<script type="application/ld+json">{
            "@context": "https://schema.org",
            "@type": "Product",
            "name": "Widget",
            "isFamilyFriendly": "true"
        }</script>"#;
        let graph = extract_all(html).unwrap();
        let result = validation::validate(&graph);
        let bool_warn = result
            .warnings()
            .find(|d| d.code == DiagnosticCode::InvalidBoolean);
        assert!(
            bool_warn.is_some(),
            "Should warn about boolean as string: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn boolean_native_no_warning() {
        let html = r#"<script type="application/ld+json">{
            "@context": "https://schema.org",
            "@type": "Product",
            "name": "Widget",
            "isFamilyFriendly": true
        }</script>"#;
        let graph = extract_all(html).unwrap();
        let result = validation::validate(&graph);
        let bool_warn = result
            .diagnostics
            .iter()
            .find(|d| d.code == DiagnosticCode::InvalidBoolean);
        assert!(
            bool_warn.is_none(),
            "Native boolean should not warn: {:?}",
            result.diagnostics
        );
    }
}
