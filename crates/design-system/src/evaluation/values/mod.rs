//! Value resolution for utility bodies and variant captures.
//!
//! A registry definition names placeholders such as `--value(...)`, `--modifier(...)`,
//! or `--alpha(...)`, and this module decides what they become for one candidate.
//! Resolution fails closed: when a placeholder cannot be resolved, the whole
//! declaration is rejected, and the caller tries the next overload instead of emitting
//! a partially substituted value.
//!
//! Modules:
//!
//! - [`arbitrary`] decodes arbitrary-value and CSS-variable captures.
//! - [`colors`] resolves color opacity expressions, compatibility branches, and theme
//!   shadow modifiers.
//! - [`domains`] decides which spellings a `--value(...)` placeholder accepts.
//! - [`expansion`] performs bounded expansion of compiler-owned CSS functions.
//! - [`macros`] evaluates the arguments of the placeholder forms that run before the
//!   main value family.
//! - [`placeholders`] expands every placeholder family of one declaration in order.
//! - [`theme`] looks up theme tokens for candidate values.
//! - [`token_values`] checks authored token values against the types of their namespace
//!   for lint.

mod arbitrary;
mod colors;
mod domains;
mod expansion;
mod macros;
mod placeholders;
mod theme;
mod token_values;

pub use domains::{
    NumericDomain, NumericKind, named_domain_values, placeholder_namespaces,
    unresolvable_value_namespaces,
};
pub use token_values::{ThemeValueMismatch, theme_value_mismatch};

pub(crate) use colors::color_fallback;

pub(super) use domains::{resolve_value_domains, theme_domain_pattern};
pub(super) use expansion::expand_placeholder;
pub(super) use placeholders::{PreparedValues, resolve_css_placeholders};
