//! Decisions about Tailwind spellings rather than about evaluation.
//!
//! [`marker_use`] decides whether a candidate adds a marker class, and
//! [`is_tailwind_relationship_candidate`] answers the relationship-pattern query used
//! by ref APIs.

use lean_string::{LeanString, ToLeanString};

use stillwind_diagnostics::MarkerUseErrorKind;
use stillwind_syntax::{Utility, Value, VariantKind, parse_candidate};

use super::super::stillwind::{stillwind_markers, stillwind_relationship_patterns};
use crate::{MarkerForm, ThemeIndex};

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
            .chain(stillwind_relationship_patterns(theme).iter())
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

/// How a candidate uses a marker class.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MarkerUse {
    /// The candidate adds this class to the element as written, including a named
    /// modifier of a modifiable marker.
    Class(LeanString),
    /// The candidate names a marker class in a form that cannot add it.
    Invalid {
        /// Marker class that the candidate names, without an unsupported modifier.
        class: LeanString,
        /// Form of the candidate that prevents the class from being added.
        reason: MarkerUseErrorKind,
    },
}

/// Classifies a candidate that names a marker class of `theme` or of its Stillwind
/// definitions.
///
/// A marker class is added unconditionally, so a candidate adds one only without
/// variants, `!`, and a leading `-`. A named modifier is part of the class when the
/// whole name, such as `foo/bar`, is a marker, or when the name before the modifier is
/// a [`MarkerForm::Modifiable`] marker. Classes stay as written and are never hashed,
/// so selectors in separately compiled CSS can match them.
///
/// Returns `None` when the candidate names no marker class or cannot be parsed. A
/// marker name can also be a utility, so the caller still resolves the candidate as a
/// utility in both returned cases.
#[must_use]
pub fn marker_use(raw: &str, theme: &ThemeIndex) -> Option<MarkerUse> {
    let candidate = parse_candidate(raw)?;
    let Utility::Named { name, modifier } = &candidate.utility else {
        return None;
    };
    let form = |name: &str| {
        theme
            .markers()
            .get(name)
            .into_iter()
            .chain(stillwind_markers(theme).get(name))
            .max()
    };
    let (class, accepted) = match modifier {
        None => (form(name).map(|_| name.to_lean_string())?, true),
        Some(Value::Named(modifier)) => {
            let class = format_args!("{name}/{modifier}").to_lean_string();
            if form(&class).is_some() {
                (class, true)
            } else {
                (class, form(name)? == MarkerForm::Modifiable)
            }
        }
        Some(Value::Arbitrary(_) | Value::CssVariable(_)) => {
            form(name)?;
            (name.to_lean_string(), false)
        }
    };
    let reason = if !candidate.variants.is_empty() {
        MarkerUseErrorKind::Conditional
    } else if candidate.negative {
        MarkerUseErrorKind::Negative
    } else if candidate.important {
        MarkerUseErrorKind::Important
    } else if !accepted {
        MarkerUseErrorKind::Modifier
    } else {
        return Some(MarkerUse::Class(class));
    };
    let class = match modifier {
        Some(Value::Named(_)) if !accepted => name.to_lean_string(),
        _ => class,
    };
    Some(MarkerUse::Invalid { class, reason })
}
