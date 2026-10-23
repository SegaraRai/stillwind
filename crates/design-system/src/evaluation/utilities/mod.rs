//! Utility resolution: matching a candidate name to definition branches and turning the
//! matched body into declarations.
//!
//! A candidate may match an exact definition, several wildcard families, or none.
//! [`matching`] decides which branches are relevant and how competing branches merge,
//! [`body`] and [`nested`] split a CSS body into its selector, declarations, and
//! conditional rules, [`declarations`] resolves placeholders per declaration,
//! [`formatting`] applies the registry's serialization metadata, and [`resolve`] is the
//! public entry point that ties them together. [`diagnosis`] explains a candidate that
//! resolution rejects, and [`variables`] checks a `var()` value without a type hint
//! against the known type of its variable. [`order`] retains pre-emission Tailwind
//! property/count keys and authored precedence. [`tailwind_property_order`] supplies
//! its generated property indices.
//!
//! Value-domain mismatches are non-matches, allowing another overload to apply. When
//! every definition rejects an arbitrary value without a type hint, the value is
//! emitted as written and recorded as an [`InvalidPropertyValue`], as are a variable of
//! a known type that the property rejects and an opacity modifier outside `0%` to
//! `100%`. Malformed matched
//! definitions return a typed error.

mod body;
mod declarations;
mod diagnosis;
mod formatting;
mod matching;
mod nested;
mod order;
mod resolve;
mod tailwind_property_order;
mod variables;

pub use body::UtilityResolutionError;
pub use diagnosis::{
    UtilityFailure, UtilityFailureReason, check_candidate_variants, utility_value_range,
};
pub use matching::dynamic_named_spelling;
pub use nested::{NestedUtilityAtRule, NestedUtilityRule};
pub use order::{UtilityOrder, UtilityPropertySort};
pub use resolve::{
    InvalidPropertyValue, InvalidValueReason, ResolvedUtility, ResolvedUtilityTarget,
    UntypedVariable, UtilityArbitraryUse, UtilityDeclaration, UtilityDiagnosticData,
    UtilityTokenUse, resolve_utility,
};
pub use variables::{VariableMismatch, VariableTypeSource};

pub(super) use body::PreparedUtilityBody;
pub(super) use declarations::PreparedDeclarations;
