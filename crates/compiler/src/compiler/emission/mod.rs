//! Structure-preserving style emission.
//!
//! [`CallEmitter::run`] resolves style inputs into class units and typed contributions
//! for fragment folding. It retains guards and source occurrences. Subtrees with no
//! contribution are omitted under the macro purity contract. It does not choose final
//! framework keys or allocate runtime helper imports.
//!
//! [`super::class_units::ClassUnits`] owns class identities and base-before-conditional CSS publication.
//! [`super::literal::LiteralContribution`] resolves utility text,
//! and fragment rendering later chooses a representation without changing those CSS
//! identities or declaration order.
//!
//! `call` owns the walk state and shares its fields only within emission. It runs the
//! walk and dispatches each style input. `class_value` lowers class expressions.
//! `ref_projection`, `keyframes_use`, and `view_transition` hold the arms of one
//! contribution kind each. `style_analysis` records declarations, markers, and source
//! decisions before class-unit merging removes their original use boundaries.
//!
//! `static_properties` finds literals that can be emitted outside runtime guards.
//! `asset_record` defines the property keys shared with asset serialization.

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
