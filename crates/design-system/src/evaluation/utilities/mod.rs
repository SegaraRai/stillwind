//! Utility resolution: matching a candidate name to definition branches and turning the
//! matched body into declarations.
//!
//! A candidate may match an exact definition, several wildcard families, or none.
//! [`matching`] decides which branches are relevant and how competing branches merge,
//! [`body`] and [`nested`] split a CSS body into its selector, declarations, and
//! conditional rules, [`declarations`] resolves placeholders per declaration,
//! [`formatting`] applies the registry's serialization metadata, and [`resolve`] is the
//! public entry point that ties them together.
//!
//! Every failure is a non-match: an overload that cannot express this candidate
//! is skipped so a later branch or an arbitrary property can still apply.

pub(super) mod body;
pub(super) mod declarations;
pub(super) mod formatting;
pub(super) mod matching;
pub(super) mod nested;
pub(super) mod resolve;

pub use nested::{NestedUtilityAtRule, NestedUtilityRule};
pub use resolve::{
    ResolvedUtility, ResolvedUtilityTarget, resolve_animation_option, resolve_utility,
};
