//! Complete conditional keyframe definitions and their registration requirements.

use stillwind_design_system::{ThemeIndex, VariantApplication};
use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::{AnimationBody, AnimationDefinition, AnimationFrame, StyleVariant};
use stillwind_semantic::AssetKind;
use stillwind_source::{SourceFile, Span};

use super::super::CssContribution;
use super::super::lexer::lex_utilities;
use super::super::style_variants::StyleVariantPlan;
use super::super::symbols::motion_name;
use super::super::utilities::declaration_order::combine_declarations_for_theme;
use super::super::utilities::resolve::resolve_utility;
use super::super::variants::{VariantStep, variant_paths};
use crate::identifier::IdentifierContext;

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
    let name = match motion_name(
        identifiers,
        source.path(),
        AssetKind::Animation,
        &animation.name,
    ) {
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
        let rule = format!("@keyframes {name}{{{}}}", frame_contribution.css);
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
        let valid = plan.applications().iter().all(|applications| {
            !applications.iter().any(VariantApplication::uses_subject)
                && variant_paths(applications)
                    .iter()
                    .flatten()
                    .all(|step| matches!(step, VariantStep::Media(_) | VariantStep::Supports(_)))
        });
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
