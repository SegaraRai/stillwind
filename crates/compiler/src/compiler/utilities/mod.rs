//! Utility resolution and rule emission.
//!
//! [`resolve`] turns candidate text into [`CompiledUtility`] values, and
//! [`declaration_order`] implements the declaration-level ordering rules. [`render`]
//! turns one utility into selector blocks, and [`emission`] groups them into a class
//! unit. [`policy`] checks emitted values and variant targets and filters canonical
//! suggestions. [`clamping`] finds literal values that browsers clamp.
//!
//! The split follows the CSS ordering levels: declaration order is decided in
//! [`declaration_order`], and block order is decided in [`emission`]. The two renderers
//! only serialize the result.

mod clamping;
mod declaration_order;
mod emission;
mod policy;
mod render;
mod resolve;

pub(crate) use emission::{FLATTEN_COLOR_COMPATIBILITY, emit_utility_rules};
pub(crate) use policy::{candidate_allowed, check_candidate_name};
pub(crate) use resolve::{
    CompiledUtility, invalid_property_value_finding, resolve_utility_targets, unresolved_lint_kind,
    unresolved_utility_finding, with_type_hint, written_span,
};

pub(super) use clamping::clamped_value;
pub(super) use declaration_order::{
    combine_declarations_for_theme, order_utilities, prepend_missing_declarations,
};
pub(super) use policy::{check, check_variant, spacing_property};
pub(super) use render::render_utility_rule;
pub(super) use resolve::{CompiledUtilityDiagnosticData, resolve_first_target};

#[cfg(test)]
pub(super) use render::render_utility_rule_path;
