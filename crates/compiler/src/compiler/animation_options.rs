//! Resolution of the option list of a keyframes use.
//!
//! Options resolve through the `animation` registry context. Its definitions write
//! `animation-*` declarations, so `duration-180` sets `animation-duration` here and
//! `transition-duration` on an element.

use lean_string::{LeanStr, LeanString};

use stillwind_design_system::{RegistryContext, ThemeIndex, resolve_utility};
use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintDiagnostic, LintFinding};
use stillwind_source::Spanned;
use stillwind_syntax::parse_candidate;

use super::lexer::lex_utilities;

/// Resolves a whitespace-separated option list and returns its declarations in written
/// order.
///
/// Each option contributes its own declarations, so writing two durations is not
/// rejected here but resolves in source order. An option that cannot apply is reported
/// as an unknown option at its own span and contributes nothing: text that is not
/// candidate syntax, a variant, a negative value, `!important`, a name that the
/// `animation` context does not define, or a definition that produces selectors or
/// conditional rules.
pub(crate) fn resolve_options(
    options: &Spanned<LeanStr>,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
    lint_findings: &mut Vec<LintFinding>,
) -> LeanString {
    let theme = theme.context(RegistryContext::Animation);
    let mut declaration = LeanString::new();
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
        if candidate.important || candidate.negative || !candidate.variants.is_empty() {
            lint_findings.push(unknown());
            continue;
        }
        match resolve_utility(&raw, theme) {
            Ok(Ok(resolved))
                if resolved.selector.is_none()
                    && resolved.nested.is_empty()
                    && resolved.additional.is_empty() =>
            {
                declaration.push_str(&resolved.declaration);
            }
            Ok(_) => {
                lint_findings.push(unknown());
            }
            Err(error) => {
                diagnostics.push(CompileError::new(
                    CompileDiagnostic::MalformedUtilityRequirement {
                        utility: error.name,
                        offset: error.source.offset,
                    },
                    option.span,
                ));
            }
        }
    }
    declaration
}

#[cfg(test)]
mod tests {
    use lean_string::ToLeanStr;

    use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
    use stillwind_js_oxc::OxcSourceAnalyzer;
    use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile, Span};
    use stillwind_test_support::css_source;

    use super::*;
    use crate::test_support::read_theme;
    use crate::{Compiler, CompilerOptions};

    fn resolve(options: &str, theme: &str) -> (LeanString, Vec<LintFinding>) {
        let theme = read_theme(&css_source(FileId(1), theme).unwrap());
        let options = Spanned::new(
            options.to_lean_str(),
            Span::new(FileId(2), 0, u32::try_from(options.len()).unwrap()),
        );
        let mut diagnostics = Vec::new();
        let mut lint_findings = Vec::new();
        let declaration = resolve_options(&options, &theme, &mut diagnostics, &mut lint_findings);
        assert_eq!(diagnostics, [] as [CompileError; 0]);
        (declaration, lint_findings)
    }

    #[test]
    fn resolves_timing_utilities_as_animation_declarations() {
        assert_eq!(
            resolve(
                "duration-normal both",
                "@theme { --duration-normal: 180ms; }"
            ),
            (
                "animation-duration:var(--duration-normal);animation-fill-mode:both;".into(),
                Vec::new()
            )
        );
        assert_eq!(
            resolve("[animation-delay:75ms]", "").0,
            "animation-delay:75ms;"
        );
    }

    #[test]
    fn rejects_variants_and_utilities_outside_the_animation_context() {
        for option in [
            "motion-reduce:duration-0",
            "opacity-0",
            "[transition-duration:75ms]",
        ] {
            let (declaration, lint_findings) = resolve(option, "");
            assert!(declaration.is_empty(), "{option}: {declaration}");
            assert_eq!(
                lint_findings
                    .iter()
                    .map(|error| error.diagnostic.clone())
                    .collect::<Vec<_>>(),
                vec![LintDiagnostic::UnknownAnimationOption {
                    option: option.to_lean_str(),
                }]
            );
        }
    }

    #[test]
    fn reports_an_unknown_option_at_its_own_span() {
        let source = SourceFile::new(
            FileId(1), None, None,
            "import { style } from 'stillwind/react'; const enter = style.keyframes({ from: 'opacity-0' }); export const motion = style(enter('both nope-0'));",
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
                        option: "nope-0".to_lean_str(),
                    }
            })
            .collect::<Vec<_>>();
        assert_eq!(errors.len(), 1, "{:?}", result.diagnostics);
        assert_eq!(source.slice(errors[0].labels[0].span).unwrap(), "nope-0");
    }
}
