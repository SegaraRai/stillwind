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

use stillwind_design_system::{
    ThemeIndex, UtilityOrder, VariantApplication, resolve_animation_option, resolve_variant,
};
use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::{AssetReference, ViewTransitionDefinition};
use stillwind_source::{SourceFile, Spanned};
use stillwind_syntax::Variant;

use super::CssContribution;
use super::lexer::lex_utilities;
use super::symbols::{identity_declaration, named_declaration};
use super::utilities::declaration_order::combine_declarations_for_theme;
use super::utilities::resolve::{CompiledUtility, resolve_utility};
use super::variants::{VariantStep, conditions_only, variant_paths};
use crate::identifiers::IdentifierContext;

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
        match identifiers.animation_name(&identity_declaration(&animation.identity)) {
            Ok(name) => name.custom_ident(),
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
/// The group rule carries the transition options, and the `old`, `new`, and `image-pair`
/// pseudo-element rules carry their utility lists. Every pseudo-element is keyed by the same
/// generated name, which is what pairs them at runtime.
pub(super) fn emit_view_transition_definition(
    contribution: &mut CssContribution,
    source: &SourceFile,
    identifiers: &IdentifierContext,
    transition: &ViewTransitionDefinition,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) {
    let output = &mut contribution.css;
    let name = match identifiers
        .view_transition_class(&named_declaration(source.path(), &transition.name))
    {
        Ok(name) => name,
        Err(failure) => {
            diagnostics.push(CompileError::new(failure, transition.call_span));
            return;
        }
    };
    let name = name.selector();
    if let Some(group) = &transition.group {
        output.push_str("::view-transition-group(");
        output.push_str(&name);
        output.push_str("){ ");
        output.push_str(&resolve_motion_options(group, theme, diagnostics));
        output.push('}');
    }
    for (pseudo, utilities) in [
        ("old", transition.old.as_ref()),
        ("new", transition.new.as_ref()),
        ("image-pair", transition.image_pair.as_ref()),
    ] {
        let Some(utilities) = utilities else {
            continue;
        };
        emit_view_transition_utilities(
            output,
            &format!("::view-transition-{pseudo}({name})"),
            utilities,
            theme,
            diagnostics,
            &mut contribution.required_properties,
        );
    }
}

/// Writes one pseudo-element phase's rules for `selector`.
///
/// Utilities without variants share one rule. A utility carrying condition variants emits its
/// own rule per condition path, in source order after the shared rule, so a condition selects
/// its declarations without changing what the unconditional rule says. Rejected tokens
/// contribute nothing; successful tokens still append their property requirements, so callers
/// must check diagnostics before publishing.
fn emit_view_transition_utilities(
    output: &mut String,
    selector: &str,
    input: &Spanned<String>,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
    required_properties: &mut Vec<String>,
) {
    let mut plain = Vec::new();
    let mut conditional = String::new();
    for utility in lex_utilities(input, diagnostics) {
        let Some(resolved) = resolve_utility(&utility.value.raw, theme) else {
            diagnostics.push(CompileError::new(
                CompileErrorKind::UnavailableUtility {
                    utility: utility.value.raw.clone(),
                },
                utility.span,
            ));
            continue;
        };
        if resolved.variants.is_empty() {
            plain.push(resolved);
            continue;
        }
        let Some(applications) = condition_applications(&resolved.variants, theme) else {
            diagnostics.push(CompileError::new(
                CompileErrorKind::InvalidViewTransitionVariant,
                utility.span,
            ));
            continue;
        };
        required_properties.extend(resolved.required_properties.clone());
        let declaration = combine_declarations_for_theme(&[&resolved], theme);
        for path in variant_paths(&applications) {
            let mut rule = format!("{selector}{{ {declaration}}}");
            for step in path.iter().rev() {
                match step {
                    VariantStep::Media(condition) => {
                        rule = format!("@media {condition}{{{rule}}}");
                    }
                    VariantStep::Supports(condition) => {
                        rule = format!("@supports ({condition}){{{rule}}}");
                    }
                    _ => {}
                }
            }
            conditional.push_str(&rule);
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

/// Resolves a bucket utility's variants into the applications a pseudo-element rule can carry.
///
/// A view-transition pseudo-element has no element state of its own and no container ancestor,
/// so only media and supports conditions can wrap it. Any other variant fails the utility rather
/// than dropping its condition silently.
fn condition_applications(
    variants: &[Variant],
    theme: &ThemeIndex,
) -> Option<Vec<VariantApplication>> {
    let mut applications = Vec::new();
    for variant in variants {
        match resolve_variant(variant, theme)? {
            VariantApplication::Compound(children) => applications.extend(children),
            application => applications.push(application),
        }
    }
    conditions_only(&variant_paths(&applications)).then_some(applications)
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
        for (definition, kind, expected) in [
            (
                "style.animation({ from: 'hover:opacity-0' })",
                CompileErrorKind::AnimationFrameVariants,
                "hover:opacity-0",
            ),
            (
                "style.viewTransition({ old: 'hover:opacity-0' })",
                CompileErrorKind::InvalidViewTransitionVariant,
                "hover:opacity-0",
            ),
            (
                "style.viewTransition({ imagePair: 'focus:opacity-50' })",
                CompileErrorKind::InvalidViewTransitionVariant,
                "focus:opacity-50",
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
            assert_eq!(source.slice(errors[0].labels[0].span).unwrap(), expected);
        }
    }
}
