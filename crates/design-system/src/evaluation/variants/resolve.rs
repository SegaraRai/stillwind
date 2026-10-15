//! Public variant resolution.
//!
//! Resolution answers what one variant contributes to a selector: a selector template
//! with a slot for the utility, a condition to wrap it in, declarations that travel
//! with the variant, or a compound of several of those. The answer is data, so the
//! compiler decides how to combine it with the utility and how to place the result in
//! the stylesheet.
//!
//! Project definitions are consulted before built-ins, and a variant that resolves to
//! nothing is not an error: the caller reports the unresolved candidate. Two
//! compatibility helpers live here because they are decisions about Tailwind spellings
//! rather than about evaluation: relational marker classes and the relationship-pattern
//! query used by ref APIs.

use std::sync::Arc;

use cssparser::{Parser, Token};
use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{selector_repeats_subject, selector_uses_subject};
use stillwind_source::ReplaceText;
use stillwind_syntax::{Utility, Value, Variant, VariantKind, parse_candidate};

use super::super::builtins::builtin_variants;
use super::super::builtins::{builtin_marker_utilities, builtin_relationship_patterns};
use super::super::values::decode_arbitrary;
use super::body::resolve_variant_body;
use super::negation::negate;
use super::order::variant_sort_namespace;
use super::substitution::{
    apply_nested_variant, registered_variant_candidates, resolve_variant_css_placeholders,
    substitute_quoted_variant_value, substitute_typed_modifier, substitute_variant_capture,
};
use crate::evaluation::EvaluationContext;
use crate::{BuiltinVariantValueType, CustomVariant, RuntimeCondition, ThemeIndex};

/// A variant's selector, at-rule, declaration, or computed-value effect.
/// Resolution retains runtime conditions as data; rendering chooses their CSS strategy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantApplication {
    /// Tests custom-property presence or equality at computed-value time.
    Conditional(
        /// Canonical [`RuntimeCondition`] text: a presence test or `--name:value`
        /// comparison, optionally prefixed by one `!`.
        /// Refers to the styled element or pseudo-element, never a ref ancestor.
        LeanStr,
    ),
    /// Several applications, one per block of a compound body, in authored order.
    Compound(Vec<VariantApplication>),
    /// Applications composed in order within one branch, retaining nested wrappers.
    Sequence(Vec<VariantApplication>),
    /// A fixed selector suffix appended to the utility's own selector.
    SelectorSuffix(&'static str),
    /// A resolved selector template containing `&` for the utility selector.
    SelectorTemplate(LeanStr),
    /// Adds a compound selector condition to the final element selected by a rule.
    /// The renderer inserts it before that element's pseudo-elements, if present.
    SelectorCondition(LeanStr),
    /// A selector template that also declares properties of its own.
    SelectorRule {
        /// Selector template for the wrapped utility.
        selector: LeanStr,
        /// Declarations the variant contributes.
        declarations: Vec<VariantDeclaration>,
    },
    /// A selector template inside a media condition.
    SelectorMedia {
        /// Selector template for the wrapped utility.
        selector: LeanStr,
        /// Media condition the rule is nested under.
        condition: LeanStr,
    },
    /// A selector template inside a media condition that also declares properties of
    /// its own.
    SelectorMediaRule {
        /// Selector template for the wrapped utility.
        selector: LeanStr,
        /// Declarations the variant contributes.
        declarations: Vec<VariantDeclaration>,
        /// Media condition the rule is nested under.
        condition: LeanStr,
    },
    /// A media condition that wraps the utility without changing its selector.
    Media(LeanStr),
    /// A container condition that wraps the utility without changing its selector.
    Container(LeanStr),
    /// A support condition that wraps the utility without changing its selector.
    Supports(
        /// Complete `@supports` prelude, such as `(display:grid)` or
        /// `not (display:grid)`.
        LeanStr,
    ),
    /// The starting-style at-rule, which the emitter applies around the rule.
    StartingStyle,
}

impl VariantApplication {
    /// Whether resolving this application requires the current element's selector.
    /// External container identities do not themselves require a subject selector.
    pub fn uses_subject(&self) -> bool {
        match self {
            Self::Conditional(_) | Self::SelectorSuffix(_) | Self::SelectorCondition(_) => true,
            Self::Compound(children) | Self::Sequence(children) => {
                children.iter().any(Self::uses_subject)
            }
            Self::SelectorTemplate(selector)
            | Self::SelectorRule { selector, .. }
            | Self::SelectorMedia { selector, .. }
            | Self::SelectorMediaRule { selector, .. } => selector_uses_subject(selector),
            Self::Media(_) | Self::Container(_) | Self::Supports(_) | Self::StartingStyle => false,
        }
    }
}

