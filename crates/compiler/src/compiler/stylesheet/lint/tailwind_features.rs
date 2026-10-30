//! `ignored-tailwind-feature`: Tailwind's `@config`, `@source`, and `@reference`
//! statements, the import modifiers that change how Tailwind interprets an imported
//! file, and the v3 path form of `theme()`.
//!
//! The loader removes the statements from the expansion and loads an import without
//! these modifiers, so the stylesheet compiles while the feature has no effect. A
//! `theme()` call with a path stays in the output, where browsers reject the
//! declaration. The finding covers the statement, the modifier, or the call and names
//! the replacement where one exists.

use std::ops::Range;

use lean_string::ToLeanStr;

use stillwind_css::{CssNode, CssNodeKind, find_css_functions};
use stillwind_design_system::{IgnoredTailwindFeatureKind, ignored_tailwind_features};
use stillwind_diagnostics::{LintDiagnostic, LintFinding, LintKind};

use super::analysis::Checker;

/// Replacement that a finding names for Tailwind's `prefix()`.
const THEME_PREFIX_REPLACEMENT: &str = "Stillwind generates unique class names without a prefix; set the project's `themePrefix` field in stillwind.config.ts to prefix theme variables";

/// Reports every ignored Tailwind feature of the checked source.
pub(super) fn check_tailwind_features(checker: &mut Checker<'_>, text: &str) {
    if !checker.enabled(LintKind::IgnoredTailwindFeature) {
        return;
    }
    for feature in ignored_tailwind_features(text) {
        let (name, instead) = match &feature.kind {
            IgnoredTailwindFeatureKind::ThemePrefix => ("prefix", THEME_PREFIX_REPLACEMENT),
            IgnoredTailwindFeatureKind::Config => (
                "@config",
                "set the project's `tailwind` field in stillwind.config.ts instead",
            ),
            IgnoredTailwindFeatureKind::Source => (
                "@source",
                "only `style()` calls generate utilities, so move each utility into a call",
            ),
            IgnoredTailwindFeatureKind::Reference => (
                "@reference",
                "import the stylesheet from the project's stylesheet to use its definitions",
            ),
            IgnoredTailwindFeatureKind::ImportModifier(name) => (
                name.as_str(),
                match name.as_str() {
                    "important" => {
                        "set the project's `important` field in stillwind.config.ts instead"
                    }
                    "source" => {
                        "Stillwind collects utilities from the modules that the project's `files` field selects"
                    }
                    "prefix" => THEME_PREFIX_REPLACEMENT,
                    _ => "the file is imported without this modifier",
                },
            ),
        };
        let span = checker.span(feature.range);
        checker.report(LintFinding::new(
            LintDiagnostic::IgnoredTailwindFeature {
                feature: name.into(),
                instead: instead.into(),
            },
            span,
        ));
    }
}

/// Reports each `theme()` call in `nodes` and their descendants that reads a v3 path.
pub(super) fn check_theme_paths(checker: &mut Checker<'_>, nodes: &[CssNode]) {
    if !checker.enabled(LintKind::IgnoredTailwindFeature) {
        return;
    }
    for node in nodes {
        match &node.kind {
            CssNodeKind::StyleRule { selector, block } => {
                check_theme_path_text(checker, selector.clone());
                check_theme_paths(checker, &block.children);
            }
            CssNodeKind::AtRule { prelude, block, .. } => {
                check_theme_path_text(checker, prelude.clone());
                if let Some(block) = block {
                    check_theme_paths(checker, &block.children);
                }
            }
            CssNodeKind::Declaration { value, .. } => {
                check_theme_path_text(checker, value.clone());
            }
        }
    }
}

/// Reports each `theme()` call in the source text at `range` whose argument is not a
/// custom property.
fn check_theme_path_text(checker: &mut Checker<'_>, range: Range<usize>) {
    let text = &checker.source.text()[range.clone()];
    for found in find_css_functions(text, &["theme"], 0) {
        let Some(end) = found.end else {
            continue;
        };
        if text[found.arguments].trim().starts_with("--") {
            continue;
        }
        let span = checker.span(range.start + found.start..range.start + end);
        checker.report(LintFinding::new(
            LintDiagnostic::IgnoredTailwindFeature {
                feature: text[found.start..end].to_lean_str(),
                instead:
                    "read a theme variable with `--theme()`, such as `--theme(--color-red-500)`"
                        .into(),
            },
            span,
        ));
    }
}

#[cfg(test)]
mod tests {
    use stillwind_diagnostics::LintKind;

    use super::super::tests::{default_findings, finding};

    #[test]
    fn source_and_reference_statements_are_reported() {
        assert_eq!(
            default_findings(".a { color: red; } @source './x'; @reference './y.css';"),
            [
                finding(LintKind::IgnoredTailwindFeature, "@source './x';"),
                finding(LintKind::IgnoredTailwindFeature, "@reference './y.css';"),
            ]
        );
    }

    #[test]
    fn theme_paths_are_reported_and_custom_properties_are_not() {
        assert_eq!(
            default_findings(
                ".a { color: theme(colors.red.500); fill: theme(--color-red-500); }\
                 @utility u { color: theme(colors.blue.500 / 50%); }"
            ),
            [
                finding(LintKind::IgnoredTailwindFeature, "theme(colors.red.500)"),
                finding(
                    LintKind::IgnoredTailwindFeature,
                    "theme(colors.blue.500 / 50%)"
                ),
            ]
        );
    }

    #[test]
    fn config_statements_are_reported() {
        assert_eq!(
            default_findings(".a { color: red; } @config './tailwind.config.js';"),
            [finding(
                LintKind::IgnoredTailwindFeature,
                "@config './tailwind.config.js';"
            )]
        );
    }
}
