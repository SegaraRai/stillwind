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

mod arbitrary;
mod colors;
mod domains;
mod expansion;
mod macros;
mod placeholders;
mod theme;

pub(super) use arbitrary::decode_arbitrary;
pub(super) use colors::CompatColorMode;
pub(super) use domains::{resolve_value_domains, split_domains, theme_domain_pattern};
pub(super) use expansion::expand_placeholder;
pub(super) use placeholders::{PreparedValues, resolve_css_placeholders};
pub(super) use theme::theme_arbitrary_types;
