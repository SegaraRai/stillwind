//! Detected lint causes and their mapping to named rules.
//!
//! [`LintDiagnostic`] retains the concrete values used in its message template. Source
//! locations and reporting settings are supplied by the caller.
//!
//! `cause` defines [`LintDiagnostic`] and selects its rule, `candidate` formats the
//! optional parts of messages about utility candidates, and `rule_group` names the
//! inputs that a rule group cannot contain.

mod candidate;
mod cause;
mod rule_group;

pub use cause::LintDiagnostic;
pub use rule_group::RuleGroupInputErrorKind;