/// Returns the selector of the first selector-bearing application that does not
/// reference the subject element, if one exists.
///
/// A rule whose selector never mentions its subject is live wherever the stylesheet
/// loads, so a host can reject such input where the rule must stay scoped to the
/// element that produced it. Conditions, runtime conditions, and selectors that keep
/// the subject are not reported.
#[must_use]
pub fn document_scoped_selector(application: &VariantApplication) -> Option<&str> {
    match application {
        VariantApplication::Compound(children) | VariantApplication::Sequence(children) => {
            children.iter().find_map(document_scoped_selector)
        }
        VariantApplication::SelectorTemplate(selector)
        | VariantApplication::SelectorRule { selector, .. }
        | VariantApplication::SelectorMedia { selector, .. }
        | VariantApplication::SelectorMediaRule { selector, .. } => {
            (!selector_uses_subject(selector)).then_some(selector.as_str())
        }
        VariantApplication::Conditional(_)
        | VariantApplication::SelectorCondition(_)
        | VariantApplication::SelectorSuffix(_)
        | VariantApplication::Media(_)
        | VariantApplication::Container(_)
        | VariantApplication::Supports(_)
        | VariantApplication::StartingStyle => None,
    }
}

/// Returns a selector of `variant` that uses `&` more than once in one selector.
///
/// `&` stands for the generated utility class. A selector that repeats it, such as
/// `& + &`, matches elements by the identity of generated classes, which depends on the
/// compiler's class naming rather than on an element relationship. The check covers the
/// selectors of the resolved application and, for a `[variant]` definition such as
/// `group-*` or `not-*`, the selectors of its inner variant. The inner variant is
/// checked first because `not-*` replaces its `&` with `*`. Returns `None` for an
/// unresolved variant.
#[must_use]
pub fn repeated_subject_selector(variant: &Variant, theme: &ThemeIndex) -> Option<LeanString> {
    let (application, inner) = match &variant.kind {
        VariantKind::Arbitrary(_) => (resolve_variant(variant, theme)?, None),
        VariantKind::Named(value) => {
            resolve_named_variant_with(value, theme, |custom, capture, concrete| {
                let application =
                    resolve_registered_variant_candidate(custom, capture, concrete, theme)?;
                let inner = capture
                    .filter(|_| custom.value_type == Some(BuiltinVariantValueType::Variant))
                    .map(|capture| parse_nested_variant(split_typed_capture(custom, capture).0));
                Some((application, inner))
            })?
        }
    };
    inner
        .and_then(|inner| repeated_subject_selector(&inner, theme))
        .or_else(|| repeated_application_selector(&application).map(LeanString::from))
}

/// Returns the first selector of `application` that repeats `&` in one selector.
fn repeated_application_selector(application: &VariantApplication) -> Option<&str> {
    match application {
        VariantApplication::Compound(children) | VariantApplication::Sequence(children) => {
            children.iter().find_map(repeated_application_selector)
        }
        VariantApplication::SelectorTemplate(selector)
        | VariantApplication::SelectorCondition(selector)
        | VariantApplication::SelectorRule { selector, .. }
        | VariantApplication::SelectorMedia { selector, .. }
        | VariantApplication::SelectorMediaRule { selector, .. } => {
            selector_repeats_subject(selector).then_some(selector.as_str())
        }
        VariantApplication::Conditional(_)
        | VariantApplication::SelectorSuffix(_)
        | VariantApplication::Media(_)
        | VariantApplication::Container(_)
        | VariantApplication::Supports(_)
        | VariantApplication::StartingStyle => None,
    }
}

/// One declaration carried by a variant rather than by the utility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantDeclaration {
    /// Property name the variant sets.
    pub property: LeanStr,
    /// Value of that property.
    pub value: LeanStr,
}

