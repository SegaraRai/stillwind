//! Utility resolution and rule emission.
//!
//! `resolve` turns candidate text into [`CompiledUtility`](resolve::CompiledUtility)
//! values, `declaration_order` implements the declaration-level ordering rules,
//! `render` turns one utility into selector blocks, and `emission` groups them into a
//! class unit. The split follows the CSS ordering levels: declaration order is decided
//! in `declaration_order`, block order in `emission`, and the two renderers only
//! serialize the result. `policy` checks emitted values and variant targets and filters
//! canonical suggestions.

pub(super) mod declaration_order;
pub(super) mod emission;
pub(crate) mod policy;
pub(super) mod render;
pub(super) mod resolve;
