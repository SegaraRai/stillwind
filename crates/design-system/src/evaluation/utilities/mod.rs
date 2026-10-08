//! Utility resolution: matching a candidate name to definition branches and turning the
//! matched body into declarations.
//!
//! A candidate may match an exact definition, several wildcard families, or none.
//! [`matching`] decides which branches are relevant and how competing branches merge,
//! [`body`] and [`nested`] split a CSS body into its selector, declarations, and
//! conditional rules, [`declarations`] resolves placeholders per declaration,
//! [`formatting`] applies the registry's serialization metadata, and [`resolve`] is the
//! public entry point that ties them together. [`order`] retains pre-emission Tailwind
//! property/count keys and authored precedence. [`tailwind_property_order`] supplies
//! its generated property indices.
//!
//! Value-domain mismatches are non-matches, allowing another overload to apply.
//! Malformed matched definitions return a typed error.

pub(super) mod body;
pub(super) mod declarations;
pub(super) mod formatting;
pub(super) mod matching;
pub(super) mod nested;
pub(super) mod order;
pub(super) mod resolve;
mod tailwind_property_order;

pub use body::UtilityResolutionError;
pub use nested::{NestedUtilityAtRule, NestedUtilityRule};
pub use order::{UtilityOrder, UtilityPropertySort};
pub use resolve::{
    ResolvedUtility, ResolvedUtilityTarget, UtilityArbitraryUse, UtilityTokenUse,
    resolve_animation_option, resolve_utility,
};
