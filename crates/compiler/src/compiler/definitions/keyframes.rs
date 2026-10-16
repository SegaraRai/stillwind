//! Complete conditional keyframe definitions and their registration requirements.

use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_design_system::ThemeIndex;
use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintDiagnostic, LintFinding};
use stillwind_ir::{KeyframesBody, KeyframesDefinition, KeyframesFrame, StyleVariant};
use stillwind_source::{ModuleId, Span, Spanned};

use super::super::CssContribution;
use super::super::lexer::lex_utilities;
use super::super::style_variants::StyleVariantPlan;
use super::super::symbols::named_declaration;
use super::super::utilities::{
    combine_declarations_for_theme, invalid_property_value_finding, resolve_utility,
    unresolved_utility_finding,
};
use super::super::variants::{OptionCondition, application_condition_paths, wrap_conditions};
use crate::identifiers::IdentifierContext;

/// Emits default keyframes first and complete replacements in canonical variant order.
/// Nested branches conjoin their conditions; missing frames never inherit ancestor frames.
pub(in crate::compiler) fn emit_keyframes_definition(
    contribution: &mut CssContribution,
    module_id: &ModuleId,
    identifiers: &IdentifierContext,
    keyframes: &KeyframesDefinition,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
    lint_findings: &mut Vec<LintFinding>,
) {
    let mut definitions = Vec::new();
    collect_definitions(
        &keyframes.body,
        &mut Vec::new(),
        identifiers,
        theme,
        diagnostics,
        &mut definitions,
        lint_findings,
    );
    definitions.sort_by(|(left, _, _), (right, _, _)| left.compare_order(right, theme));
    let name = match identifiers.keyframes_name(&named_declaration(
        module_id,
        &keyframes.binding,
        keyframes.label.as_ref(),
    )) {
        Ok(name) => name,
        Err(failure) => {
            diagnostics.push(CompileError::new(failure, keyframes.call_span));
            return;
        }
    };

    let mut css: LeanString = std::mem::take(&mut contribution.css).into();
    for (_, paths, frames) in definitions {
        let mut frame_contribution = CssContribution::default();
        emit_frames(
            &mut frame_contribution,
            frames,
            theme,
            diagnostics,
            lint_findings,
        );
        if frame_contribution.css.is_empty() {
            continue;
        }
        contribution.keyframes = true;
        let rule = format_args!(
            "@keyframes {}{{{}}}",
            name.custom_ident(),
            frame_contribution.css
        )
        .to_lean_string();
        contribution
            .required_properties
            .extend(frame_contribution.required_properties);
        for path in paths {
            css.push_str(&wrap_conditions(&rule, &path));
        }
    }
    contribution.css = css.into();
}

/// One definition to emit: the plan its wrappers resolve to, the condition paths those
/// wrappers produce, and the frames to write.
type Definition<'a> = (
    StyleVariantPlan,
    Vec<Vec<OptionCondition>>,
    &'a [KeyframesFrame],
);

fn collect_definitions<'a>(
    body: &'a KeyframesBody,
    wrappers: &mut Vec<StyleVariant>,
    identifiers: &IdentifierContext,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
    definitions: &mut Vec<Definition<'a>>,
    lint_findings: &mut Vec<LintFinding>,
) {
    let Some(plan) =
        StyleVariantPlan::resolve(identifiers, wrappers, theme, diagnostics, lint_findings)
    else {
        return;
    };
    {
        let Some(paths) = plan
            .applications()
            .iter()
            .map(|applications| application_condition_paths(applications))
            .collect::<Option<Vec<_>>>()
        else {
            if let Some(variant) = wrappers.last() {
                lint_findings.push(LintFinding::new(
                    LintDiagnostic::InvalidKeyframesVariant,
                    variant_span(variant),
                ));
            }
            return;
        };
        let paths = paths.into_iter().flatten().collect::<Vec<_>>();
        if !body.frames.is_empty() {
            definitions.push((plan, paths, &body.frames));
        }
    }
    for branch in &body.branches {
        wrappers.push(branch.variant.clone());
        collect_definitions(
            &branch.body,
            wrappers,
            identifiers,
            theme,
            diagnostics,
            definitions,
            lint_findings,
        );
        wrappers.pop();
    }
}

fn variant_span(variant: &StyleVariant) -> Span {
    match variant {
        StyleVariant::Prefix(prefix) => prefix.span,
        StyleVariant::Ref { condition, .. } => condition.prefix.span,
        StyleVariant::Container { query, .. } => query.span,
    }
}

fn emit_frames(
    contribution: &mut CssContribution,
    frames: &[KeyframesFrame],
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
    lint_findings: &mut Vec<LintFinding>,
) {
    let mut output: LeanString = std::mem::take(&mut contribution.css).into();
    for frame in frames {
        let (declaration, properties) =
            resolve_frame(&frame.utilities, theme, diagnostics, lint_findings);
        if declaration.is_empty() {
            continue;
        }
        contribution.required_properties.extend(properties);
        output.push_str(&frame.selector.value);
        output.push('{');
        output.push_str(&declaration);
        output.push('}');
    }
    contribution.css = output.into();
}

/// Resolves a frame's decoded utility list against `theme`, rejecting utility variants.
/// `input.span` locates token diagnostics in the original source or a synthetic query.
/// Invalid tokens append diagnostics and contribute no declarations; valid tokens still
/// contribute. Returns the combined declaration body without braces and the required
/// property names in utility order, including repeated names.
pub(crate) fn resolve_frame(
    input: &Spanned<LeanStr>,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
    lint_findings: &mut Vec<LintFinding>,
) -> (LeanString, Vec<LeanStr>) {
    let mut utilities = Vec::new();
    for utility in lex_utilities(input, diagnostics) {
        let raw = &utility.value.raw;
        match resolve_utility(raw, theme) {
            Ok(Ok(resolved)) if resolved.variants.is_empty() => {
                if let Some(value) = &resolved.source.invalid_value {
                    lint_findings.push(invalid_property_value_finding(
                        raw,
                        raw.len(),
                        value,
                        utility.span,
                    ));
                }
                utilities.push(resolved);
            }
            Ok(Ok(_)) => lint_findings.push(LintFinding::new(
                LintDiagnostic::KeyframesFrameVariants,
                utility.span,
            )),
            Ok(Err(failure)) => lint_findings.push(unresolved_utility_finding(
                raw,
                raw.len(),
                &failure,
                utility.span,
                theme,
            )),
            Err(error) => diagnostics.push(CompileError::new(
                CompileDiagnostic::MalformedUtilityRequirement {
                    utility: error.name,
                    offset: error.source.offset,
                },
                utility.span,
            )),
        }
    }
    let properties = utilities
        .iter()
        .flat_map(|utility| utility.required_properties.clone())
        .collect();
    let declaration = combine_declarations_for_theme(&utilities.iter().collect::<Vec<_>>(), theme);
    (declaration, properties)
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
    fn reports_frame_variants_at_their_own_span() {
        let source = SourceFile::new(
            FileId(1), None, None,
            "import { style } from 'stillwind/react'; const motion = style.keyframes({ from: 'hover:opacity-0' });",
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
            .filter(|error| error.diagnostic == LintDiagnostic::KeyframesFrameVariants)
            .collect::<Vec<_>>();
        assert_eq!(errors.len(), 1, "{:?}", result.diagnostics);
        assert_eq!(
            source.slice(errors[0].labels[0].span).unwrap(),
            "hover:opacity-0"
        );
    }
}