/// Resolves one variant against the theme.
///
/// Arbitrary selectors become selector templates; arbitrary at-rules use the
/// same structural body resolver as named definitions. A named variant is matched
/// against project definitions first and built-ins second, and only its first matching
/// candidate is used. A CSS definition with `[if-condition]` and the
/// `builtin-if-variant` intrinsic produces a `Conditional` application; its public
/// spelling comes only from the registered pattern. A `[variant]` definition with the
/// `builtin-not-variant` intrinsic produces the negation of its inner variant. Invalid
/// captures, inner variants without a negated form, and unmatched variants return
/// `None`.
#[must_use]
pub fn resolve_variant(variant: &Variant, theme: &ThemeIndex) -> Option<VariantApplication> {
    if let VariantKind::Arbitrary(value) = &variant.kind {
        return selector_or_at_rule_application(&decode_variant_template(value));
    }
    let VariantKind::Named(value) = &variant.kind else {
        return None;
    };
    resolve_named_variant_with(value, theme, |custom, capture, concrete| {
        resolve_registered_variant_candidate(custom, capture, concrete, theme)
    })
}

/// Resolves the text of an arbitrary variant or a `@custom-variant name (...)`
/// shorthand.
///
/// Text that starts with `@` is an at-rule prelude and resolves like a body that wraps
/// the slot in that at-rule. Other text is a selector template.
fn selector_or_at_rule_application(value: &str) -> Option<VariantApplication> {
    if value.starts_with('@') {
        return resolve_variant_body(&format_args!("{value} {{ @slot; }}").to_lean_string());
    }
    Some(VariantApplication::SelectorTemplate(value.into()))
}

/// Visits definitions in resolution order, including the registry's bare token form.
/// Only a successful evaluation stops the search.
pub(super) fn resolve_named_variant_with<T>(
    value: &str,
    theme: &ThemeIndex,
    mut evaluate: impl FnMut(&CustomVariant, Option<&str>, &str) -> Option<T>,
) -> Option<T> {
    for definitions in [theme.variants(), builtin_variants(theme)] {
        for (custom, capture) in registered_variant_candidates(definitions, value) {
            if let Some(result) = evaluate(custom, capture, value) {
                return Some(result);
            }
        }
    }
    for custom in builtin_variants(theme) {
        let Some(namespace) = custom
            .sort
            .as_ref()
            .filter(|sort| sort.bare)
            .and_then(|sort| sort.key.as_deref())
            .and_then(variant_sort_namespace)
        else {
            continue;
        };
        if theme.token_in(namespace, value).is_none() {
            continue;
        }
        let Some((prefix, suffix)) = custom.wildcard_parts() else {
            continue;
        };
        if let Some(result) = evaluate(
            custom,
            Some(value),
            &format_args!("{prefix}{value}{suffix}").to_lean_string(),
        ) {
            return Some(result);
        }
    }
    None
}

/// Decodes candidate whitespace while preserving CSS escapes used by selector parsing.
fn decode_variant_template(value: &str) -> LeanString {
    let mut output = LeanString::new();
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        match character {
            '_' => output.push(' '),
            '\\' => {
                if let Some(next) = characters.next() {
                    if next != '_' {
                        output.push('\\');
                    }
                    output.push(next);
                } else {
                    output.push('\\');
                }
            }
            character => output.push(character),
        }
    }
    output
}

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

/// Identifies a named variant for runtime-condition diagnostics using CSS
/// registrations. A successfully resolved definition takes precedence, returning true
/// only for a conditional application. Otherwise, a matching `[if-condition]`
/// registration with the known intrinsic returns true even when its nonempty capture is
/// malformed.
pub fn is_condition_variant(value: &str, theme: &ThemeIndex) -> bool {
    if let Some(application) = resolve_registered_variant(value, theme) {
        return matches!(application, VariantApplication::Conditional(_));
    }
    registered_variant_candidates(theme.variants(), value)
        .into_iter()
        .chain(registered_variant_candidates(
            builtin_variants(theme),
            value,
        ))
        .any(|(custom, _)| {
            custom.value_type == Some(BuiltinVariantValueType::IfCondition)
                && has_intrinsic_body(custom, "builtin-if-variant")
        })
}

/// Recognizes a body that consists only of `@stillwind-intrinsic <intrinsic>;`.
fn has_intrinsic_body(custom: &CustomVariant, intrinsic: &str) -> bool {
    let mut parser = Parser::new(&custom.body);
    matches!(parser.next(), Ok(Token::AtKeyword(name)) if name.as_ref() == "stillwind-intrinsic")
        && matches!(parser.next(), Ok(Token::Ident(name)) if name.as_ref() == intrinsic)
        && matches!(parser.next(), Ok(Token::Semicolon))
        && parser.is_exhausted()
}

