//! Arbitrary `[...]` variant text and the inner variants built from captures.

use lean_string::{LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{selector_is_subject_condition, selector_is_valid, selector_uses_subject};
use stillwind_syntax::{Variant, VariantKind};

use super::super::super::values::decode_arbitrary;
use super::super::application::VariantApplication;
use super::super::body::resolve_variant_body;
use super::VariantFailureReason;

/// Resolves the decoded text of an arbitrary variant.
///
/// A selector without `&` tests the styled element, as in Tailwind: `[:hover]` becomes
/// `&:is(:hover)`. The selector is kept as written when `&:is(...)` would be invalid or
/// could never match the styled element, such as `[::selection]` or `[:root]`. Such a
/// template does not refer to the subject, and [`document_scoped_selector`] reports it.
/// Text that starts with `@` is an at-rule prelude and resolves like a body that wraps
/// the slot in that at-rule; an at-rule that the body resolver does not accept fails
/// with [`VariantFailureReason::InvalidAtRule`]. A selector that contains `&` is kept
/// as written. Text that is not a valid selector list, such as `color:` or `&:`, fails
/// with [`VariantFailureReason::InvalidSelector`].
///
/// A custom variant shorthand resolves differently, as a nested style rule; see
/// [`shorthand_variant_application`].
pub(super) fn arbitrary_variant_application(
    value: &str,
) -> Result<VariantApplication, VariantFailureReason> {
    if value.starts_with('@') {
        return resolve_variant_body(&format_args!("{value} {{ @slot; }}").to_lean_string())
            .ok_or_else(|| VariantFailureReason::InvalidAtRule {
                rule: value.to_lean_str(),
            });
    }
    if !selector_is_valid(value) {
        return Err(VariantFailureReason::InvalidSelector {
            selector: value.to_lean_str(),
        });
    }
    if !selector_uses_subject(value) && selector_is_subject_condition(value) {
        return Ok(VariantApplication::SelectorTemplate(
            format_args!("&:is({value})").to_lean_str(),
        ));
    }
    Ok(VariantApplication::SelectorTemplate(value.into()))
}

/// Resolves the text of a `@custom-variant name (...)` shorthand.
///
/// The shorthand resolves like the body `<value> { @slot; }`. Text that starts with `@`
/// is an at-rule prelude. Other text is a nested style rule selector, so a branch
/// without `&` styles descendants, as in CSS Nesting and Tailwind: `(:hover)` becomes
/// `& :hover`.
pub(super) fn shorthand_variant_application(value: &str) -> Option<VariantApplication> {
    resolve_variant_body(&format_args!("{value} {{ @slot; }}").to_lean_string())
}

/// Decodes candidate whitespace while preserving CSS escapes used by selector parsing.
pub(super) fn decode_variant_template(value: &str) -> LeanString {
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

/// Returns the selector of an arbitrary variant that starts with a combinator, such as
/// `>img` or `+ .item`.
///
/// Such a selector is relative to an anchor element that the variant does not name.
pub(in crate::evaluation) fn relative_selector(variant: &Variant) -> Option<&str> {
    match &variant.kind {
        VariantKind::Arbitrary(value) if is_relative_selector(value) => Some(value.trim()),
        _ => None,
    }
}

/// True when `value` starts with the combinator of a relative selector.
pub(super) fn is_relative_selector(value: &str) -> bool {
    value.trim_start().starts_with(['>', '+', '~'])
}

/// Builds a synthetic nested variant from capture text, keeping the raw spelling so an
/// inner resolution can report the text the author wrote.
///
/// An arbitrary selector without `&` tests the element that the outer variant names, as
/// in Tailwind: `group-[.open]` receives `&:is(.open)`. At-rules and relative selectors
/// such as `>img` are kept as written. Text that is not a valid selector list is also
/// kept as written, so that its resolution fails; `:is()` would otherwise accept it as
/// a forgiving selector list that matches nothing.
pub(in crate::evaluation) fn parse_nested_variant(value: &str) -> Variant {
    let raw = value.into();
    let kind = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .map_or_else(
            || VariantKind::Named(value.into()),
            |value| {
                let value = decode_arbitrary(value);
                if value.starts_with('@')
                    || value.contains('&')
                    || is_relative_selector(&value)
                    || !selector_is_valid(&value)
                {
                    VariantKind::Arbitrary(value.into())
                } else {
                    VariantKind::Arbitrary(format_args!("&:is({value})").to_lean_str())
                }
            },
        );
    Variant { kind, raw }
}

#[cfg(test)]
mod tests {
    use lean_string::ToLeanStr;

    use super::*;

    #[test]
    fn arbitrary_selectors_without_subject_test_the_styled_element_when_they_can() {
        for (value, expected) in [
            (":hover", "&:is(:hover)"),
            (":nth-child(3)", "&:is(:nth-child(3))"),
            (".is-open", "&:is(.is-open)"),
            ("[data-x]", "&:is([data-x])"),
            (":hover,:focus", "&:is(:hover,:focus)"),
            (".dark .item", "&:is(.dark .item)"),
            ("&:hover", "&:hover"),
            ("::selection", "::selection"),
            ("::view-transition-old(*)", "::view-transition-old(*)"),
            (":root", ":root"),
            (":host", ":host"),
            (":hover, :root", ":hover, :root"),
        ] {
            assert_eq!(
                arbitrary_variant_application(value).ok(),
                Some(VariantApplication::SelectorTemplate(expected.to_lean_str())),
                "{value}"
            );
        }
        assert_eq!(
            arbitrary_variant_application("@media print").ok(),
            Some(VariantApplication::Media("print".to_lean_str()))
        );
        for value in ["--x:", "color:", "color:red;", "&:", "&;", ":hover,"] {
            assert_eq!(arbitrary_variant_application(value).ok(), None, "{value}");
            assert_eq!(shorthand_variant_application(value), None, "{value}");
        }
        assert_eq!(
            shorthand_variant_application(":hover"),
            Some(VariantApplication::SelectorTemplate(
                "& :hover".to_lean_str()
            ))
        );
    }
}
