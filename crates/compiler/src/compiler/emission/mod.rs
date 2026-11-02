//! Structure-preserving style emission.
//!
//! [`CallEmitter::run`] resolves style inputs into class units and typed contributions
//! for fragment folding. It keeps guards and source occurrences, and it omits subtrees
//! without a contribution under the macro purity contract. It does not choose final
//! framework keys or allocate runtime helper imports.
//!
//! [`ClassUnits`](super::class_units::ClassUnits) owns class identities and publishes
//! base CSS before conditional CSS.
//! [`LiteralContribution`](super::literal::LiteralContribution) resolves utility text.
//! Fragment rendering later chooses a representation without changing those CSS
//! identities or the declaration order.
//!
//! Modules:
//!
//! - [`asset_record`] defines the property keys shared with asset serialization.
//! - [`call`] owns the walk state, runs the walk, and dispatches each style input. Its
//!   fields are shared only within emission.
//! - [`class_value`] lowers class expressions.
//! - [`keyframes_use`] lowers animation uses.
//! - [`ref_projection`] lowers ref projections.
//! - [`static_properties`] finds literal custom properties that can be emitted outside
//!   runtime guards.
//! - [`style_analysis`] records declarations, markers, and source decisions before
//!   class-unit merging removes the original use boundaries.
//! - [`view_transition`] lowers view-transition uses.

mod asset_record;
mod call;
mod class_value;
mod keyframes_use;
mod ref_projection;
mod static_properties;
mod style_analysis;
mod view_transition;

pub(super) use asset_record::{CLASS_KEY, KEYFRAMES_KEY, VIEW_TRANSITION_CLASS_KEY};
pub(super) use call::{CallEmitter, CallEmitterInputs};
pub(super) use style_analysis::collect_declarations;

use call::PendingRegion;
use static_properties::static_custom_properties;
