//! The definition band (`docs/css-ordering.md` L3-1): CSS a declaration owns and that is
//! emitted once per declaration.
//!
//! `animation` writes complete conditional keyframe definitions; `view_transition` writes the
//! group rule and the pseudo-element phases of one transition.

mod animation;
mod view_transition;

pub(super) use animation::emit_animation_definition;
pub(super) use view_transition::emit_view_transition_definition;

use stillwind_semantic::AssetKind;

/// Whether the CSS of one asset kind belongs to this band.
pub(super) fn is_definition_kind(kind: AssetKind) -> bool {
    matches!(kind, AssetKind::Animation | AssetKind::ViewTransition)
}
