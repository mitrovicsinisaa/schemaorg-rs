//! Schema.org vocabulary lookup -- generated at compile time from the official definitions.
//!
//! This module provides zero-cost lookup functions for Schema.org types, properties,
//! and enumeration members. All data is compiled into static match statements by
//! `build.rs`, meaning **zero heap allocation and zero runtime parsing**.
//!
//! # Architecture
//!
//! ```text
//! schemaorg-all-https.jsonld  ->  build.rs  ->  generated.rs (in $OUT_DIR)
//!                                                  |
//!                                          lookup_type("Product")
//!                                          lookup_property("price")
//!                                          lookup_enum_member("InStock")
//! ```
//!
//! # Examples
//!
//! ```no_run
//! # #[cfg(feature = "validation")]
//! # {
//! use schemaorg_rs::vocabulary;
//!
//! // Type lookup
//! let product = vocabulary::lookup_type("Product").unwrap();
//! assert!(product.has_property("name"));
//!
//! // Property lookup
//! let price = vocabulary::lookup_property("price").unwrap();
//! assert!(price.expected_types.contains(&"Number"));
//!
//! // Schema version
//! let version = vocabulary::schema_version();
//! println!("Using Schema.org v{version}");
//! # }
//! ```

pub mod types;

#[allow(clippy::too_many_lines)]
mod generated {
    use super::types::{EnumMemberDef, PropertyDef, TypeDef};
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

pub use generated::{lookup_enum_member, lookup_property, lookup_type, schema_version};
pub use types::{EnumMemberDef, PropertyDef, TypeDef};

/// Returns a list of all known Schema.org type names.
///
/// Useful for "did you mean?" suggestions and autocomplete.
#[must_use]
pub fn all_type_names() -> &'static [&'static str] {
    generated::ALL_TYPE_NAMES
}

/// Returns a list of all known Schema.org property names.
///
/// Useful for "did you mean?" suggestions and autocomplete.
#[must_use]
pub fn all_property_names() -> &'static [&'static str] {
    generated::ALL_PROPERTY_NAMES
}

/// Checks if `child_type` is a subtype of `ancestor_type` by walking
/// the Schema.org type hierarchy via BFS.
///
/// Uses `&str` references from the static vocabulary data to avoid
/// heap allocations during the traversal.
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "validation")]
/// # {
/// use schemaorg_rs::vocabulary;
///
/// assert!(vocabulary::is_subtype("NewsArticle", "Article"));
/// assert!(vocabulary::is_subtype("Product", "Thing"));
/// assert!(!vocabulary::is_subtype("Product", "Person"));
/// # }
/// ```
#[must_use]
pub fn is_subtype(child_type: &str, ancestor_type: &str) -> bool {
    let Some(child_def) = lookup_type(child_type) else {
        return false;
    };

    let mut queue: Vec<&str> = child_def.parent_types.to_vec();
    let mut visited = std::collections::HashSet::new();

    while let Some(current) = queue.pop() {
        if current == ancestor_type {
            return true;
        }
        if !visited.insert(current) {
            continue;
        }
        if let Some(td) = lookup_type(current) {
            queue.extend_from_slice(td.parent_types);
        }
    }
    false
}
