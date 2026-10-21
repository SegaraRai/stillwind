//! View-transition definitions: the rules of the four pseudo-elements of one transition.

use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_design_system::{RegistryContext, ThemeIndex};
use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintDiagnostic, LintFinding};
use stillwind_ir::ViewTransitionDefinition;
use stillwind_source::{ModuleId, Spanned};

use super::super::CssContribution;
use super::super::lexer::lex_utilities;
use super::super::symbols::named_declaration;
use super::super::utilities::{
    combine_declarations_for_theme, invalid_property_value_finding, resolve_first_target,
    unresolved_utility_finding,
};
use super::super::variants::{condition_paths, wrap_conditions};
use crate::identifiers::IdentifierContext;

/// Writes the view-transition rules for one declaration.
///
/// The `group`, `old`, `new`, and `image-pair` pseudo-element rules carry their utility
/// lists. Every pseudo-element is keyed by the same generated name, which is what pairs
/// them at runtime.
pub(in crate::compiler) fn emit_view_transition_definition(
    contribution: &mut CssContribution,
    module_id: &ModuleId,
    identifiers: &IdentifierContext,
    transition: &ViewTransitionDefinition,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
    lint_findings: &mut Vec<LintFinding>,
) {
    let class = match identifiers.view_transition_class(&named_declaration(
        module_id,
        &transition.binding,
        transition.label.as_ref(),
    )) {
        Ok(class) => class,
        Err(failure) => {
            diagnostics.push(CompileError::new(failure, transition.call_span));
            return;
        }
    };
    let class = class.selector();
    let mut css: LeanString = std::mem::take(&mut contribution.css).into();
    for (pseudo, utilities) in [
        ("group", transition.group.as_ref()),
        ("old", transition.old.as_ref()),
        ("new", transition.new.as_ref()),
        ("image-pair", transition.image_pair.as_ref()),
    ] {
        let Some(utilities) = utilities else {
            continue;
        };
        emit_view_transition_utilities(
            &mut css,
            &format_args!("::view-transition-{pseudo}({class})").to_lean_string(),
            utilities,
            theme,
            diagnostics,
            &mut contribution.required_properties,
            lint_findings,
        );
    }
    contribution.css = css.into();
}

/// Writes one pseudo-element's rules for `selector`, resolving utilities through the
/// `view-transition` context of `theme`.
///
/// Utilities without variants share one rule. A utility carrying condition variants emits its
/// own rule per condition path, in source order after the shared rule, so a condition selects
/// its declarations without changing what the unconditional rule says. Rejected tokens
/// contribute nothing; successful tokens still append their property requirements, so callers
/// must check diagnostics before publishing.
pub(crate) fn emit_view_transition_utilities(
    output: &mut LeanString,
    selector: &str,
    input: &Spanned<LeanStr>,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
    required_properties: &mut Vec<LeanStr>,
    lint_findings: &mut Vec<LintFinding>,
) {
    let theme = theme.context(RegistryContext::ViewTransition);
    let mut plain = Vec::new();
    let mut conditional = LeanString::new();
    for utility in lex_utilities(input, diagnostics) {
        let raw = &utility.value.raw;
        let resolved = match resolve_first_target(raw, theme) {
            Ok(Ok(resolved)) => resolved,
            Ok(Err(failure)) => {
                lint_findings.push(unresolved_utility_finding(
                    raw,
                    raw.len(),
                    &failure,
                    utility.span,
                    theme,
                ));
                continue;
            }
            Err(error) => {
                diagnostics.push(CompileError::new(
                    CompileDiagnostic::MalformedUtilityRequirement {
                        utility: error.name,
                        offset: error.source.offset,
                    },
                    utility.span,
                ));
                continue;
            }
        };
        if let Some(value) = &resolved.diagnostic_data.invalid_value {
            lint_findings.push(invalid_property_value_finding(
                raw,
                raw.len(),
                value,
                utility.span,
            ));
        }
        if resolved.variants.is_empty() {
            plain.push(resolved);
            continue;
        }
        let Some(paths) = condition_paths(&resolved.variants, theme) else {
            lint_findings.push(LintFinding::new(
                LintDiagnostic::InvalidViewTransitionVariant,
                utility.span,
            ));
            continue;
        };
        required_properties.extend(resolved.required_properties.clone());
        let declaration = combine_declarations_for_theme(&[&resolved], theme);
        for path in paths {
            let rule = format_args!("{selector}{{ {declaration}}}").to_lean_string();
            conditional.push_str(&wrap_conditions(&rule, &path));
        }
    }
    if !plain.is_empty() {
        required_properties.extend(
            plain
                .iter()
                .flat_map(|utility| utility.required_properties.clone()),
        );
        output.push_str(selector);
        output.push_str("{ ");
        output.push_str(&combine_declarations_for_theme(
            &plain.iter().collect::<Vec<_>>(),
            theme,
        ));
        output.push('}');
    }
    output.push_str(&conditional);
}

#[cfg(test)]
mod tests {
    use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
    use stillwind_js_oxc::OxcSourceAnalyzer;
    use stillwind_source::{CssSourceName, FileId, SourceFile};
    use stillwind_test_support::css_source;

    use super::*;
    use crate::{Compiler, CompilerOptions};

    #[test]
    fn reports_bucket_and_group_variants_at_their_own_spans() {
        for (definition, expected) in [
            (
                "style.viewTransition({ old: 'hover:opacity-0' })",
                "hover:opacity-0",
            ),
            (
                "style.viewTransition({ imagePair: 'focus:opacity-50' })",
                "focus:opacity-50",
            ),
            (
                "style.viewTransition({ group: 'hover:duration-180' })",
                "hover:duration-180",
            ),
        ] {
            let source = SourceFile::new(
                FileId(1),
                None,
                None,
                (format_args!(
                    "import {{ style }} from 'stillwind/react'; const motion = {definition};"
                )
                .to_lean_string())
                .as_str(),
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
                .filter(|error| error.diagnostic == LintDiagnostic::InvalidViewTransitionVariant)
                .collect::<Vec<_>>();
            assert_eq!(errors.len(), 1, "{definition}: {:?}", result.diagnostics);
            assert_eq!(source.slice(errors[0].labels[0].span).unwrap(), expected);
        }
    }
}
