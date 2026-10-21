//! Discovery of candidates of utility families that a host plugin defines on demand.
//!
//! A host resolves such candidates outside the compiler and adds their definitions to
//! the design system, so the compiler reports which ones a module needs.
//! [`dynamic_candidates`] scans the whole source text of a module, as Tailwind's
//! content scanner does: a candidate in a comment or an unrelated string is also
//! reported. Its definition is unused until a module styles with it, so
//! over-reporting costs only resolution work. [`stylesheet_dynamic_candidates`]
//! reads only the candidates that CSS applies, because stylesheets also contain
//! definition headers and registry metadata that name utilities without using them.

use std::collections::BTreeSet;

use lean_string::{LeanStr, LeanString, ToLeanStr};

use stillwind_css::{CssNode, CssNodeKind, parse_structure};
use stillwind_design_system::{
    DynamicFamilyKind, ThemeIndex, compound_variant_inner, resolve_utility,
};
use stillwind_syntax::{Utility, parse_candidate, parse_variant_prefix, utility_registration_key};

/// Returns sorted, unique requests for host plugin families found in `text`.
///
/// Utility names that a `@stillwind-undef-utility` statement of the `default` registry
/// context matches are not requested. Native families report unresolved utility names
/// without modifiers. Tailwind utility families retain negative signs and modifiers
/// even when a built-in already resolves. Variant requests have an `@variant ` prefix
/// followed by the complete variant name.
///
/// A bracket candidate such as `icon-[mdi--close]` is reported by its named spelling,
/// `icon-mdi--close`, when its family accepts both forms, and as written otherwise. A
/// theme without dynamic families returns nothing without scanning.
pub(crate) fn dynamic_candidates(text: &str, theme: &ThemeIndex) -> Vec<LeanStr> {
    if theme.dynamic_utilities().is_empty() {
        return Vec::new();
    }
    requests(candidate_tokens(text), theme)
}

/// Returns sorted, unique requests for host plugin families that `css` applies, in the
/// forms that [`dynamic_candidates`] returns.
///
/// The candidates of every `@apply` statement and the variant sequences of every
/// `@variant` rule count, at any nesting depth, including the bodies of definitions.
/// A sequence requests its variants, as a `data-[open]:` style key does in a module.
/// Comments, strings, declarations, and at-rule preludes other than `@variant` are not
/// read. Incomplete CSS reports the uses that its structure contains.
pub(crate) fn stylesheet_dynamic_candidates(css: &str, theme: &ThemeIndex) -> Vec<LeanStr> {
    if theme.dynamic_utilities().is_empty() {
        return Vec::new();
    }
    let mut tokens = Vec::new();
    applied_tokens(css, &parse_structure(css), &mut tokens);
    requests(tokens.iter().map(LeanStr::as_str), theme)
}

/// Appends the candidates and variant prefixes that `nodes` apply to `tokens`.
/// Node ranges must address `css`. A variant sequence gains a trailing `:`, so it
/// parses as variants without a utility.
fn applied_tokens(css: &str, nodes: &[CssNode], tokens: &mut Vec<LeanStr>) {
    for node in nodes {
        match &node.kind {
            CssNodeKind::Apply { candidates } => {
                tokens.extend(
                    candidates
                        .iter()
                        .map(|range| LeanStr::from(&css[range.clone()])),
                );
            }
            CssNodeKind::AtRule {
                name,
                prelude,
                block,
            } => {
                if name == "variant" {
                    let prelude = &css[prelude.clone()];
                    tokens.extend(
                        super::variant_sequences(prelude)
                            .into_iter()
                            .filter(|range| !range.is_empty())
                            .map(|range| format_args!("{}:", &prelude[range]).to_lean_str()),
                    );
                }
                if let Some(block) = block {
                    applied_tokens(css, &block.children, tokens);
                }
            }
            CssNodeKind::StyleRule { block, .. } => applied_tokens(css, &block.children, tokens),
            CssNodeKind::Declaration { .. } => {}
        }
    }
}

