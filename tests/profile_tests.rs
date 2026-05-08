//! Profile system tests  --  Google Rich Results + Baseline.

#[cfg(feature = "profiles")]
mod profile_tests {
    use schemaorg_rs::extract_all;
    use schemaorg_rs::profiles::{Eligibility, ProfileRegistry};
    use schemaorg_rs::validation;

    // Helpers
    fn eval_html(html: &str, profile: &str) -> schemaorg_rs::profiles::ProfileResult {
        let graph = extract_all(html).unwrap();
        let vocab = validation::validate(&graph);
        let reg = ProfileRegistry::with_google();
        reg.evaluate(profile, &graph, &vocab.diagnostics).unwrap()
    }

    fn jsonld(json: &str) -> String {
        format!(r#"<html><head><script type="application/ld+json">{json}</script></head></html>"#)
    }

    // Registry / Engine
    #[test]
    fn registry_google_has_profiles() {
        let reg = ProfileRegistry::with_google();
        assert!(reg.profile_names().contains(&"google"));
    }

    #[test]
    fn registry_baseline_has_profiles() {
        let reg = ProfileRegistry::with_baseline();
        assert!(reg.profile_names().contains(&"baseline"));
    }

    #[test]
    fn unknown_profile_errors() {
        let reg = ProfileRegistry::new();
        let graph = extract_all("<html></html>").unwrap();
        let err = reg.evaluate("nope", &graph, &[]);
        assert!(err.is_err());
    }

    #[test]
    fn empty_graph_not_eligible() {
        let r = eval_html("<html></html>", "google");
        assert_eq!(r.eligibility, Eligibility::NotEligible);
    }

    // Product
    #[test]
    fn product_valid_eligible() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Product",
            "name":"Widget","image":"https://x.com/w.jpg","description":"A widget",
            "brand":{"@type":"Brand","name":"Acme"},"sku":"W123","gtin":"012345678",
            "offers":{"@type":"Offer","price":"10","priceCurrency":"USD",
            "availability":"https://schema.org/InStock"},
            "aggregateRating":{"@type":"AggregateRating","ratingValue":"4.5","ratingCount":"10"},
            "review":{"@type":"Review","author":{"@type":"Person","name":"Bob"}}}"#,
        );
        let r = eval_html(&html, "google");
        // WarningsOnly because Review is missing recommended 'reviewRating'
        assert_eq!(r.eligibility, Eligibility::WarningsOnly);
        assert!(r
            .type_results
            .iter()
            .any(|t| t.schema_type == "Product" && t.eligible));
    }

    #[test]
    fn product_missing_name_not_eligible() {
        let html = jsonld(r#"{"@context":"https://schema.org","@type":"Product","sku":"X"}"#);
        let r = eval_html(&html, "google");
        assert_eq!(r.eligibility, Eligibility::NotEligible);
        assert!(r.type_results[0]
            .required_missing
            .contains(&"name".to_string()));
    }

    #[test]
    fn product_name_only_warnings() {
        let html = jsonld(r#"{"@context":"https://schema.org","@type":"Product","name":"W"}"#);
        let r = eval_html(&html, "google");
        // Has name (required), but missing recommended fields
        assert!(r.type_results[0].eligible);
        assert!(!r.type_results[0].recommended_missing.is_empty());
    }

    #[test]
    fn product_nested_offer_missing_price() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Product","name":"W",
            "offers":{"@type":"Offer","priceCurrency":"USD"}}"#,
        );
        let r = eval_html(&html, "google");
        assert_eq!(r.eligibility, Eligibility::NotEligible);
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.path.contains("offers") && d.path.contains("price")));
    }

    #[test]
    fn product_aggregate_rating_needs_count() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Product","name":"W",
            "aggregateRating":{"@type":"AggregateRating","ratingValue":"4"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.path.contains("aggregateRating") && d.message.contains("ratingCount")));
    }

    // Article
    #[test]
    fn article_valid_eligible() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Article",
            "headline":"Test","image":"https://x.com/a.jpg","datePublished":"2024-01-01",
            "author":{"@type":"Person","name":"Alice"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.type_results[0].eligible);
    }

    #[test]
    fn article_missing_headline() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Article",
            "image":"https://x.com/a.jpg","datePublished":"2024-01-01",
            "author":{"@type":"Person","name":"A"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(!r.type_results[0].eligible);
        assert!(r.type_results[0]
            .required_missing
            .contains(&"headline".to_string()));
    }

    #[test]
    fn newsarticle_subtype_matched() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"NewsArticle",
            "headline":"News","image":"https://x.com/a.jpg","datePublished":"2024-01-01",
            "author":{"@type":"Person","name":"A"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.type_results[0].eligible);
        assert_eq!(r.type_results[0].schema_type, "NewsArticle");
    }

    #[test]
    fn blogposting_subtype_matched() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"BlogPosting",
            "headline":"Blog","image":"https://x.com/b.jpg","datePublished":"2024-01-01",
            "author":{"@type":"Person","name":"A"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.type_results[0].eligible);
    }

    #[test]
    fn article_author_missing_name() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Article",
            "headline":"T","image":"https://x.com/a.jpg","datePublished":"2024-01-01",
            "author":{"@type":"Person","url":"https://x.com"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.path.contains("author") && d.path.contains("name")));
    }

    // FAQPage
    #[test]
    fn faqpage_valid_restricted() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"FAQPage",
            "mainEntity":[{"@type":"Question","name":"Q1",
            "acceptedAnswer":{"@type":"Answer","text":"A1"}}]}"#,
        );
        let r = eval_html(&html, "google");
        assert_eq!(r.eligibility, Eligibility::Restricted);
        assert!(r.type_results[0].eligible);
    }

    #[test]
    fn faqpage_missing_main_entity() {
        let html = jsonld(r#"{"@context":"https://schema.org","@type":"FAQPage"}"#);
        let r = eval_html(&html, "google");
        assert!(r.type_results[0]
            .required_missing
            .contains(&"mainEntity".to_string()));
    }

    #[test]
    fn faqpage_question_missing_answer() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"FAQPage",
            "mainEntity":[{"@type":"Question","name":"Q1"}]}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.path.contains("acceptedAnswer")));
    }

    #[test]
    fn faqpage_answer_missing_text() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"FAQPage",
            "mainEntity":[{"@type":"Question","name":"Q1",
            "acceptedAnswer":{"@type":"Answer"}}]}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.diagnostics.iter().any(|d| d.path.contains("text")));
    }

    // BreadcrumbList
    #[test]
    fn breadcrumb_valid_eligible() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"BreadcrumbList",
            "itemListElement":[
                {"@type":"ListItem","position":1,"name":"Home","item":"https://x.com"},
                {"@type":"ListItem","position":2,"name":"Category","item":"https://x.com/cat"},
                {"@type":"ListItem","position":3,"name":"Page"}
            ]}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.type_results[0].eligible);
    }

    #[test]
    fn breadcrumb_missing_items() {
        let html = jsonld(r#"{"@context":"https://schema.org","@type":"BreadcrumbList"}"#);
        let r = eval_html(&html, "google");
        assert!(!r.type_results[0].eligible);
    }

    #[test]
    fn breadcrumb_wrong_position() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"BreadcrumbList",
            "itemListElement":[
                {"@type":"ListItem","position":5,"name":"Home","item":"https://x.com"}
            ]}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.diagnostics.iter().any(|d| d.path.contains("position")));
    }

    #[test]
    fn breadcrumb_last_item_no_url_ok() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"BreadcrumbList",
            "itemListElement":[
                {"@type":"ListItem","position":1,"name":"Home","item":"https://x.com"},
                {"@type":"ListItem","position":2,"name":"Current"}
            ]}"#,
        );
        let r = eval_html(&html, "google");
        // Last item doesn't need "item"  --  should not have error for missing item
        let has_item_error = r
            .diagnostics
            .iter()
            .any(|d| d.path.contains("[1].item") && d.severity == schemaorg_rs::Severity::Error);
        assert!(!has_item_error);
    }

    // LocalBusiness
    #[test]
    fn local_business_valid() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"LocalBusiness",
            "name":"Acme Shop","address":{"@type":"PostalAddress",
            "streetAddress":"123 Main St","addressLocality":"Springfield",
            "postalCode":"12345","addressCountry":"US"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.type_results[0].eligible);
    }

    #[test]
    fn local_business_missing_address() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"LocalBusiness",
            "name":"Acme Shop"}"#,
        );
        let r = eval_html(&html, "google");
        assert!(!r.type_results[0].eligible);
        assert!(r.type_results[0]
            .required_missing
            .contains(&"address".to_string()));
    }

    #[test]
    fn restaurant_subtype_via_inheritance() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Restaurant",
            "name":"Pizza Place","address":{"@type":"PostalAddress",
            "streetAddress":"1 St","addressLocality":"Town",
            "postalCode":"000","addressCountry":"US"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(!r.type_results.is_empty());
        assert!(r.type_results[0].eligible);
    }

    #[test]
    fn local_business_address_missing_fields() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"LocalBusiness",
            "name":"Shop","address":{"@type":"PostalAddress","streetAddress":"1 St"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.path.contains("addressLocality")));
    }

    // Event
    #[test]
    fn event_valid() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Event",
            "name":"Concert","startDate":"2024-06-01T20:00",
            "location":{"@type":"Place","name":"Venue","address":"123 St"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.type_results[0].eligible);
    }

    #[test]
    fn event_missing_start_date() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Event",
            "name":"Concert","location":{"@type":"Place","name":"V","address":"1"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(!r.type_results[0].eligible);
        assert!(r.type_results[0]
            .required_missing
            .contains(&"startDate".to_string()));
    }

    #[test]
    fn event_missing_location() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Event",
            "name":"Concert","startDate":"2024-06-01T20:00"}"#,
        );
        let r = eval_html(&html, "google");
        assert!(!r.type_results[0].eligible);
    }

    #[test]
    fn event_recommended_fields_warned() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Event",
            "name":"E","startDate":"2024-06-01","location":{"@type":"Place",
            "name":"V","address":"1"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(!r.type_results[0].recommended_missing.is_empty());
    }

    // Recipe
    #[test]
    fn recipe_valid() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Recipe",
            "name":"Cake","image":"https://x.com/cake.jpg"}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.type_results[0].eligible);
    }

    #[test]
    fn recipe_missing_image() {
        let html = jsonld(r#"{"@context":"https://schema.org","@type":"Recipe","name":"Cake"}"#);
        let r = eval_html(&html, "google");
        assert!(!r.type_results[0].eligible);
        assert!(r.type_results[0]
            .required_missing
            .contains(&"image".to_string()));
    }

    #[test]
    fn recipe_recommended_fields() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Recipe",
            "name":"Cake","image":"https://x.com/c.jpg"}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.type_results[0].recommended_missing.len() > 5);
    }

    // Baseline
    #[test]
    fn baseline_with_name_eligible() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Thing",
            "name":"Test","image":"https://x.com/i.jpg","description":"A test"}"#,
        );
        let graph = extract_all(&html).unwrap();
        let vocab = validation::validate(&graph);
        let reg = ProfileRegistry::with_baseline();
        let r = reg
            .evaluate("baseline", &graph, &vocab.diagnostics)
            .unwrap();
        assert!(r.type_results[0].eligible);
    }

    #[test]
    fn baseline_missing_name_warns() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Thing",
            "description":"No name here"}"#,
        );
        let graph = extract_all(&html).unwrap();
        let vocab = validation::validate(&graph);
        let reg = ProfileRegistry::with_baseline();
        let r = reg
            .evaluate("baseline", &graph, &vocab.diagnostics)
            .unwrap();
        assert!(r.type_results[0]
            .recommended_missing
            .contains(&"name/headline".to_string()));
    }

    #[test]
    fn baseline_http_url_warns() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Thing",
            "name":"T","url":"http://insecure.com"}"#,
        );
        let graph = extract_all(&html).unwrap();
        let vocab = validation::validate(&graph);
        let reg = ProfileRegistry::with_baseline();
        let r = reg
            .evaluate("baseline", &graph, &vocab.diagnostics)
            .unwrap();
        assert!(r.diagnostics.iter().any(|d| d.message.contains("HTTPS")));
    }

    // Multi-type graph
    #[test]
    fn multi_type_graph_evaluates_all() {
        let html = jsonld(
            r#"[
            {"@context":"https://schema.org","@type":"Product","name":"W"},
            {"@context":"https://schema.org","@type":"Article","headline":"H",
             "image":"https://x.com/a.jpg","datePublished":"2024-01-01",
             "author":{"@type":"Person","name":"A"}}
        ]"#,
        );
        let r = eval_html(&html, "google");
        assert_eq!(r.type_results.len(), 2);
    }

    // Additional Product tests (->12 total)
    #[test]
    fn product_offer_missing_currency() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Product","name":"W",
            "offers":{"@type":"Offer","price":"10","availability":"InStock"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.path.contains("priceCurrency")));
    }

    #[test]
    fn product_offer_missing_availability() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Product","name":"W",
            "offers":{"@type":"Offer","price":"10","priceCurrency":"USD"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.path.contains("availability")));
    }

    #[test]
    fn product_review_missing_author() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Product","name":"W",
            "review":{"@type":"Review","reviewBody":"Good"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.path.contains("review") && d.path.contains("author")));
    }

    #[test]
    fn product_multiple_offers() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Product","name":"W",
            "offers":[
                {"@type":"Offer","price":"10","priceCurrency":"USD","availability":"InStock"},
                {"@type":"Offer","price":"15","priceCurrency":"EUR","availability":"InStock"}
            ]}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r
            .type_results
            .iter()
            .any(|t| t.schema_type == "Product" && t.eligible));
    }

    #[test]
    fn product_recommended_image_missing() {
        let html = jsonld(r#"{"@context":"https://schema.org","@type":"Product","name":"W"}"#);
        let r = eval_html(&html, "google");
        assert!(r
            .type_results
            .iter()
            .any(|t| t.recommended_missing.contains(&"image".to_string())));
    }

    #[test]
    fn product_recommended_description_missing() {
        let html = jsonld(r#"{"@context":"https://schema.org","@type":"Product","name":"W"}"#);
        let r = eval_html(&html, "google");
        assert!(r
            .type_results
            .iter()
            .any(|t| t.recommended_missing.contains(&"description".to_string())));
    }

    #[test]
    fn product_aggregate_rating_with_review_count() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Product","name":"W",
            "aggregateRating":{"@type":"AggregateRating","ratingValue":"4","reviewCount":"5"}}"#,
        );
        let r = eval_html(&html, "google");
        // reviewCount satisfies the ratingCount|reviewCount requirement
        let has_count_error = r.diagnostics.iter().any(|d| {
            d.path.contains("aggregateRating")
                && d.message.contains("ratingCount")
                && d.severity == schemaorg_rs::Severity::Error
        });
        assert!(!has_count_error);
    }

    // Additional Article tests (->10 total)
    #[test]
    fn article_missing_image() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Article",
            "headline":"T","datePublished":"2024-01-01",
            "author":{"@type":"Person","name":"A"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(!r.type_results[0].eligible);
        assert!(r.type_results[0]
            .required_missing
            .contains(&"image".to_string()));
    }

    #[test]
    fn article_missing_date_published() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Article",
            "headline":"T","image":"https://x.com/a.jpg",
            "author":{"@type":"Person","name":"A"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(!r.type_results[0].eligible);
        assert!(r.type_results[0]
            .required_missing
            .contains(&"datePublished".to_string()));
    }

    #[test]
    fn article_missing_author() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Article",
            "headline":"T","image":"https://x.com/a.jpg","datePublished":"2024-01-01"}"#,
        );
        let r = eval_html(&html, "google");
        assert!(!r.type_results[0].eligible);
        assert!(r.type_results[0]
            .required_missing
            .contains(&"author".to_string()));
    }

    #[test]
    fn article_recommended_publisher_missing() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Article",
            "headline":"T","image":"https://x.com/a.jpg","datePublished":"2024-01-01",
            "author":{"@type":"Person","name":"A"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.type_results[0]
            .recommended_missing
            .contains(&"publisher".to_string()));
    }

    #[test]
    fn article_publisher_missing_name() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Article",
            "headline":"T","image":"https://x.com/a.jpg","datePublished":"2024-01-01",
            "author":{"@type":"Person","name":"A"},
            "publisher":{"@type":"Organization","url":"https://x.com"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.path.contains("publisher") && d.path.contains("name")));
    }

    // Additional FAQPage tests (->8 total)
    #[test]
    fn faqpage_multiple_questions() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"FAQPage",
            "mainEntity":[
                {"@type":"Question","name":"Q1","acceptedAnswer":{"@type":"Answer","text":"A1"}},
                {"@type":"Question","name":"Q2","acceptedAnswer":{"@type":"Answer","text":"A2"}},
                {"@type":"Question","name":"Q3","acceptedAnswer":{"@type":"Answer","text":"A3"}}
            ]}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.type_results[0].eligible);
    }

    #[test]
    fn faqpage_question_missing_name() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"FAQPage",
            "mainEntity":[{"@type":"Question",
            "acceptedAnswer":{"@type":"Answer","text":"A"}}]}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.path.contains("mainEntity") && d.path.contains("name")));
    }

    #[test]
    fn faqpage_eligibility_restricted_diagnostic() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"FAQPage",
            "mainEntity":[{"@type":"Question","name":"Q",
            "acceptedAnswer":{"@type":"Answer","text":"A"}}]}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.code == schemaorg_rs::DiagnosticCode::EligibilityRestricted));
    }

    #[test]
    fn faqpage_empty_not_eligible() {
        let html = jsonld(r#"{"@context":"https://schema.org","@type":"FAQPage"}"#);
        let r = eval_html(&html, "google");
        assert!(!r.type_results[0].eligible);
    }

    // Additional BreadcrumbList tests (->8 total)
    #[test]
    fn breadcrumb_single_item() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"BreadcrumbList",
            "itemListElement":[
                {"@type":"ListItem","position":1,"name":"Home"}
            ]}"#,
        );
        let r = eval_html(&html, "google");
        // Single item = last item, so no "item" URL required
        assert!(r.type_results[0].eligible);
    }

    #[test]
    fn breadcrumb_missing_name_in_item() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"BreadcrumbList",
            "itemListElement":[
                {"@type":"ListItem","position":1,"item":"https://x.com"}
            ]}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.diagnostics.iter().any(|d| d.path.contains("name")));
    }

    #[test]
    fn breadcrumb_missing_position() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"BreadcrumbList",
            "itemListElement":[
                {"@type":"ListItem","name":"Home","item":"https://x.com"}
            ]}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.diagnostics.iter().any(|d| d.path.contains("position")));
    }

    #[test]
    fn breadcrumb_non_last_item_needs_url() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"BreadcrumbList",
            "itemListElement":[
                {"@type":"ListItem","position":1,"name":"Home"},
                {"@type":"ListItem","position":2,"name":"Page"}
            ]}"#,
        );
        let r = eval_html(&html, "google");
        // First item (not last) should require "item" URL
        assert!(r.diagnostics.iter().any(|d| d.path.contains("[0].item")));
    }

    // Additional LocalBusiness tests (->10 total)
    #[test]
    fn local_business_missing_name() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"LocalBusiness",
            "address":{"@type":"PostalAddress","streetAddress":"1 St",
            "addressLocality":"Town","postalCode":"000","addressCountry":"US"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(!r.type_results[0].eligible);
        assert!(r.type_results[0]
            .required_missing
            .contains(&"name".to_string()));
    }

    #[test]
    fn local_business_missing_both() {
        let html = jsonld(r#"{"@context":"https://schema.org","@type":"LocalBusiness"}"#);
        let r = eval_html(&html, "google");
        assert!(!r.type_results[0].eligible);
        assert!(r.type_results[0].required_missing.len() >= 2);
    }

    #[test]
    fn local_business_recommended_fields() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"LocalBusiness",
            "name":"Shop","address":{"@type":"PostalAddress","streetAddress":"1",
            "addressLocality":"T","postalCode":"0","addressCountry":"US"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.type_results[0]
            .recommended_missing
            .contains(&"telephone".to_string()));
        assert!(r.type_results[0]
            .recommended_missing
            .contains(&"url".to_string()));
    }

    #[test]
    fn local_business_address_missing_postal_code() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"LocalBusiness",
            "name":"Shop","address":{"@type":"PostalAddress",
            "streetAddress":"1","addressLocality":"T","addressCountry":"US"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.diagnostics.iter().any(|d| d.path.contains("postalCode")));
    }

    #[test]
    fn local_business_address_missing_country() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"LocalBusiness",
            "name":"Shop","address":{"@type":"PostalAddress",
            "streetAddress":"1","addressLocality":"T","postalCode":"0"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.path.contains("addressCountry")));
    }

    #[test]
    fn store_subtype_via_inheritance() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Store",
            "name":"My Store","address":{"@type":"PostalAddress",
            "streetAddress":"1","addressLocality":"T",
            "postalCode":"0","addressCountry":"US"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(!r.type_results.is_empty());
    }

    // Additional Event tests (->10 total)
    #[test]
    fn event_missing_name() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Event",
            "startDate":"2024-06-01T20:00",
            "location":{"@type":"Place","name":"V","address":"1"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(!r.type_results[0].eligible);
        assert!(r.type_results[0]
            .required_missing
            .contains(&"name".to_string()));
    }

    #[test]
    fn event_all_required_only() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Event",
            "name":"E","startDate":"2024-06-01",
            "location":{"@type":"Place","name":"V","address":"1"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.type_results[0].eligible);
        assert!(r.type_results[0].required_missing.is_empty());
    }

    #[test]
    fn event_missing_all_required() {
        let html = jsonld(r#"{"@context":"https://schema.org","@type":"Event"}"#);
        let r = eval_html(&html, "google");
        assert!(!r.type_results[0].eligible);
        assert!(r.type_results[0].required_missing.len() >= 3);
    }

    #[test]
    fn event_place_missing_address() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Event",
            "name":"E","startDate":"2024-06-01",
            "location":{"@type":"Place","name":"Venue"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.path.contains("location") && d.path.contains("address")));
    }

    #[test]
    fn event_recommended_offers() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Event",
            "name":"E","startDate":"2024-06-01",
            "location":{"@type":"Place","name":"V","address":"1"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.type_results[0]
            .recommended_missing
            .contains(&"offers".to_string()));
    }

    #[test]
    fn event_recommended_description() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Event",
            "name":"E","startDate":"2024-06-01",
            "location":{"@type":"Place","name":"V","address":"1"}}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.type_results[0]
            .recommended_missing
            .contains(&"description".to_string()));
    }

    // Additional Recipe tests (->10 total)
    #[test]
    fn recipe_missing_name() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Recipe",
            "image":"https://x.com/c.jpg"}"#,
        );
        let r = eval_html(&html, "google");
        assert!(!r.type_results[0].eligible);
        assert!(r.type_results[0]
            .required_missing
            .contains(&"name".to_string()));
    }

    #[test]
    fn recipe_missing_both_required() {
        let html = jsonld(r#"{"@context":"https://schema.org","@type":"Recipe"}"#);
        let r = eval_html(&html, "google");
        assert!(!r.type_results[0].eligible);
        assert_eq!(r.type_results[0].required_missing.len(), 2);
    }

    #[test]
    fn recipe_recommended_author() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Recipe",
            "name":"C","image":"https://x.com/c.jpg"}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.type_results[0]
            .recommended_missing
            .contains(&"author".to_string()));
    }

    #[test]
    fn recipe_recommended_instructions() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Recipe",
            "name":"C","image":"https://x.com/c.jpg"}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.type_results[0]
            .recommended_missing
            .contains(&"recipeInstructions".to_string()));
    }

    #[test]
    fn recipe_recommended_ingredients() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Recipe",
            "name":"C","image":"https://x.com/c.jpg"}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.type_results[0]
            .recommended_missing
            .contains(&"recipeIngredient".to_string()));
    }

    #[test]
    fn recipe_recommended_nutrition() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Recipe",
            "name":"C","image":"https://x.com/c.jpg"}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.type_results[0]
            .recommended_missing
            .contains(&"nutrition".to_string()));
    }

    #[test]
    fn recipe_with_howto_steps() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Recipe",
            "name":"C","image":"https://x.com/c.jpg",
            "recipeInstructions":[
                {"@type":"HowToStep","text":"Mix ingredients"},
                {"@type":"HowToStep","text":"Bake at 350F"}
            ]}"#,
        );
        let r = eval_html(&html, "google");
        assert!(r.type_results[0].eligible);
    }

    // Additional Baseline tests (->8 total)
    #[test]
    fn baseline_headline_accepted() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Thing",
            "headline":"Test","image":"https://x.com/i.jpg","description":"D"}"#,
        );
        let graph = extract_all(&html).unwrap();
        let vocab = validation::validate(&graph);
        let reg = ProfileRegistry::with_baseline();
        let r = reg
            .evaluate("baseline", &graph, &vocab.diagnostics)
            .unwrap();
        // headline satisfies name/headline requirement
        assert!(!r.type_results[0]
            .recommended_missing
            .contains(&"name/headline".to_string()));
    }

    #[test]
    fn baseline_missing_image() {
        let html = jsonld(r#"{"@context":"https://schema.org","@type":"Thing","name":"T"}"#);
        let graph = extract_all(&html).unwrap();
        let vocab = validation::validate(&graph);
        let reg = ProfileRegistry::with_baseline();
        let r = reg
            .evaluate("baseline", &graph, &vocab.diagnostics)
            .unwrap();
        assert!(r.type_results[0]
            .recommended_missing
            .contains(&"image".to_string()));
    }

    #[test]
    fn baseline_missing_description() {
        let html = jsonld(r#"{"@context":"https://schema.org","@type":"Thing","name":"T"}"#);
        let graph = extract_all(&html).unwrap();
        let vocab = validation::validate(&graph);
        let reg = ProfileRegistry::with_baseline();
        let r = reg
            .evaluate("baseline", &graph, &vocab.diagnostics)
            .unwrap();
        assert!(r.type_results[0]
            .recommended_missing
            .contains(&"description".to_string()));
    }

    #[test]
    fn baseline_https_url_no_warning() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Thing",
            "name":"T","url":"https://secure.com"}"#,
        );
        let graph = extract_all(&html).unwrap();
        let vocab = validation::validate(&graph);
        let reg = ProfileRegistry::with_baseline();
        let r = reg
            .evaluate("baseline", &graph, &vocab.diagnostics)
            .unwrap();
        assert!(!r.diagnostics.iter().any(|d| d.message.contains("HTTPS")));
    }

    #[test]
    fn baseline_http_image_warns() {
        let html = jsonld(
            r#"{"@context":"https://schema.org","@type":"Thing",
            "name":"T","image":"http://insecure.com/img.jpg"}"#,
        );
        let graph = extract_all(&html).unwrap();
        let vocab = validation::validate(&graph);
        let reg = ProfileRegistry::with_baseline();
        let r = reg
            .evaluate("baseline", &graph, &vocab.diagnostics)
            .unwrap();
        assert!(r.diagnostics.iter().any(|d| d.message.contains("HTTPS")));
    }

    // Additional Engine/Registry tests (->6 total)
    #[test]
    fn registry_default_is_empty() {
        let reg = ProfileRegistry::default();
        assert!(reg.profile_names().is_empty());
    }

    #[test]
    fn eligibility_display() {
        assert_eq!(Eligibility::Eligible.to_string(), "Eligible");
        assert_eq!(Eligibility::WarningsOnly.to_string(), "WarningsOnly");
        assert_eq!(Eligibility::NotEligible.to_string(), "NotEligible");
        assert_eq!(Eligibility::Restricted.to_string(), "Restricted");
    }

    #[test]
    fn profile_error_display() {
        use schemaorg_rs::profiles::ProfileError;
        let e = ProfileError::UnknownProfile("foo".to_string());
        assert!(e.to_string().contains("foo"));
        let e2 = ProfileError::NoMatchingTypes;
        assert!(!e2.to_string().is_empty());
    }
}