/// Resolves a named variant through project definitions and then built-ins.
fn resolve_registered_variant(value: &str, theme: &ThemeIndex) -> Option<VariantApplication> {
    if let Some(application) = resolve_registered_variant_from(theme.variants(), value, theme) {
        return Some(application);
    }
    resolve_registered_variant_from(builtin_variants(theme), value, theme)
}

/// Resolves the first definition in `variants` whose name captures `value`.
fn resolve_registered_variant_from(
    variants: &[Arc<CustomVariant>],
    value: &str,
    theme: &ThemeIndex,
) -> Option<VariantApplication> {
    registered_variant_candidates(variants, value)
        .into_iter()
        .find_map(|(custom, capture)| {
            resolve_registered_variant_candidate(custom, capture, value, theme)
        })
}

/// Resolves one definition for a concrete capture.
///
/// A typed definition delegates to [`resolve_typed_variant`]; otherwise the body is
/// filled in and classified, falling back to the definition's shorthand when the body
/// cannot produce an application. A shorthand that starts with `@`, such as
/// `(@media print)`, is an at-rule; any other shorthand is a selector template.
pub(in crate::evaluation) fn resolve_registered_variant_candidate(
    custom: &CustomVariant,
    capture: Option<&str>,
    value: &str,
    theme: &ThemeIndex,
) -> Option<VariantApplication> {
    if let Some(value_type) = custom.value_type {
        return resolve_typed_variant(custom, value_type, capture?, theme);
    }
    let body = substitute_variant_capture(&custom.body, capture)?;
    let body = resolve_variant_css_placeholders(
        &body,
        &custom.name,
        value,
        &EvaluationContext::untracked(theme),
    )?;
    resolve_variant_body(&body).or_else(|| {
        selector_or_at_rule_application(&substitute_variant_capture(&custom.selector, capture)?)
    })
}

/// Resolves a definition that declared a value type, which decides how the capture
/// becomes CSS.
///
/// A variant-stack capture is resolved recursively and applied to the body, or negated
/// when the body is exactly the `builtin-not-variant` intrinsic; aria and data captures
/// become attribute selectors; `nth` and support conditions are validated by CSS itself
/// before substitution. A modifier is serialized through the declared modifier type, so
/// a modifier that is not a valid class identifier fails instead of producing a broken
/// selector. A variant-stack definition without a modifier declaration passes the
/// modifier to its inner variant, as described by [`split_typed_capture`].
/// `[if-condition]` captures bypass modifier splitting and become their canonical
/// condition text, which keeps slashes in comparison values. They require an exact
/// `builtin-if-variant` intrinsic body and no modifier declaration.
fn resolve_typed_variant(
    custom: &CustomVariant,
    value_type: BuiltinVariantValueType,
    capture: &str,
    theme: &ThemeIndex,
) -> Option<VariantApplication> {
    if value_type == BuiltinVariantValueType::IfCondition {
        if custom.modifier_type.is_some() || !has_intrinsic_body(custom, "builtin-if-variant") {
            return None;
        }
        return RuntimeCondition::parse(capture)
            .map(|condition| VariantApplication::Conditional(condition.to_lean_str()));
    }
    let (capture, modifier) = split_typed_capture(custom, capture);
    let body = substitute_typed_modifier(&custom.body, custom.modifier_type, modifier)?;
    match value_type {
        BuiltinVariantValueType::IfCondition => None,
        BuiltinVariantValueType::Variant => {
            let inner = parse_nested_variant(capture);
            let application = resolve_variant(&inner, theme)?;
            if has_intrinsic_body(custom, "builtin-not-variant") {
                return custom
                    .modifier_type
                    .is_none()
                    .then(|| negate(&application))
                    .flatten();
            }
            apply_nested_variant(&body, &application)
        }
        BuiltinVariantValueType::Aria => {
            let value = attribute_variant_selector("aria", capture, true)?;
            resolve_variant_body(&body.replace_text(":--value()", &value))
        }
        BuiltinVariantValueType::Data => {
            let value = attribute_variant_selector("data", capture, false)?;
            resolve_variant_body(&body.replace_text(":--value()", &value))
        }
        BuiltinVariantValueType::Nth | BuiltinVariantValueType::SupportsCondition => {
            let value = decode_variant_capture(capture);
            if !value_type.accepts_css(&value) {
                return None;
            }
            let body = if value_type == BuiltinVariantValueType::Nth {
                substitute_quoted_variant_value(&body, &value)?
            } else {
                body.replace_text("--value()", &value)
            };
            resolve_variant_body(&body)
        }
    }
}

