//! Animation and view-transition definitions, and the declarations their uses emit.
//!
//! A definition is emitted once per declaration under a name hashed from the declaration's
//! identity, and every use references that name, so a keyframe pair or view-transition pair is
//! written to the stylesheet once however many call sites use it. `animation` emits complete
//! conditional keyframe definitions. Frame utilities and view-transition buckets require
//! unprefixed utilities; conditions on animations belong to their definition branches.
//!
//! Options are resolved through the shared candidate resolver, which is why a theme duration
//! is usable as an animation option even though it is not a utility.

use stillwind_design_system::{ThemeIndex, UtilityOrder, resolve_animation_option};
use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::{AssetReference, ViewTransitionDefinition};
use stillwind_semantic::AssetKind;
use stillwind_source::{SourceFile, Spanned};

use super::CssContribution;
use super::lexer::lex_utilities;
use super::symbols::{motion_name, motion_symbol_name};
use super::utilities::declaration_order::combine_declarations_for_theme;
use super::utilities::resolve::{CompiledUtility, resolve_utility};
use crate::identifier::IdentifierContext;

mod animation;

pub(super) use animation::emit_animation_definition;

/// Builds the declarations one animation use contributes.
///
/// The `animation-name` always names this declaration's keyframes, and the options follow in
/// their written order.
pub(super) fn resolve_animation_use(
    identifiers: &IdentifierContext,
    animation: &AssetReference,
    options: &Spanned<String>,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) -> CompiledUtility {
    let mut declaration = format!(
        "animation-name:{};",
        match motion_symbol_name(identifiers, &animation.identity) {
            Ok(name) => name,
            Err(failure) => {
                diagnostics.push(CompileError::new(failure, options.span));
                String::new()
            }
        }
    );
    declaration.push_str(&resolve_motion_options(options, theme, diagnostics));
    CompiledUtility {
        required_properties: Vec::new(),
        variants: Vec::new(),
        variant_applications: Vec::new(),
        declaration,
        selector: None,
        container: None,
        nested: Vec::new(),
        order: UtilityOrder::Authored(None),
        candidate: String::new(),
    }
}

/// Resolves a whitespace-separated option list into declarations.
///
/// Each option contributes its own declarations, so writing two durations is not rejected here
/// but resolves in source order. An unknown option is reported at its own span and contributes
/// nothing.
pub(super) fn resolve_motion_options(
    options: &Spanned<String>,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) -> String {
    let mut declaration = String::new();
    for option in lex_utilities(options, diagnostics) {
        if let Some(resolved) = resolve_animation_option(&option.value.raw, theme) {
            declaration.push_str(&resolved);
        } else {
            diagnostics.push(CompileError::new(
                CompileErrorKind::UnknownAnimationOption {
                    option: option.value.raw.clone(),
                },
                option.span,
            ));
        }
    }
    declaration
}

/// Writes the view-transition rules for one declaration.
///
/// The group rule carries the transition options, and the `old` and `new` pseudo-element rules
/// carry their utility lists. Both pseudo-elements are keyed by the same generated name, which
/// is what pairs them at runtime.
pub(super) fn emit_view_transition_definition(
    contribution: &mut CssContribution,
    source: &SourceFile,
    identifiers: &IdentifierContext,
    transition: &ViewTransitionDefinition,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) {
    let output = &mut contribution.css;
    let name = match motion_name(
        identifiers,
        source.path(),
        AssetKind::ViewTransition,
        &transition.name,
    ) {
        Ok(name) => name,
        Err(failure) => {
            diagnostics.push(CompileError::new(failure, transition.call_span));
            return;
        }
    };
    if let Some(group) = &transition.group {
        output.push_str("::view-transition-group(");
        output.push('.');
        output.push_str(&name);
        output.push_str("){ ");
        output.push_str(&resolve_motion_options(group, theme, diagnostics));
        output.push('}');
    }
    for (pseudo, utilities) in [
        ("old", transition.old.as_ref()),
        ("new", transition.new.as_ref()),
    ] {
        let Some(utilities) = utilities else {
            continue;
        };
        output.push_str("::view-transition-");
        output.push_str(pseudo);
        output.push('(');
        output.push('.');
        output.push_str(&name);
        output.push_str("){ ");
        output.push_str(&resolve_static_utility_list(
            utilities,
            theme,
            diagnostics,
            &mut contribution.required_properties,
        ));
        output.push('}');
    }
}

/// Resolves a definition's utility list and combines the successful declarations.
///
/// Invalid coordinates, unresolved utilities, and variant-qualified utilities append
/// fatal errors. Rejected tokens contribute nothing; successful tokens still append
/// their property requirements, so callers must check diagnostics before publishing.
pub(super) fn resolve_static_utility_list(
    input: &Spanned<String>,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
    required_properties: &mut Vec<String>,
) -> String {
    let mut utilities = Vec::new();
    for utility in lex_utilities(input, diagnostics) {
        match resolve_utility(&utility.value.raw, theme) {
            Some(utility) if utility.variants.is_empty() => utilities.push(utility),
            Some(_) => diagnostics.push(CompileError::new(
                CompileErrorKind::MotionVariants,
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
    required_properties.extend(
        utilities
            .iter()
            .flat_map(|utility| utility.required_properties.clone()),
    );
    combine_declarations_for_theme(&utilities.iter().collect::<Vec<_>>(), theme)
}

#[cfg(test)]
mod tests {
    use stillwind_js_oxc::OxcSourceAnalyzer;
    use stillwind_source::{FileId, SourceFile, Span};

    use super::*;
    use crate::Compiler;

    #[test]
    fn resolves_themed_duration_as_an_animation_option_only() {
        let theme: SourceFile = crate::css_fixture::source(
            FileId(1),
            "stillwind.css",
            "@theme { --duration-normal: 180ms; }",
        )
        .unwrap();
        let theme = Compiler::read_theme(&theme).unwrap();
        let options = Spanned::new(
            "duration-normal both".to_owned(),
            Span::new(FileId(2), 0, 20),
        );
        let mut diagnostics = Vec::new();

        assert_eq!(
            resolve_motion_options(&options, &theme, &mut diagnostics),
            "animation-duration:var(--duration-normal);animation-fill-mode:both;"
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn motion_variants_report_the_exact_source_span_through_compilation() {
        for (definition, kind) in [
            (
                "style.animation({ from: 'hover:opacity-0' })",
                CompileErrorKind::AnimationFrameVariants,
            ),
            (
                "style.viewTransition({ old: 'hover:opacity-0' })",
                CompileErrorKind::MotionVariants,
            ),
        ] {
            let source = SourceFile::new(
                FileId(1),
                "motion.tsx",
                format!("import {{ style }} from 'stillwind/react'; const motion = {definition};"),
            )
            .unwrap();
            let theme = crate::css_fixture::source(FileId(2), "theme", "").unwrap();
            let result = Compiler::compile_source(&OxcSourceAnalyzer, &source, &theme);
            let errors = result
                .diagnostics
                .iter()
                .filter(|error| error.kind == kind)
                .collect::<Vec<_>>();
            assert_eq!(errors.len(), 1, "{:?}", result.diagnostics);
            assert_eq!(
                source.slice(errors[0].labels[0].span).unwrap(),
                "hover:opacity-0"
            );
        }
    }
}
