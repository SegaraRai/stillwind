//! Resolution of option lists and the conditions their options carry.
//!
//! An `animation(options)` use and a view-transition group option list share this
//! resolver; they differ only in whether a variant on an option is rejected or kept as
//! a condition. Options resolve through the animation option registry, which is why a
//! theme duration is usable as an option even though it is not a utility.

use lean_string::{LeanStr, LeanString};

use stillwind_design_system::{ThemeIndex, resolve_animation_option};
use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintDiagnostic, LintFinding};
use stillwind_source::Spanned;
use stillwind_syntax::{Variant, parse_candidate};

use super::lexer::lex_utilities;
use super::variants::{OptionCondition, condition_paths};

/// How an option list treats the variants an option carries.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum OptionVariants {
    /// A variant makes the option unknown: an animation option describes a value.
    Rejected,
    /// Media and supports conditions wrap the option's own rule.
    Conditions,
}

/// Declarations one option list contributes, split by the conditions its options carry.
#[derive(Default)]
pub(crate) struct ResolvedOptions {
    /// Declarations of the options that apply unconditionally, in written order.
    pub(crate) declaration: LeanString,
    /// One entry per conditioned option, in written order. Always empty under
    /// [`OptionVariants::Rejected`].
    pub(crate) conditioned: Vec<ConditionedOption>,
}

/// Declarations one option contributes under the conditions it carries.
pub(crate) struct ConditionedOption {
    /// Alternative paths for this option, each with conditions ordered outermost first.
    pub(crate) paths: Vec<Vec<OptionCondition>>,
    /// Declarations the option contributes.
    pub(crate) declaration: LeanString,
}

/// Resolves a whitespace-separated option list, keeping the conditions its options
/// carry.
///
/// Each option contributes its own declarations, so writing two durations is not
/// rejected here but resolves in source order. An option that cannot apply - text that
/// is not candidate syntax, a negative value, `!important`, or an unknown name - is
/// reported as an unknown option at its own span and contributes nothing. A variant
/// that cannot condition a rule is reported as an unknown option under
/// [`OptionVariants::Rejected`] and as an invalid view-transition variant under
/// [`OptionVariants::Conditions`].
pub(crate) fn resolve_options(
    options: &Spanned<LeanStr>,
    variants: OptionVariants,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
    lint_findings: &mut Vec<LintFinding>,
) -> ResolvedOptions {
    let mut resolved = ResolvedOptions::default();
    for option in lex_utilities(options, diagnostics) {
        let raw = option.value.raw.clone();
        let unknown = || {
            LintFinding::new(
                LintDiagnostic::UnknownAnimationOption {
                    option: (raw.clone()),
                },
                option.span,
            )
        };
        let Some(candidate) = parse_candidate(&raw) else {
            lint_findings.push(unknown());
            continue;
        };
        if candidate.important || candidate.negative {
            lint_findings.push(unknown());
            continue;
        }
        let declaration =
            match resolve_animation_option(strip_variants(&raw, &candidate.variants), theme) {
                Ok(Some(declaration)) => declaration,
                Ok(None) => {
                    lint_findings.push(unknown());
                    continue;
                }
                Err(error) => {
                    diagnostics.push(CompileError::new(
                        CompileDiagnostic::MalformedUtilityRequirement {
                            utility: error.name,
                            offset: error.source.offset,
                        },
                        option.span,
                    ));
                    continue;
                }
            };
        if candidate.variants.is_empty() {
            resolved.declaration.push_str(&declaration);
            continue;
        }
        if variants == OptionVariants::Rejected {
            lint_findings.push(unknown());
            continue;
        }
        let Some(paths) = condition_paths(&candidate.variants, theme) else {
            lint_findings.push(LintFinding::new(
                LintDiagnostic::InvalidViewTransitionVariant,
                option.span,
            ));
            continue;
        };
        resolved.conditioned.push(ConditionedOption {
            paths,
            declaration: (declaration),
        });
    }
    resolved
}

/// The option text without its variant prefixes.
///
/// A parsed candidate keeps each prefix's own spelling, so dropping them in order
/// leaves the text the option registries resolve.
fn strip_variants<'a>(raw: &'a str, variants: &[Variant]) -> &'a str {
    let prefixes: usize = variants.iter().map(|variant| variant.raw.len() + 1).sum();
    &raw[prefixes..]
}

#[cfg(test)]
mod tests {
    use lean_string::ToLeanStr;

