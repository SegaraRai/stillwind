//! `ignored-tailwind-feature`: Tailwind's `@config` and the import modifiers that
//! change how Tailwind interprets an imported file.
//!
//! The loader removes `@config` from the expansion and loads an import without these
//! modifiers, so the stylesheet compiles while the feature has no effect. The finding
//! covers the statement or the modifier and names the project setting that replaces it
//! where one exists.

use stillwind_design_system::{IgnoredTailwindFeatureKind, ignored_tailwind_features};
use stillwind_diagnostics::{LintDiagnostic, LintFinding, LintKind};

use super::analysis::Checker;

/// Reports every ignored Tailwind feature of the checked source.
pub(super) fn check_tailwind_features(checker: &mut Checker<'_>, text: &str) {
    if !checker.enabled(LintKind::IgnoredTailwindFeature) {
        return;
    }
    for feature in ignored_tailwind_features(text) {
        let (name, instead) = match &feature.kind {
            IgnoredTailwindFeatureKind::Config => (
                "@config",
                "set the project's `tailwind` field in stillwind.config.ts instead",
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
                    "prefix" => "Stillwind generates unique class names without a prefix",
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

#[cfg(test)]
mod tests {
    use stillwind_diagnostics::LintKind;

    use super::super::tests::{default_findings, finding};

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
