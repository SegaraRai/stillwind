//! Decisions about Tailwind spellings rather than about evaluation.
//!
//! [`compatibility_marker_class`] keeps the marker classes of Tailwind relationship
//! utilities, and [`is_tailwind_relationship_candidate`] answers the
//! relationship-pattern query used by ref APIs.

use lean_string::{LeanString, ToLeanString};

use stillwind_syntax::{Utility, Value, VariantKind, parse_candidate};

use super::super::builtins::{builtin_marker_utilities, builtin_relationship_patterns};
use crate::ThemeIndex;

/// Whether a Tailwind candidate uses a relationship primitive for which projects may
/// prefer Stillwind's typed ref API.
///
/// The answer is about the spelling, not about resolution: a candidate matches when its
/// utility name or one of its variants matches an authored relationship pattern, or
/// when it uses Stillwind's `@`-prefixed ref syntax. Projects use this to report a
/// compatibility path rather than to change emitted CSS.
#[must_use]
pub fn is_tailwind_relationship_candidate(raw: &str, theme: &ThemeIndex) -> bool {
    let Some(candidate) = parse_candidate(raw) else {
        return false;
    };

    let matches_pattern = |value: &str| {
        theme
            .relationship_patterns()
            .iter()
            .chain(builtin_relationship_patterns(theme).iter())
            .any(|pattern| {
                pattern
                    .strip_suffix('*')
                    .map_or(value == pattern, |prefix| value.starts_with(prefix))
            })
    };
    matches!(
        &candidate.utility,
        Utility::Named { name, .. } if matches_pattern(name)
        || name.starts_with('@'))
        || raw.starts_with('@')
        || candidate.variants.iter().any(|variant| {
            matches!(&variant.kind, VariantKind::Named(name) if matches_pattern(name))
                || variant.raw.starts_with('@')
        })
}

/// Returns the literal marker class carried by Tailwind's declaration-only `group` and
/// `peer` utilities. These classes intentionally bypass hashing so relational selectors
/// can find them across independent `style()` calls.
///
/// Only a non-important, unnegated, variant-free candidate can produce a marker, and
/// only when the registry lists the name as a marker utility. The returned class is
/// emitted as written, including any named modifier, so two independent calls agree on
/// the class they must match.
#[must_use]
pub fn compatibility_marker_class(raw: &str, theme: &ThemeIndex) -> Option<LeanString> {
    let candidate = parse_candidate(raw)?;
    if candidate.negative || candidate.important || !candidate.variants.is_empty() {
        return None;
    }
    let Utility::Named { name, modifier } = candidate.utility else {
        return None;
    };
    if !theme.marker_utilities().contains(name.as_str())
        && !builtin_marker_utilities(theme).contains(name.as_str())
    {
        return None;
    }
    match modifier {
        None => Some(name.to_lean_string()),
        Some(Value::Named(modifier)) => Some(format_args!("{name}/{modifier}").to_lean_string()),
        Some(Value::Arbitrary(_) | Value::CssVariable(_)) => None,
    }
}
