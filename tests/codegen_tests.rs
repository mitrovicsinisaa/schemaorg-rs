//! Codegen verification tests  --  ensure build.rs produced correct vocabulary data.

#[cfg(feature = "validation")]
mod codegen {
    use schemaorg_rs::vocabulary;

    // Type lookup
    #[test]
    fn lookup_known_types() {
        assert!(vocabulary::lookup_type("Product").is_some());
        assert!(vocabulary::lookup_type("Thing").is_some());
        assert!(vocabulary::lookup_type("Person").is_some());
        assert!(vocabulary::lookup_type("Offer").is_some());
        assert!(vocabulary::lookup_type("Event").is_some());
        assert!(vocabulary::lookup_type("Article").is_some());
        assert!(vocabulary::lookup_type("Organization").is_some());
        assert!(vocabulary::lookup_type("LocalBusiness").is_some());
    }

    #[test]
    fn lookup_unknown_types() {
        assert!(vocabulary::lookup_type("NotAType").is_none());
        assert!(vocabulary::lookup_type("").is_none());
        assert!(vocabulary::lookup_type("Produc").is_none());
        assert!(vocabulary::lookup_type("product").is_none()); // case-sensitive
    }

    #[test]
    fn data_types_excluded() {
        // DataTypes should NOT appear as regular types
        assert!(vocabulary::lookup_type("Text").is_none());
        assert!(vocabulary::lookup_type("Number").is_none());
        assert!(vocabulary::lookup_type("Boolean").is_none());
        assert!(vocabulary::lookup_type("URL").is_none());
    }

    // Inheritance
    #[test]
    fn product_inherits_thing_properties() {
        let product = vocabulary::lookup_type("Product").unwrap();
        // Own property
        assert!(product.has_property("offers"));
        assert!(product.has_property("brand"));
        assert!(product.has_property("sku"));
        // Inherited from Thing
        assert!(product.has_property("name"));
        assert!(product.has_property("description"));
        assert!(product.has_property("url"));
        assert!(product.has_property("image"));
        // Not valid for Product
        assert!(!product.has_property("recipeCategory"));
        assert!(!product.has_property("startDate"));
    }

    #[test]
    fn multi_parent_inheritance() {
        // LocalBusiness extends both Organization and Place
        let lb = vocabulary::lookup_type("LocalBusiness").unwrap();
        assert!(lb.parent_types.contains(&"Organization"));
        assert!(lb.parent_types.contains(&"Place"));
        // Should have properties from Organization
        assert!(lb.has_property("employee") || lb.has_property("founder"));
        // Should have properties from Place
        assert!(lb.has_property("geo") || lb.has_property("latitude"));
        // Should have properties from Thing (via Organization/Place)
        assert!(lb.has_property("name"));
        assert!(lb.has_property("url"));
    }

    #[test]
    fn thing_is_root_type() {
        let thing = vocabulary::lookup_type("Thing").unwrap();
        assert!(thing.parent_types.is_empty());
        assert!(thing.has_property("name"));
        assert!(thing.has_property("url"));
    }

    // Property lookup
    #[test]
    fn lookup_known_properties() {
        assert!(vocabulary::lookup_property("name").is_some());
        assert!(vocabulary::lookup_property("price").is_some());
        assert!(vocabulary::lookup_property("image").is_some());
        assert!(vocabulary::lookup_property("offers").is_some());
    }

    #[test]
    fn lookup_unknown_properties() {
        assert!(vocabulary::lookup_property("notaproperty").is_none());
        assert!(vocabulary::lookup_property("").is_none());
    }

    #[test]
    fn property_constraints() {
        let price = vocabulary::lookup_property("price").unwrap();
        assert!(price.expected_types.contains(&"Number"));
        assert!(price.expected_types.contains(&"Text"));
        assert!(price.domain_types.contains(&"Offer"));
    }

    #[test]
    fn name_property_from_thing() {
        let name = vocabulary::lookup_property("name").unwrap();
        assert!(name.expected_types.contains(&"Text"));
        assert!(name.domain_types.contains(&"Thing"));
    }

    // Enum members
    #[test]
    fn lookup_known_enum_members() {
        let member = vocabulary::lookup_enum_member("InStock").unwrap();
        assert_eq!(member.name, "InStock");
        assert_eq!(member.enum_type, "ItemAvailability");
    }

    #[test]
    fn lookup_unknown_enum_member() {
        assert!(vocabulary::lookup_enum_member("NotAnEnumMember").is_none());
    }

    #[test]
    fn out_of_stock_enum() {
        let member = vocabulary::lookup_enum_member("OutOfStock").unwrap();
        assert_eq!(member.enum_type, "ItemAvailability");
    }

    // Version
    #[test]
    fn schema_version_is_set() {
        let version = vocabulary::schema_version();
        assert!(!version.is_empty());
        assert!(
            version.contains('.'),
            "Version should contain a dot: {version}"
        );
    }

    // Name lists
    #[test]
    fn all_type_names_non_empty() {
        let names = vocabulary::all_type_names();
        assert!(
            names.len() > 500,
            "Expected 500+ types, got {}",
            names.len()
        );
        assert!(names.contains(&"Product"));
        assert!(names.contains(&"Thing"));
    }

    #[test]
    fn all_property_names_non_empty() {
        let names = vocabulary::all_property_names();
        assert!(
            names.len() > 1000,
            "Expected 1000+ properties, got {}",
            names.len()
        );
        assert!(names.contains(&"name"));
        assert!(names.contains(&"price"));
    }

    #[test]
    fn all_type_names_sorted() {
        let names = vocabulary::all_type_names();
        for window in names.windows(2) {
            assert!(
                window[0] <= window[1],
                "Type names not sorted: {} > {}",
                window[0],
                window[1]
            );
        }
    }

    #[test]
    fn all_properties_sorted_for_binary_search() {
        let product = vocabulary::lookup_type("Product").unwrap();
        for window in product.all_properties.windows(2) {
            assert!(
                window[0] <= window[1],
                "Product.all_properties not sorted: {} > {}",
                window[0],
                window[1]
            );
        }
    }
}
