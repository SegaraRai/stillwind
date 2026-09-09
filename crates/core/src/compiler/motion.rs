use super::{
    AnimationDefinition, AssetReference, CompileError, CompiledUtility, SourceFile, Spanned,
    ViewTransitionDefinition, asset_symbol_name, combine_declarations_for_theme, lex_utilities,
    resolve_utility, symbol_name,
};

pub(super) fn emit_animation_definition(
    output: &mut String,
    source: &SourceFile,
    animation: &AnimationDefinition,
    theme: &stillwind_css::ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) {
    output.push_str("@keyframes ");
    output.push_str(&symbol_name(source, "animation", &animation.name));
    output.push('{');
    for frame in &animation.frames {
        let mut utilities = Vec::new();
        for utility in lex_utilities(&frame.utilities) {
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
    theme: &stillwind_css::ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) -> CompiledUtility {
    let mut declaration = format!(
        "animation-name:{};",
        asset_symbol_name(&animation.module_path, "animation", &animation.name)
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
    theme: &stillwind_css::ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) -> String {
    let mut declaration = String::new();
    for option in lex_utilities(options) {
        if let Some(resolved) =
            stillwind_tailwind::resolve_animation_option(&option.value.raw, theme)
        {
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
    theme: &stillwind_css::ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) {
    let name = symbol_name(source, "view-transition", &transition.name);
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
    theme: &stillwind_css::ThemeIndex,
    diagnostics: &mut Vec<CompileError>,
) -> String {
    let mut utilities = Vec::new();
    for utility in lex_utilities(input) {
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