    use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
    use stillwind_js_oxc::OxcSourceAnalyzer;
    use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile, Span};
    use stillwind_test_support::css_source;

    use super::super::variants::wrap_conditions;
    use super::*;
    use crate::{Compiler, CompilerOptions};

    #[test]
    fn resolves_themed_duration_as_an_animation_option_only() {
        let theme: SourceFile =
            css_source(FileId(1), "@theme { --duration-normal: 180ms; }").unwrap();
        let theme = Compiler::read_theme(&theme, &CssSourceName::from("stillwind.css")).unwrap();
        let options = Spanned::new(
            "duration-normal both".to_lean_str(),
            Span::new(FileId(2), 0, 20),
        );
        let mut diagnostics = Vec::new();
        let mut lint_findings = Vec::new();

        assert_eq!(
            resolve_options(
                &options,
                OptionVariants::Rejected,
                &theme,
                &mut diagnostics,
                &mut lint_findings
            )
            .declaration,
            "animation-duration:var(--duration-normal);animation-fill-mode:both;"
        );
        assert_eq!(diagnostics, [] as [CompileError; 0]);
    }

    #[test]
    fn splits_a_conditioned_option_from_the_unconditional_ones() {
        let theme = css_source(FileId(2), "").unwrap();
        let theme = Compiler::read_theme(&theme, &CssSourceName::from("stillwind.css")).unwrap();
        let options = Spanned::new(
            "duration-180 motion-reduce:duration-0".to_lean_str(),
            Span::new(FileId(2), 0, 34),
        );
        let mut diagnostics = Vec::new();
        let mut lint_findings = Vec::new();

        let resolved = resolve_options(
            &options,
            OptionVariants::Conditions,
            &theme,
            &mut diagnostics,
            &mut lint_findings,
        );

        assert_eq!(resolved.declaration, "animation-duration:calc(180 * 1ms);");
        assert_eq!(resolved.conditioned.len(), 1);
        assert_eq!(
            resolved.conditioned[0].declaration,
            "animation-duration:calc(0 * 1ms);"
        );
        let [path] = resolved.conditioned[0].paths.as_slice() else {
            panic!("one condition path: {:?}", resolved.conditioned[0].paths);
        };
        assert_eq!(
            wrap_conditions("rule", path),
            "@media (prefers-reduced-motion: reduce){rule}"
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn rejects_a_variant_on_an_animation_option() {
        let theme = css_source(FileId(2), "").unwrap();
        let theme = Compiler::read_theme(&theme, &CssSourceName::from("stillwind.css")).unwrap();
        let options = Spanned::new(
            "motion-reduce:duration-0".to_lean_str(),
            Span::new(FileId(2), 0, 24),
        );
        let mut diagnostics = Vec::new();
        let mut lint_findings = Vec::new();

        let resolved = resolve_options(
            &options,
            OptionVariants::Rejected,
            &theme,
            &mut diagnostics,
            &mut lint_findings,
        );

        assert!(resolved.declaration.is_empty() && resolved.conditioned.is_empty());
        assert_eq!(
            lint_findings
                .iter()
                .map(|error| error.diagnostic.clone())
                .collect::<Vec<_>>(),
            vec![LintDiagnostic::UnknownAnimationOption {
                option: "motion-reduce:duration-0".to_lean_str(),
            }]
        );
    }

    #[test]
    fn strips_every_variant_prefix_from_an_option() {
        for (option, expected) in [
            ("duration-180", "duration-180"),
            ("motion-reduce:duration-0", "duration-0"),
            (
                "md:motion-reduce:[transition-duration:75ms]",
                "[transition-duration:75ms]",
            ),
        ] {
            let candidate = parse_candidate(option).expect("a candidate");
            assert_eq!(strip_variants(option, &candidate.variants), expected);
        }
    }

    #[test]
    fn reports_an_unknown_option_at_its_own_span() {
        let source = SourceFile::new(
            FileId(1), None, None,
            "import { style } from 'stillwind/react'; const motion = style.viewTransition({ group: 'motion-reduce:nope-0' });",
        )
        .unwrap();
        let theme = css_source(FileId(2), "").unwrap();
        let result = Compiler::compile_source(
            &OxcSourceAnalyzer,
            &source,
            &ModuleId::from("test.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
            &theme,
            &CssSourceName::from("stillwind.css"),
            &CompilerOptions::default(),
        );
        let errors = result
            .lint_findings
            .iter()
            .filter(|error| {
                error.diagnostic
                    == LintDiagnostic::UnknownAnimationOption {
                        option: "motion-reduce:nope-0".to_lean_str(),
                    }
            })
            .collect::<Vec<_>>();
        assert_eq!(errors.len(), 1, "{:?}", result.diagnostics);
        assert_eq!(
            source.slice(errors[0].labels[0].span).unwrap(),
            "motion-reduce:nope-0"
        );
    }
}
