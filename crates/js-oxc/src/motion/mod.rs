//! Animation and view-transition extraction.
//!
//! Both are declaration-site assets that `style()` consumes through a call: an
//! animation takes an options string, a view transition optionally takes a stable key.
//! Declarations are validated here (literal frames, literal options, no computed keys)
//! so the compiler never has to guess at a dynamic form; a value that cannot be
//! resolved locally stays unresolved and is reported.
//!
//! Uses are extracted as separate variants for the static and finite-dynamic cases,
//! because they lower to different runtime shapes.

mod animation;
mod view_transition;
