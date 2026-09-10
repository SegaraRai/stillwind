use stillwind_css::{
    PropertyUsageError, ThemeIndex, ThemeNamespace, ThemeTokenKind, tailwind_v4_keyframes_for,
    tailwind_v4_preflight,
};
use stillwind_source::FileId;
use stillwind_tailwind::{builtin_omit_token_prefixes, builtin_theme_order};

use crate::compiler::options::CompilerOptions;

use super::authored_directives::expand_authored_directives;
use super::keyframes::{extract_authored_keyframes, without_keyframes};
use super::properties::render_used_properties;

#[derive(Debug, thiserror::Error)]
pub enum StylesheetError {
    #[error("{0}")]
    PropertyUsage(#[from] PropertyUsageError),
    #[error("stylesheet contribution references a missing rule")]
    MissingRule,
}

#[allow(clippy::too_many_lines)]
pub(crate) fn render_stylesheet(
    theme: &ThemeIndex,
    utility_css: &str,
    options: &CompilerOptions,
) -> Result<String, StylesheetError> {
    render(theme, utility_css, options, true)
}

/// A producer's rules and their support closure, without document-wide CSS.
pub(crate) fn render_module_stylesheet(
    theme: &ThemeIndex,
    utility_css: &str,
    options: &CompilerOptions,
) -> Result<String, StylesheetError> {
    render(theme, utility_css, options, false)
}

#[allow(clippy::too_many_lines)]
fn render(
    theme: &ThemeIndex,
    utility_css: &str,
    options: &CompilerOptions,
    document: bool,
) -> Result<String, StylesheetError> {
    let imports = &theme.imports;
    let mut css = String::new();
    let authored_css = if document {
        expand_authored_directives(&without_keyframes(theme.custom_css()), theme)
    } else {
        String::new()
    };
    if document && imports.layer_order {
        css.push_str("@layer theme, base, components, utilities;\n");
    }
    let mut keyframes_css = String::new();
    let mut animation_names = Vec::new();
    if let Some(layer) = &imports.theme_layer {
        let used_css = format!("{utility_css}{authored_css}");
        let mut required_tokens = theme.tokens_required_by_with_omit_rules(
            &used_css,
            document && imports.base_layer.is_some(),
            builtin_omit_token_prefixes(),
        );
        required_tokens.sort_by_key(|token| (token.name.span.file, token.name.span.start));
        let theme_order = builtin_theme_order();
        if !theme_order.is_empty()
            && required_tokens
                .iter()
                .any(|token| token.name.span.file != FileId(0))
        {
            required_tokens.sort_by_key(|token| {
                let namespace = match token.kind {
                    ThemeTokenKind::Scale(namespace) | ThemeTokenKind::Token { namespace, .. } => {
                        Some(namespace.css_name())
                    }
                    ThemeTokenKind::Custom => None,
                };
                let rank = namespace
                    .and_then(|name| theme_order.iter().position(|item| item == name))
                    .unwrap_or(theme_order.len());
                (rank, token.name.span.file, token.name.span.start)
            });
        }
        let animations = required_tokens
            .iter()
            .filter_map(|token| match &token.kind {
                ThemeTokenKind::Token {
                    namespace: ThemeNamespace::Animate,
                    ..
                } => token
                    .value
                    .value
                    .split_whitespace()
                    .next()
                    .map(str::to_owned),
                _ => None,
            })
            .collect::<Vec<_>>();
        animation_names.clone_from(&animations);
        if !required_tokens.is_empty() || !animations.is_empty() {
            css.push_str("@layer ");
            css.push_str(layer);
            css.push_str("{:root,:host{");
            for token in &required_tokens {
                let Some(value) = theme.resolve_theme_references(&token.value.value) else {
                    continue;
                };
                css.push_str(&token.name.value);
                css.push(':');
                css.push_str(&value);
                css.push(';');
            }
            css.push_str("}}");
            keyframes_css = options.rewrite_generated(&tailwind_v4_keyframes_for(&animations));
            css.push('\n');
        }
    }
    if let Some(layer) = imports.base_layer.as_ref().filter(|_| document) {
        css.push_str("@layer ");
        css.push_str(layer);
        css.push('{');
        css.push_str(&options.rewrite_generated(&tailwind_v4_preflight()));
        css.push('}');
    }
    if let Some(layer) = &imports.utilities_layer {
        css.push_str("\n@layer ");
        css.push_str(layer);
        if utility_css.is_empty() {
            css.push_str(";\n");
        } else {
            css.push('{');
            css.push_str(utility_css);
            css.push_str("}\n");
        }
    }
    if !authored_css.trim().is_empty() {
        css.push_str(&authored_css);
        css.push('\n');
    }
    if !keyframes_css.is_empty() {
        css.push_str(&keyframes_css);
    }
    let authored_keyframes = extract_authored_keyframes(theme.custom_css(), &animation_names);
    if !authored_keyframes.is_empty() {
        css.push_str(&authored_keyframes);
    }
    let properties = render_used_properties(theme, &css, options)?;
    if !properties.is_empty() {
        css.insert_str(0, "@layer properties;\n");
        css.push('\n');
        css.push_str(&properties);
    }
    Ok(css)
}
