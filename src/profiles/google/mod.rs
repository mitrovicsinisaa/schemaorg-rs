//! Google Rich Results profiles.
//!
//! Implements validation for 7 Schema.org types that Google supports
//! for rich result display in Search.

pub(crate) mod common;

mod article;
mod breadcrumb;
mod event;
mod faqpage;
mod local_business;
mod product;
mod recipe;

pub use article::GoogleArticleProfile;
pub use breadcrumb::GoogleBreadcrumbProfile;
pub use event::GoogleEventProfile;
pub use faqpage::GoogleFaqPageProfile;
pub use local_business::GoogleLocalBusinessProfile;
pub use product::GoogleProductProfile;
pub use recipe::GoogleRecipeProfile;

use super::ProfileRegistry;

/// Registers all Google Rich Results profiles in the given registry.
pub fn register_all(registry: &mut ProfileRegistry) {
    registry.register(Box::new(GoogleProductProfile));
    registry.register(Box::new(GoogleArticleProfile));
    registry.register(Box::new(GoogleFaqPageProfile));
    registry.register(Box::new(GoogleBreadcrumbProfile));
    registry.register(Box::new(GoogleLocalBusinessProfile));
    registry.register(Box::new(GoogleEventProfile));
    registry.register(Box::new(GoogleRecipeProfile));
}
