//! Discovery of candidates of utility families that a host plugin defines on demand.
//!
//! A host resolves such candidates outside the compiler and adds their definitions to
//! the design system, so the compiler reports which ones a module needs. Discovery
//! scans the whole source text, as Tailwind's content scanner does: a candidate in
//! a comment or an unrelated string is also reported. Its definition is unused until
//! a module styles with it, so over-reporting costs only resolution work.

use std::collections::BTreeSet;

use lean_string::{LeanStr, ToLeanStr};

use stillwind_design_system::{ThemeIndex, resolve_utility};
use stillwind_syntax::{Utility, parse_candidate};

/// Returns the utility names, without variants or modifiers, of candidates in `text`
/// that belong to a dynamic family of `theme` and that do not resolve yet, sorted and
/// deduplicated.
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
    for token in text.split(is_boundary) {
        if !families
            .iter()
            .any(|family| token.contains(family.prefix.as_str()))
        {
            continue;
        }
        let Some(candidate) = parse_candidate(token) else {
            continue;
        };
        let Utility::Named { name, .. } = &candidate.utility else {
            continue;
        };
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
}
