//! Vocabulary type definitions used by the generated Schema.org lookup code.
//!
//! These types are populated at compile time by `build.rs` from the vendored
//! `schemaorg-all-https.jsonld` vocabulary file. All data is stored as
//! `&'static` references -- zero heap allocation at runtime.

/// Definition of a Schema.org type (e.g. `Product`, `Person`, `Event`).
///
/// Each type knows its parent types (for inheritance), its own properties,
/// and the full set of properties including inherited ones.
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "validation")]
/// # {
/// use schemaorg_rs::vocabulary;
///
/// let product = vocabulary::lookup_type("Product").unwrap();
/// assert!(product.has_property("name"));    // inherited from Thing
/// assert!(product.has_property("offers"));  // own property
/// assert!(!product.has_property("recipeCategory")); // not valid for Product
/// # }
/// ```
#[derive(Debug)]
pub struct TypeDef {
    /// The local name of the type (e.g. `"Product"`).
    pub name: &'static str,
    /// Direct parent type names (e.g. `["Thing"]` for `Product`).
    pub parent_types: &'static [&'static str],
    /// Properties defined directly on this type (not inherited).
    pub own_properties: &'static [&'static str],
    /// All valid properties: own + inherited, sorted for binary search.
    pub all_properties: &'static [&'static str],
    /// Human-readable description from the Schema.org vocabulary.
    pub comment: &'static str,
    /// `true` if this type is in `pending.schema.org` (not yet stable).
    pub is_pending: bool,
    /// `true` if this type has been retired to `attic.schema.org`.
    pub is_attic: bool,
}

impl TypeDef {
    /// Checks if a property name is valid for this type (including inherited).
    ///
    /// Uses binary search on the sorted `all_properties` slice -- O(log n).
    #[must_use]
    #[inline]
    pub fn has_property(&self, name: &str) -> bool {
        self.all_properties.binary_search(&name).is_ok()
    }
}

/// Definition of a Schema.org property (e.g. `name`, `price`, `image`).
///
/// Each property knows what value types it expects (`expected_types`)
/// and which types it belongs to (`domain_types`).
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "validation")]
/// # {
/// use schemaorg_rs::vocabulary;
///
/// let price = vocabulary::lookup_property("price").unwrap();
/// assert!(price.expected_types.contains(&"Number"));
/// assert!(price.expected_types.contains(&"Text"));
/// assert!(price.domain_types.contains(&"Offer"));
/// # }
/// ```
#[derive(Debug)]
pub struct PropertyDef {
    /// The local name of the property (e.g. `"price"`).
    pub name: &'static str,
    /// Expected value types from `rangeIncludes` (e.g. `["Number", "Text"]`).
    pub expected_types: &'static [&'static str],
    /// Types this property belongs to from `domainIncludes` (e.g. `["Offer"]`).
    pub domain_types: &'static [&'static str],
    /// `true` if this property has been superseded by another.
    pub is_superseded: bool,
    /// The property that supersedes this one, if any.
    pub superseded_by: Option<&'static str>,
    /// `true` if this property is in `pending.schema.org`.
    pub is_pending: bool,
}

/// Definition of a Schema.org enumeration member (e.g. `InStock`, `Discontinued`).
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "validation")]
/// # {
/// use schemaorg_rs::vocabulary;
///
/// let member = vocabulary::lookup_enum_member("InStock").unwrap();
/// assert_eq!(member.enum_type, "ItemAvailability");
/// # }
/// ```
#[derive(Debug)]
pub struct EnumMemberDef {
    /// The local name of the enum member (e.g. `"InStock"`).
    pub name: &'static str,
    /// The enumeration type this member belongs to (e.g. `"ItemAvailability"`).
    pub enum_type: &'static str,
}
