//! Utility resolution and rule emission.
//!
//! `resolve` turns candidate text into [`CompiledUtility`]
//! values, and `declaration_order` implements the declaration-level ordering rules.
//! `render` turns one utility into selector blocks, and `emission` groups them into a
//! class unit. The split follows the CSS ordering levels: declaration order is decided
//! in `declaration_order`, and block order is decided in `emission`. The two renderers only
//! serialize the result. `policy` checks emitted values and variant targets and filters
//! canonical suggestions.

mod declaration_order;
mod emission;
mod policy;
mod render;
mod resolve;

pub(super) use declaration_order::{
    combine_declarations_for_theme, order_utilities, prepend_missing_declarations,
};
pub(crate) use emission::emit_utility_rules;
pub(crate) use policy::{candidate_allowed, check_candidate_name};
pub(super) use policy::{check, check_variant, spacing_property};
pub(super) use render::render_utility_rule;
pub(crate) use resolve::{
    CompiledUtility, invalid_property_value_finding, resolve_utility_targets, unresolved_lint_kind,
    unresolved_utility_finding,
};
pub(super) use resolve::{UtilitySource, resolve_utility};

#[cfg(test)]
pub(super) use render::render_utility_rule_path;
