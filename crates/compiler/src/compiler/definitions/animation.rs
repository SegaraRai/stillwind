//! Complete conditional keyframe definitions and their registration requirements.

use stillwind_design_system::ThemeIndex;
use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::{AnimationBody, AnimationDefinition, AnimationFrame, StyleVariant};
use stillwind_source::{SourceFile, Span};

use super::super::CssContribution;
use super::super::lexer::lex_utilities;
use super::super::style_variants::StyleVariantPlan;
use super::super::symbols::named_declaration;
use super::super::utilities::declaration_order::combine_declarations_for_theme;
use super::super::utilities::resolve::resolve_utility;
use super::super::variants::{VariantStep, conditions_only, variant_paths};
use crate::identifiers::IdentifierContext;

/// Emits default keyframes first and complete replacements in canonical variant order.
/// Nested branches conjoin their conditions; missing frames never inherit ancestor frames.
pub(in crate::compiler) fn emit_animation_definition(
    contribution: &mut CssContribution,
    source: &SourceFile,
    identifiers: &IdentifierContext,
    animation: &AnimationDefinition,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) {
    let mut definitions = Vec::new();
    collect_definitions(
        &animation.body,
        &mut Vec::new(),
        identifiers,
        theme,
        diagnostics,
        &mut definitions,
    );
    definitions.sort_by(|(left, _), (right, _)| left.compare_order(right, theme));
    let name =
        match identifiers.animation_name(&named_declaration(source.path(), &animation.binding)) {
            Ok(name) => name,
            Err(failure) => {
                diagnostics.push(CompileError::new(failure, animation.call_span));
                return;
            }
        };
    contribution.keyframes = true;
    for (plan, frames) in definitions {
        let mut frame_contribution = CssContribution::default();
        emit_frames(&mut frame_contribution, frames, theme, diagnostics);
        let rule = format!(
            "@keyframes {}{{{}}}",
            name.custom_ident(),
            frame_contribution.css
        );
        contribution
            .required_properties
            .extend(frame_contribution.required_properties);
        for applications in plan.applications() {
            for path in variant_paths(applications) {
                let mut css = rule.clone();
                for step in path.iter().rev() {
                    css = match step {
                        VariantStep::Media(query) => format!("@media {query}{{{css}}}"),
                        VariantStep::Supports(query) => format!("@supports ({query}){{{css}}}"),
                        _ => continue,
                    };
                }
                contribution.css.push_str(&css);
            }
        }
    }
}

fn collect_definitions<'a>(
    body: &'a AnimationBody,
    wrappers: &mut Vec<StyleVariant>,
    identifiers: &IdentifierContext,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
    definitions: &mut Vec<(StyleVariantPlan, &'a [AnimationFrame])>,
) {
    if let Some(plan) = StyleVariantPlan::resolve(identifiers, wrappers, theme, diagnostics) {
        let valid = plan
            .applications()
            .iter()
            .all(|applications| conditions_only(&variant_paths(applications)));
        if !valid {
            if let Some(variant) = wrappers.last() {
                diagnostics.push(CompileError::new(
                    CompileErrorKind::InvalidAnimationVariant,
                    variant_span(variant),
                ));
            }
            return;
        }
        if !body.frames.is_empty() {
            definitions.push((plan, &body.frames));
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
        );
        wrappers.pop();
    }
}

fn variant_span(variant: &StyleVariant) -> Span {
    match variant {
        StyleVariant::Prefix(prefix) | StyleVariant::Ref { prefix, .. } => prefix.span,
        StyleVariant::Container { query, .. } => query.span,
    }
}

fn emit_frames(
    contribution: &mut CssContribution,
    frames: &[AnimationFrame],
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) {
    let output = &mut contribution.css;
    for frame in frames {
        let mut utilities = Vec::new();
        for utility in lex_utilities(&frame.utilities, diagnostics) {
            match resolve_utility(&utility.value.raw, theme) {
                Some(utility) if utility.variants.is_empty() => utilities.push(utility),
                Some(_) => diagnostics.push(CompileError::new(
                    CompileErrorKind::AnimationFrameVariants,
                    utility.span,
                )),
                None => diagnostics.push(CompileError::new(
                    CompileErrorKind::UnavailableUtility {
                        utility: utility.value.raw.clone(),
                    },
                    utility.span,
                )),
            }
        }
        contribution.required_properties.extend(
            utilities
                .iter()
                .flat_map(|utility| utility.required_properties.clone()),
        );
        output.push_str(&frame.selector.value);
        output.push('{');
        output.push_str(&combine_declarations_for_theme(
            &utilities.iter().collect::<Vec<_>>(),
            theme,
        ));
        output.push('}');
    }
}

#[cfg(test)]
mod tests {
    use stillwind_js_oxc::OxcSourceAnalyzer;
    use stillwind_source::FileId;

    use super::*;
    use crate::Compiler;

    #[test]
    fn reports_frame_variants_at_their_own_span() {
        let source = SourceFile::new(
            FileId(1),
            "motion.tsx",
            "import { style } from 'stillwind/react'; const motion = style.animation({ from: 'hover:opacity-0' });",
        )
        .unwrap();
        let theme = crate::css_fixture::source(FileId(2), "theme", "").unwrap();
        let result = Compiler::compile_source(&OxcSourceAnalyzer, &source, &theme);
        let errors = result
            .diagnostics
            .iter()
            .filter(|error| error.kind == CompileErrorKind::AnimationFrameVariants)
            .collect::<Vec<_>>();
        assert_eq!(errors.len(), 1, "{:?}", result.diagnostics);
        assert_eq!(
            source.slice(errors[0].labels[0].span).unwrap(),
            "hover:opacity-0"
        );
    }
}
