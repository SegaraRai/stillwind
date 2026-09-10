use stillwind_design_system::{ThemeIndex, resolve_animation_option};
use stillwind_ir::{AnimationDefinition, AssetReference, ViewTransitionDefinition};
use stillwind_semantic::AssetKind;
use stillwind_source::{CompileError, SourceFile, Spanned};

use super::lexer::lex_utilities;
use super::symbols::{asset_symbol_name, symbol_name};
use super::utilities::declaration_order::combine_declarations_for_theme;
use super::utilities::resolve::{CompiledUtility, resolve_utility};

pub(super) fn emit_animation_definition(
    output: &mut String,
    source: &SourceFile,
    animation: &AnimationDefinition,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) {
    output.push_str("@keyframes ");
    output.push_str(&symbol_name(source, AssetKind::Animation, &animation.name));
    output.push('{');
    for frame in &animation.frames {
        let mut utilities = Vec::new();
        for utility in lex_utilities(&frame.utilities, diagnostics) {
            match resolve_utility(&utility.value.raw, theme) {
                Some(utility) if utility.variants.is_empty() => utilities.push(utility),
                Some(_) => diagnostics.push(CompileError::new(
                    "SHANIMATION003",
                    "animation keyframes cannot contain variants",
                    utility.span,
                )),
                None => diagnostics.push(CompileError::new(
                    "SH001",
                    format!("unknown or unavailable utility `{}`", utility.value.raw),
                    utility.span,
                )),
            }
        }
        output.push_str(&frame.selector.value);
        output.push('{');
        output.push_str(&combine_declarations_for_theme(
            &utilities.iter().collect::<Vec<_>>(),
            theme,
        ));
        output.push('}');
    }
    output.push('}');
}

pub(super) fn resolve_animation_use(
    animation: &AssetReference,
    options: &Spanned<String>,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) -> CompiledUtility {
    let mut declaration = format!(
        "animation-name:{};",
        asset_symbol_name(
            &animation.module_path,
            AssetKind::Animation,
            &animation.name
        )
    );
    declaration.push_str(&resolve_motion_options(options, theme, diagnostics));
    CompiledUtility {
        variants: Vec::new(),
        declaration,
        selector: None,
        container: None,
        nested: Vec::new(),
        builtin_order: None,
        candidate: String::new(),
    }
}

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
                "SHANIMATION002",
                format!("unknown animation option `{}`", option.value.raw),
                option.span,
            ));
        }
    }
    declaration
}

pub(super) fn emit_view_transition_definition(
    output: &mut String,
    source: &SourceFile,
    transition: &ViewTransitionDefinition,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) {
    let name = symbol_name(source, AssetKind::ViewTransition, &transition.name);
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
        output.push_str(&resolve_static_utility_list(utilities, theme, diagnostics));
        output.push('}');
    }
}

pub(super) fn resolve_static_utility_list(
    input: &Spanned<String>,
    theme: &ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) -> String {
    let mut utilities = Vec::new();
    for utility in lex_utilities(input, diagnostics) {
        match resolve_utility(&utility.value.raw, theme) {
            Some(utility) if utility.variants.is_empty() => utilities.push(utility),
            Some(_) => diagnostics.push(CompileError::new(
                "SHMOTION001",
                "motion definition utilities cannot contain variants",
                utility.span,
            )),
            None => diagnostics.push(CompileError::new(
                "SH001",
                format!("unknown or unavailable utility `{}`", utility.value.raw),
                utility.span,
            )),
        }
    }
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
        let source = crate::css_fixture::source(
            FileId(1),
            "stillwind.css",
            "@theme { --duration-normal: 180ms; }",
        )
        .unwrap();
        let theme = Compiler::read_theme(&source).unwrap();
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
        for (definition, code) in [
            (
                "style.animation({ from: 'hover:opacity-0' })",
                "SHANIMATION003",
            ),
            (
                "style.viewTransition({ old: 'hover:opacity-0' })",
                "SHMOTION001",
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
                .filter(|error| error.code == code)
                .collect::<Vec<_>>();
            assert_eq!(errors.len(), 1, "{:?}", result.diagnostics);
            assert_eq!(
                source.slice(errors[0].labels[0].span).unwrap(),
                "hover:opacity-0"
            );
        }
    }
}
