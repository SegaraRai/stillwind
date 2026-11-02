//! Detected lint causes and their mapping to named rules.
//!
//! [`LintDiagnostic`] keeps the concrete values that its message template uses. The
//! caller supplies source locations and reporting settings.
//!
//! Modules:
//!
//! - [`candidate`] formats the optional parts of messages about utility candidates.
//! - [`cause`] defines [`LintDiagnostic`] and selects its rule.
//! - [`marker`] names the candidate forms that cannot add a marker class.
//! - [`rule_group`] names the inputs that a rule group cannot contain.
//! - [`variables`] formats messages about variables of known types.
//! - [`variants`] formats messages about variant stacks and about CSS that a variant
//!   can replace.

mod candidate;
mod cause;
mod marker;
mod rule_group;
mod variables;
mod variants;

pub use cause::LintDiagnostic;
pub use marker::MarkerUseErrorKind;
pub use rule_group::RuleGroupInputErrorKind;
