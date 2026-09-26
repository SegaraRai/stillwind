//! View-transition definitions: the group rule and the three pseudo-element phases.

use stillwind_design_system::ThemeIndex;
use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::ViewTransitionDefinition;
use stillwind_source::Spanned;

use super::super::CssContribution;
use super::super::animation_options::{OptionVariants, ResolvedOptions, resolve_options};
use super::super::lexer::lex_utilities;
use super::super::symbols::named_declaration;
use super::super::utilities::declaration_order::combine_declarations_for_theme;
use super::super::utilities::resolve::resolve_utility;
use super::super::variants::{condition_paths, wrap_conditions};
use crate::identifiers::IdentifierContext;

/// Writes the view-transition rules for one declaration.
///
/// The group rule carries the transition options, and the `old`, `new`, and `image-pair`
/// pseudo-element rules carry their utility lists. Every pseudo-element is keyed by the same
/// generated name, which is what pairs them at runtime.
pub(in crate::compiler) fn emit_view_transition_definition(
    contribution: &mut CssContribution,
    module_id: &str,
    identifiers: &IdentifierContext,
    transition: &ViewTransitionDefinition,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) {
    let output = &mut contribution.css;
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
    if let Some(group) = &transition.group {
        let selector = format!("::view-transition-group({class})");
        let resolved = resolve_options(group, OptionVariants::Conditions, theme, diagnostics);
        emit_option_rules(output, &selector, &resolved);
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
            &format!("::view-transition-{pseudo}({class})"),
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
        let resolved = match resolve_utility(&utility.value.raw, theme) {
            Ok(Some(resolved)) => resolved,
            Ok(None) => {
                diagnostics.push(CompileError::new(
                    CompileErrorKind::UnavailableUtility {
                        utility: utility.value.raw.clone(),
                    },
                    utility.span,
                ));
                continue;
            }
            Err(error) => {
                diagnostics.push(CompileError::new(
                    CompileErrorKind::MalformedUtilityRequirement {
                        utility: error.name,
                        offset: error.source.offset,
                    },
                    utility.span,
                ));
                continue;
            }
        };
        if resolved.variants.is_empty() {
            plain.push(resolved);
            continue;
        }
        let Some(paths) = condition_paths(&resolved.variants, theme) else {
            diagnostics.push(CompileError::new(
                CompileErrorKind::InvalidViewTransitionVariant,
                utility.span,
            ));
            continue;
        };
        required_properties.extend(resolved.required_properties.clone());
        let declaration = combine_declarations_for_theme(&[&resolved], theme);
        for path in paths {
            let rule = format!("{selector}{{ {declaration}}}");
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

/// Writes one rule for the options that apply unconditionally, then one rule per conditioned
/// option.
///
/// Every rule keeps the same selector, so a condition changes only the properties its own options
/// state: the conditioned rule follows the unconditional one and wins for the properties both
/// declare. `view-transition-group` carries no element state the condition could select through,
/// so the condition wraps the rule rather than the selector.
fn emit_option_rules(output: &mut String, selector: &str, options: &ResolvedOptions) {
    if !options.declaration.is_empty() {
        output.push_str(selector);
        output.push_str("{ ");
        output.push_str(&options.declaration);
        output.push('}');
    }
    for option in &options.conditioned {
        let rule = format!("{selector}{{ {}}}", option.declaration);
        for path in &option.paths {
            output.push_str(&wrap_conditions(&rule, path));
        }
    }
}

#[cfg(test)]
mod tests {
    use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
    use stillwind_js_oxc::OxcSourceAnalyzer;
    use stillwind_source::{FileId, SourceFile};

    use super::*;
    use crate::Compiler;

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
                format!("import {{ style }} from 'stillwind/react'; const motion = {definition};"),
            )
            .unwrap();
            let theme = crate::css_fixture::source(FileId(2), "").unwrap();
            let result = Compiler::compile_source(
                &OxcSourceAnalyzer,
                &source,
                "test.tsx",
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
                &theme,
                "stillwind.css",
                &crate::CompilerOptions::default(),
            );
            let errors = result
                .diagnostics
                .iter()
                .filter(|error| error.kind == CompileErrorKind::InvalidViewTransitionVariant)
                .collect::<Vec<_>>();
            assert_eq!(errors.len(), 1, "{definition}: {:?}", result.diagnostics);
            assert_eq!(source.slice(errors[0].labels[0].span).unwrap(), expected);
        }
    }
}
