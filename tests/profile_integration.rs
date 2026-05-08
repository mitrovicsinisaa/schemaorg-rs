//! End-to-end integration tests: HTML -> extract -> validate -> profile.
//!
//! Tests the full pipeline from raw HTML through to profile eligibility.

#[cfg(feature = "profiles")]
mod profile_integration {
    use schemaorg_rs::extract_all;
    use schemaorg_rs::profiles::{Eligibility, ProfileRegistry};
    use schemaorg_rs::validation;

    /// Full pipeline helper: HTML string -> ProfileResult
    fn full_pipeline(html: &str, profile: &str) -> schemaorg_rs::profiles::ProfileResult {
        let graph = extract_all(html).unwrap();
        let vocab = validation::validate(&graph);
        let reg = ProfileRegistry::with_google();
        reg.evaluate(profile, &graph, &vocab.diagnostics).unwrap()
    }

    #[test]
    fn microdata_product_eligible() {
        let html = r#"<html><body>
            <div itemscope itemtype="https://schema.org/Product">
                <span itemprop="name">Widget</span>
                <img itemprop="image" src="https://example.com/widget.jpg">
                <meta itemprop="description" content="A great widget">
                <meta itemprop="sku" content="W123">
                <meta itemprop="gtin" content="012345678">
                <div itemprop="brand" itemscope itemtype="https://schema.org/Brand">
                    <span itemprop="name">Acme</span>
                </div>
            </div>
        </body></html>"#;
        let r = full_pipeline(html, "google");
        assert!(r
            .type_results
            .iter()
            .any(|t| t.schema_type == "Product" && t.eligible));
    }

    #[test]
    fn rdfa_article_eligible() {
        let html = r#"<html><body>
            <div vocab="https://schema.org/" typeof="Article">
                <h1 property="headline">Test Article</h1>
                <img property="image" src="https://example.com/a.jpg">
                <time property="datePublished" datetime="2024-01-15">Jan 15</time>
                <div property="author" typeof="Person">
                    <span property="name">Alice</span>
                </div>
            </div>
        </body></html>"#;
        let r = full_pipeline(html, "google");
        assert!(r
            .type_results
            .iter()
            .any(|t| t.schema_type == "Article" && t.eligible));
    }

    #[test]
    fn jsonld_and_microdata_mixed() {
        let html = r#"<html><head>
            <script type="application/ld+json">{
                "@context": "https://schema.org",
                "@type": "Product",
                "name": "Widget"
            }</script>
        </head><body>
            <div itemscope itemtype="https://schema.org/Event">
                <span itemprop="name">Concert</span>
                <time itemprop="startDate" datetime="2024-06-01T20:00">June 1</time>
                <div itemprop="location" itemscope itemtype="https://schema.org/Place">
                    <span itemprop="name">Venue</span>
                    <span itemprop="address">123 Main St</span>
                </div>
            </div>
        </body></html>"#;
        let r = full_pipeline(html, "google");
        assert!(r.type_results.len() >= 2);
        assert!(r.type_results.iter().any(|t| t.schema_type == "Product"));
        assert!(r.type_results.iter().any(|t| t.schema_type == "Event"));
    }

    #[test]
    fn vocab_errors_dont_block_profile() {
        // Product with unknown property  --  vocab warns, but profile still evaluates
        let html = r#"<html><head>
            <script type="application/ld+json">{
                "@context": "https://schema.org",
                "@type": "Product",
                "name": "Widget",
                "fakeProperty": "value"
            }</script>
        </head></html>"#;
        let graph = extract_all(html).unwrap();
        let vocab = validation::validate(&graph);
        assert!(vocab.has_errors()); // fakeProperty is unknown
        let reg = ProfileRegistry::with_google();
        let r = reg.evaluate("google", &graph, &vocab.diagnostics).unwrap();
        // Profile still evaluates  --  Product is eligible (name present)
        assert!(r
            .type_results
            .iter()
            .any(|t| t.schema_type == "Product" && t.eligible));
    }

    #[test]
    fn empty_html_no_types() {
        let r = full_pipeline("<html></html>", "google");
        assert_eq!(r.eligibility, Eligibility::NotEligible);
        assert!(r.type_results.is_empty());
    }

    #[test]
    fn non_google_type_ignored() {
        let html = r#"<html><head>
            <script type="application/ld+json">{
                "@context": "https://schema.org",
                "@type": "Person",
                "name": "Alice"
            }</script>
        </head></html>"#;
        let r = full_pipeline(html, "google");
        // Person is not a Google profile type  --  no type_results
        assert!(r.type_results.iter().all(|t| t.schema_type != "Person"));
    }

    #[test]
    fn baseline_profile_matches_all_types() {
        let html = r#"<html><head>
            <script type="application/ld+json">{
                "@context": "https://schema.org",
                "@type": "CreativeWork",
                "name": "Test",
                "image": "https://example.com/i.jpg",
                "description": "Desc"
            }</script>
        </head></html>"#;
        let graph = extract_all(html).unwrap();
        let vocab = validation::validate(&graph);
        let reg = ProfileRegistry::with_baseline();
        let r = reg
            .evaluate("baseline", &graph, &vocab.diagnostics)
            .unwrap();
        // CreativeWork is a subtype of Thing -> baseline matches
        assert!(!r.type_results.is_empty());
    }

    #[test]
    fn full_pipeline_preserves_source_format() {
        let html = r#"<html><head>
            <script type="application/ld+json">{
                "@context": "https://schema.org",
                "@type": "Recipe",
                "name": "Cake",
                "image": "https://example.com/cake.jpg"
            }</script>
        </head></html>"#;
        let graph = extract_all(html).unwrap();
        assert_eq!(
            graph.nodes[0].source_format,
            schemaorg_rs::SourceFormat::JsonLd
        );
        let vocab = validation::validate(&graph);
        let reg = ProfileRegistry::with_google();
        let r = reg.evaluate("google", &graph, &vocab.diagnostics).unwrap();
        assert!(r
            .type_results
            .iter()
            .any(|t| t.schema_type == "Recipe" && t.eligible));
    }
}
