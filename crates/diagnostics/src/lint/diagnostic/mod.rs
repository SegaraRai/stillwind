//! Detected lint causes and their mapping to named rules.
//!
//! [`LintDiagnostic`] retains the concrete values used in its message template. Source
//! locations and reporting settings are supplied by the caller.
//!
//! [`cause`] defines [`LintDiagnostic`] and selects its rule, [`candidate`] formats the
//! optional parts of messages about utility candidates, [`variables`] formats messages
//! about variables of known types, [`variants`] formats messages about variant stacks
//! and about CSS that a variant can replace, [`rule_group`] names the inputs that a
//! rule group cannot contain, and [`marker`] names the candidate forms that cannot add
//! a marker class.

mod candidate;
mod cause;
mod marker;
mod rule_group;
mod variables;
mod variants;

pub use cause::LintDiagnostic;
pub use marker::MarkerUseErrorKind;
pub use rule_group::RuleGroupInputErrorKind;