/// Splits a typed definition's capture into its value and the definition's own
/// modifier.
///
/// A `[variant]` definition without a `modifier:` declaration has no modifier of its
/// own, so the whole capture, including `/modifier`, belongs to the inner variant:
/// `not-group-hover/menu` passes `group-hover/menu` to the inner variant. Other
/// definitions split the capture with [`split_variant_modifier`].
pub(in crate::evaluation) fn split_typed_capture<'a>(
    custom: &CustomVariant,
    capture: &'a str,
) -> (&'a str, Option<&'a str>) {
    if custom.value_type == Some(BuiltinVariantValueType::Variant) && custom.modifier_type.is_none()
    {
        (capture, None)
    } else {
        split_variant_modifier(capture)
    }
}

/// Splits a capture at the first top-level `/`, leaving nested brackets and parentheses
/// intact.
///
/// Returns the capture paired with the modifier, which is `None` when no top-level
/// slash exists.
pub(in crate::evaluation) fn split_variant_modifier(value: &str) -> (&str, Option<&str>) {
    let mut depth = 0_u32;
    for (index, character) in value.char_indices() {
        match character {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            '/' if depth == 0 => return (&value[..index], Some(&value[index + 1..])),
            _ => {}
        }
    }
    (value, None)
}

/// Decodes a variant capture into CSS text.
///
/// An arbitrary capture is decoded with escape handling; a named capture only has
/// underscores turned into spaces, because its text is a Tailwind spelling rather than
/// CSS.
fn decode_variant_capture(value: &str) -> LeanString {
    value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .map_or_else(|| value.replace_char('_', " "), decode_arbitrary)
}

/// Builds a synthetic nested variant from capture text, keeping the raw spelling so an
/// inner resolution can report the text the author wrote.
pub(in crate::evaluation) fn parse_nested_variant(value: &str) -> Variant {
    let raw = value.into();
    let kind = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .map_or_else(
            || VariantKind::Named(value.into()),
            |value| VariantKind::Arbitrary(decode_arbitrary(value).into()),
        );
    Variant { kind, raw }
}

/// Builds an attribute selector for an aria or data variant.
///
/// `name=value` becomes an equality selector with the value quoted and escaped; a bare
/// capture becomes `="true"` for aria variants and a presence check for data variants.
/// An empty name or value is rejected rather than emitted.
fn attribute_variant_selector(
    prefix: &str,
    capture: &str,
    default_true: bool,
) -> Option<LeanString> {
    let arbitrary = capture
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'));
    let value = arbitrary.map_or_else(|| capture.to_lean_string(), decode_arbitrary);
    if value.is_empty() {
        return None;
    }
    if let Some((name, expected)) = value.split_once('=') {
        if name.is_empty() || expected.is_empty() {
            return None;
        }
        return Some(
            format_args!(
                "[{prefix}-{name}=\"{}\"]",
                expected.replace_char('"', r#"\""#)
            )
            .to_lean_string(),
        );
    }
    Some(if default_true {
        format_args!("[{prefix}-{value}=\"true\"]").to_lean_string()
    } else {
        format_args!("[{prefix}-{value}]").to_lean_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_and_attribute_captures_preserve_nested_syntax() {
        assert_eq!(
            split_variant_modifier("[x/y]/group"),
            ("[x/y]", Some("group"))
        );
        assert_eq!(split_variant_modifier("[x/y]"), ("[x/y]", None));
        assert_eq!(decode_variant_capture("[a_b]"), "a b");
        assert_eq!(
            attribute_variant_selector("aria", "checked", true),
            Some("[aria-checked=\"true\"]".into())
        );
        assert_eq!(
            attribute_variant_selector("data", "open", false),
            Some("[data-open]".into())
        );
        assert_eq!(
            attribute_variant_selector("data", "[state=on]", false),
            Some("[data-state=\"on\"]".into())
        );
        assert_eq!(attribute_variant_selector("data", "state=", false), None);
        assert!(matches!(
            parse_nested_variant("[&:hover]").kind,
            VariantKind::Arbitrary(_)
        ));
    }
}
