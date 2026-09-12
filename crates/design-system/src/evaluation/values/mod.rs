//! Value resolution for utility bodies and variant captures.
//!
//! A registry definition names placeholders such as `--value(...)`, `--modifier(...)`,
//! or `--alpha(...)`; this module decides what they become for one candidate. The work
//! is split by what the value depends on: the theme ([`theme`]), the capture's grammar
//! ([`domains`]), CSS color compatibility ([`colors`]), textual decoding of arbitrary
//! values ([`arbitrary`]), the nested placeholder forms ([`macros`]), and the ordered
//! expansion that applies all of them ([`placeholders`]).
//!
//! Resolution is fail-closed: when a placeholder cannot be resolved the whole
//! declaration is rejected, and the caller then tries the next overload rather than
//! emitting a partially substituted value.

pub(super) mod arbitrary;
pub(super) mod colors;
pub(super) mod domains;
pub(super) mod expansion;
pub(super) mod macros;
pub(super) mod placeholders;
pub(super) mod theme;
