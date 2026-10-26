//! Resolves a utility candidate: it finds the definitions that match the candidate
//! name, and turns the body of each matching definition into declarations.
//!
//! A candidate may match one exact definition, several functional families, or nothing.
//! The modules divide the work as follows:
//!
//! - [`matching`] chooses the matching definition branches and merges competing
//!   branches.
//! - [`body`] and [`nested`] split a definition body into its selector, its
//!   declarations, and its conditional rules.
//! - [`declarations`] replaces the placeholders in each declaration.
//! - [`formatting`] writes values with the serialization settings of the registry.
//! - [`resolve`] is the public entry point that runs these steps.
//! - [`diagnosis`] explains why a candidate does not resolve, and [`ranges`] finds the
//!   parts of the candidate that the explanation points at.
//! - [`variables`] checks a `var()` value without a type hint against the known type of
//!   its variable.
//! - [`order`] keeps the Tailwind sort keys of each utility and the order in which
//!   declarations are written. [`tailwind_property_order`] contains the generated
//!   property order that these keys use.
//!
//! When a value does not fit the domain of a definition, that definition does not
//! match, and another definition can still match. When every definition rejects an
//! arbitrary value without a type hint, the value is written to the CSS unchanged and
//! recorded as an [`InvalidPropertyValue`]. The same happens for a variable whose known
//! type the property rejects, and for an opacity modifier outside `0%` to `100%`. A
//! matching definition that is malformed returns an error.

mod body;
mod declarations;
mod diagnosis;
mod formatting;
mod matching;
mod nested;
mod order;
mod ranges;
mod resolve;
mod tailwind_property_order;
mod variables;

pub use body::UtilityResolutionError;
pub use diagnosis::{UtilityFailure, UtilityFailureReason, check_candidate_variants};
pub use matching::dynamic_named_spelling;
pub use nested::{NestedUtilityAtRule, NestedUtilityRule};
pub use order::{UtilityOrder, UtilityPropertySort};
pub use ranges::utility_value_range;
pub use resolve::{
    InvalidPropertyValue, InvalidValueReason, ResolvedUtility, ResolvedUtilityTarget,
    UntypedVariable, UtilityArbitraryUse, UtilityDeclaration, UtilityDiagnosticData,
    UtilityTokenUse, resolve_utility,
};
pub use variables::{VariableMismatch, VariableTypeSource};

pub(super) use body::PreparedUtilityBody;
pub(super) use declarations::PreparedDeclarations;
