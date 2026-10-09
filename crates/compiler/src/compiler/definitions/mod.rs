//! The definition band (`docs/css-ordering.md` L3-1): CSS a declaration owns and that
//! is emitted once per declaration.
//!
//! `keyframes` writes complete conditional keyframe definitions; `view_transition` writes
//! the group rule and the pseudo-element phases of one transition.

mod keyframes;
mod view_transition;

pub(crate) use keyframes::resolve_frame;
pub(crate) use view_transition::emit_view_transition_utilities;

pub(super) use keyframes::emit_keyframes_definition;
pub(super) use view_transition::emit_view_transition_definition;