/// Returns sorted, unique requests for the family members among `tokens`. The theme
/// must declare at least one dynamic family.
fn requests<'a>(tokens: impl IntoIterator<Item = &'a str>, theme: &ThemeIndex) -> Vec<LeanStr> {
    let families = theme.dynamic_utilities();
    let mut names = BTreeSet::new();
    for token in tokens {
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
        let Utility::Named { name, modifier } = &candidate.utility else {
            continue;
        };
        // The host adds its definitions to the `@plugin` answer, which registers before
        // the other statements, so an undefinition outside the answer removes them again.
        if theme.undefines_utility(name) {
            continue;
        }
        if families.iter().any(|family| {
            family.kind == DynamicFamilyKind::Utility && family_member(name, &family.prefix)
        }) {
            names.insert(
                utility_registration_key(name, modifier.as_ref(), candidate.negative).to_lean_str(),
            );
        }
        if theme.dynamic_family(name).is_none() || matches!(resolve_utility(name, theme), Ok(Ok(_)))
        {
            continue;
        }
        names.insert(
            theme
                .dynamic_named_utility(name)
                .map_or_else(|| name.to_lean_str(), |named| named.to_lean_str()),
        );
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
    fn requests_depend_only_on_default_definitions() {
        let text = "icon-mdi--close icon-lucide--x";
        let undefined = theme("@stillwind-dynamic icon-*; @stillwind-undef-utility icon-mdi-*;");
        assert_eq!(dynamic_candidates(text, &undefined), ["icon-lucide--x"]);
        // `default` keeps the definitions that the host adds when only a context
        // undefines the family, so the context does not suppress the request.
        let scoped = theme(
            "@stillwind-dynamic icon-*; @stillwind-registry keyframe { @stillwind-undef-utility icon-*; }",
        );
        assert_eq!(
            dynamic_candidates(text, &scoped),
            ["icon-lucide--x", "icon-mdi--close"]
        );
        let defined = theme(
            "@stillwind-dynamic icon-*; @stillwind-registry keyframe { @stillwind-undef-utility icon-*; } @utility icon-mdi--close { color: red; }",
        );
        assert_eq!(dynamic_candidates(text, &defined), ["icon-lucide--x"]);
    }

    #[test]
    fn bracket_values_that_another_definition_accepts_are_not_reported() {
        let theme = theme(
            "@stillwind-dynamic icon-*; @stillwind-dynamic icon-[*]; @utility icon-* { --size: --value([length]); }",
        );
        assert_eq!(
            dynamic_candidates("icon-[2rem] icon-[mdi--close]", &theme),
            ["icon-mdi--close"]
        );
        let bracket = self::theme(
            "@stillwind-dynamic icon-[*]; @utility icon-* { --size: --value([length]); }",
        );
        assert_eq!(
            dynamic_candidates("icon-[2rem] icon-[mdi--close]", &bracket),
            ["icon-[mdi--close]"]
        );
    }

    #[test]
    fn stylesheets_report_only_applied_candidates_and_variant_sequences() {
        let theme = theme(
            "@stillwind-plugin-family utility probe; @stillwind-plugin-family variant state; @stillwind-dynamic icon-*;",
        );
        let css = r#"
/* @apply probe-comment; */
@stillwind-utility probe-* { width: --value(integer)px; }
.a { content: "@apply probe-string"; @apply probe-4 hover:icon-mdi--close bg-[url('a;b')]; }
@media print { .b { @variant state-[open], dark:state-[closed] { color: red; } } }
@custom-variant x { @variant state-[x] { @slot; } }
.c { @apply probe-[1px] "#;
        assert_eq!(
            stylesheet_dynamic_candidates(css, &theme),
            [
                "@variant state-[closed]",
                "@variant state-[open]",
                "@variant state-[x]",
                "icon-mdi--close",
                "probe-4",
                "probe-[1px]",
            ]
        );
        assert_eq!(
            stylesheet_dynamic_candidates(css, &ThemeIndex::default()),
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
                r#"style("probe-4", "-probe-[length:calc(2px_+_3px)]/[mirror]", { "state-[open]:": "probe-(--width)" }); group-state-[closed]:probe-4 probe-one/[foo_bar]"#,
                &theme
            ),
            [
                "-probe-[length:calc(2px_+_3px)]/[mirror]",
                "@variant state-[closed]",
                "@variant state-[open]",
                "probe-(--width)",
                "probe-4",
                // The modifier is encoded again, so the key contains no whitespace.
                "probe-one/[foo_bar]"
            ]
        );
    }
}
