//! Discovery of candidates of utility families that a host plugin defines on demand.
//!
//! A host resolves such candidates outside the compiler and adds their definitions to
//! the design system, so the compiler reports which ones a module needs. Discovery
//! scans the whole source text, as Tailwind's content scanner does: a candidate in
//! a comment or an unrelated string is also reported. Its definition is unused until
//! a module styles with it, so over-reporting costs only resolution work.

use std::collections::BTreeSet;

use lean_string::{LeanStr, LeanString, ToLeanStr};

use stillwind_design_system::{
    DynamicFamilyKind, ThemeIndex, compound_variant_inner, resolve_utility,
};
use stillwind_syntax::{Utility, parse_candidate, parse_variant_prefix};

/// Returns sorted, unique requests for host plugin families found in `text`.
/// Native families report unresolved utility names without modifiers. Tailwind utility
/// families retain negative signs and modifiers even when a built-in already resolves.
/// Variant requests have an `@variant ` prefix followed by the complete variant name.
///
/// A bracket candidate such as `icon-[mdi--close]` is reported by its named spelling,
/// `icon-mdi--close`, when its family accepts both forms, and as written otherwise. A
/// theme without dynamic families returns nothing without scanning.
pub(crate) fn dynamic_candidates(text: &str, theme: &ThemeIndex) -> Vec<LeanStr> {
    let families = theme.dynamic_utilities();
    if families.is_empty() {
        return Vec::new();
    }
    let mut names = BTreeSet::new();
    for token in candidate_tokens(text) {
        if !families
            .iter()
            .any(|family| token.contains(family.prefix.as_str()))
        {
            continue;
        }
        // A style object key such as `"data-[open]:"` carries variants without a utility.
        let (variants, candidate) = match parse_variant_prefix(token) {
            Some(variants) => (variants, None),
            None => match parse_candidate(token) {
                Some(candidate) => (candidate.variants.clone(), Some(candidate)),
                None => continue,
            },
        };
        for variant in &variants {
            // A compound variant such as `group-state-[closed]` requests its inner
            // variant. The registry's variant-stack definitions decide which variants
            // are compound.
            let mut value = LeanString::from(variant.raw.as_str());
            loop {
                if families.iter().any(|family| {
                    family.kind == DynamicFamilyKind::Variant
                        && family_member(&value, &family.prefix)
                }) {
                    names.insert(format_args!("@variant {value}").to_lean_str());
                }
                match compound_variant_inner(&value, theme) {
                    Some(inner) if inner != value => value = inner,
                    _ => break,
                }
            }
        }
        let Some(candidate) = candidate else {
            continue;
        };
        let Utility::Named { name, .. } = &candidate.utility else {
            continue;
        };
        if families.iter().any(|family| {
            family.kind == DynamicFamilyKind::Utility && family_member(name, &family.prefix)
        }) && let Some(key) = candidate.utility.registration_key(candidate.negative)
        {
            names.insert(key.to_lean_str());
        }
        if theme.dynamic_family(name).is_none() {
            continue;
        }
        let name = theme
            .dynamic_named_utility(name)
            .map_or_else(|| name.clone(), Into::into);
        if !matches!(resolve_utility(&name, theme), Ok(Ok(_))) {
            names.insert(name.to_lean_str());
        }
    }
    names.into_iter().collect()
}

/// Whether a candidate selects a registered Tailwind root or one of its values.
fn family_member(candidate: &str, root: &str) -> bool {
    candidate == root
        || candidate
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with('-') || rest.starts_with('/'))
}

/// Splits source text while retaining punctuation and quoted text inside arbitrary values.
fn candidate_tokens(text: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut brackets = 0_u32;
    let mut parentheses = 0_u32;
    let mut escaped = false;
    for (index, character) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if character == '[' {
            brackets += 1;
        }
        if character == ']' {
            brackets = brackets.saturating_sub(1);
        }
        if brackets > 0 {
            continue;
        }
        if character == '('
            && (parentheses > 0
                || text[start..index].ends_with('-')
                || text[start..index].ends_with('/'))
        {
            parentheses += 1;
            continue;
        }
        if character == ')' && parentheses > 0 {
            parentheses -= 1;
            continue;
        }
        if parentheses == 0 && (is_boundary(character) || matches!(character, '(' | ')')) {
            if start < index {
                result.push(&text[start..index]);
            }
            start = index + character.len_utf8();
        }
    }
    if start < text.len() {
        result.push(&text[start..]);
    }
    result
}

/// Whether `character` cannot occur in a candidate outside brackets and therefore
/// separates candidates in source text.
fn is_boundary(character: char) -> bool {
    character.is_whitespace()
        || matches!(
            character,
            '"' | '\'' | '`' | ',' | ';' | '{' | '}' | '<' | '>' | '='
        )
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::parse_theme;
    use stillwind_source::{FileId, SourceFile};

    use super::*;

    /// Parses `css` as a theme.
    fn theme(css: &str) -> ThemeIndex {
        parse_theme(
            &SourceFile::new(FileId(1), None, None, css).unwrap(),
            &"t.css".into(),
        )
        .unwrap()
    }

    #[test]
    fn families_report_only_the_forms_that_they_accept() {
        let text = "icon-mdi--close icon-[lucide--x]";
        assert_eq!(
            dynamic_candidates(text, &theme("@stillwind-dynamic icon-*;")),
            ["icon-mdi--close"]
        );
        assert_eq!(
            dynamic_candidates(text, &theme("@stillwind-dynamic icon-[*];")),
            ["icon-[lucide--x]"]
        );
        assert_eq!(
            dynamic_candidates(
                "icon-[lucide--x] icon-[mdi--home]",
                &theme(r"@stillwind-dynamic icon-[*]; @utility icon-[mdi--home] { color: red; }")
            ),
            ["icon-[lucide--x]"]
        );
    }

    #[test]
    fn unresolved_family_members_are_reported_by_their_named_spelling() {
        let theme = theme(
            "@stillwind-dynamic icon-*; @stillwind-dynamic icon-[*]; @utility icon-mdi--home { color: red; }",
        );
        let text = r#"style("icon-mdi--close hover:icon-[lucide--x] icon-mdi--home", `md:!icon-mdi--close`); // icon-a--b
            const other = "iconography icon- p-4";"#;
        assert_eq!(
            dynamic_candidates(text, &theme),
            ["icon-a--b", "icon-lucide--x", "icon-mdi--close"]
        );
        assert_eq!(
            dynamic_candidates(text, &ThemeIndex::default()),
            [] as [LeanStr; 0]
        );
    }

    #[test]
    fn tailwind_requests_retain_typed_values_modifiers_and_compound_variants() {
        let theme = theme(
            "@stillwind-plugin-family utility probe; @stillwind-plugin-family variant state; @utility probe-4 { width:1px; } @stillwind-variant group-* (value: [variant]) { &:is(:where(.group):--value() *) { @slot; } }",
        );
        assert_eq!(
            dynamic_candidates(
                r#"style("probe-4", "-probe-[length:calc(2px_+_3px)]/[mirror]", { "state-[open]:": "probe-(--width)" }); group-state-[closed]:probe-4"#,
                &theme
            ),
            [
                "-probe-[length:calc(2px_+_3px)]/[mirror]",
                "@variant state-[closed]",
                "@variant state-[open]",
                "probe-(--width)",
                "probe-4"
            ]
        );
    }
}
