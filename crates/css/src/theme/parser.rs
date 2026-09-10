use std::collections::{BTreeMap, BTreeSet};

use lightningcss::error::{Error, ParserError};
use lightningcss::stylesheet::{ParserOptions, StyleSheet};

use stillwind_source::{FileId, SourceFile};

use crate::blocks::unterminated_block;
use crate::builtins::TAILWIND_V4_THEME;
use crate::custom_properties::registration::{
    extract_authored_property_names, extract_property_definitions,
};
use crate::directives::strip_framework_directives;
use crate::registry::formats::{extract_declaration_formats, extract_relationship_patterns};
use crate::registry::utilities::{extract_custom_utilities, extract_marker_utilities};
use crate::registry::variants::extract_custom_variants;

use super::config::{StillwindConfig, extract_stillwind_config};
use super::declarations::{ThemeTokenMode, extract_theme_declarations};
use super::imports::{FrameworkImports, framework_imports};
use super::index::ThemeIndex;

#[derive(Clone, Debug, thiserror::Error)]
pub enum ThemeError {
    #[error("{0}")]
    Output(#[from] crate::CssOutputError),
    #[error("CSS source offset {0} exceeds the 32-bit source limit")]
    InvalidOffset(usize),
    #[error("CSS parse error: {0}")]
    Parse(#[source] Error<ParserError<'static>>),
    #[error("unterminated @theme block")]
    UnterminatedTheme,
    #[error("unterminated CSS block at byte {offset}")]
    UnterminatedBlock { offset: usize },
}

/// Parses and indexes source-spanned `@theme` token declarations.
///
/// # Errors
///
/// Returns an error when Lightning CSS rejects the stylesheet or a discovered
/// `@theme` block has no closing brace.
pub fn parse_theme(source: &SourceFile) -> Result<ThemeIndex, ThemeError> {
    if let Some(offset) = unterminated_block(source.text()) {
        return Err(ThemeError::UnterminatedBlock { offset });
    }
    StyleSheet::parse(source.text(), ParserOptions::default())
        .map_err(|error| ThemeError::Parse(error.into_owned()))?;

    let mut theme = extract_theme_tokens(source.id(), source.text())?;
    theme.registry = std::sync::Arc::new(crate::BuiltinDefinitions::from_css(source.text()));
    theme.custom_css = strip_framework_directives(source.text());
    theme.utilities = extract_custom_utilities(source.text());
    theme.variants = extract_custom_variants(source.text());
    theme.properties = extract_property_definitions(source.text());
    theme.marker_utilities = extract_marker_utilities(source.text());
    theme.declaration_formats = extract_declaration_formats(source.text());
    theme.relationship_patterns = extract_relationship_patterns(source.text());
    theme.authored_properties = extract_authored_property_names(source.text());
    theme.config = extract_stillwind_config(source.text());
    Ok(theme)
}

/// Parses the Tailwind v4-compatible theme used by Stillwind's compiler.
///
/// New hosts should use `parse_resolved_theme` after resolving the CSS graph.
///
/// The vendored Tailwind defaults form the base theme, then the project CSS
/// extends, overrides, or clears namespaces with the normal `@theme` syntax.
///
/// # Errors
///
/// Returns an error when the project stylesheet is invalid or contains an
/// unterminated `@theme` block.
pub fn parse_tailwind_theme(source: &SourceFile) -> Result<ThemeIndex, ThemeError> {
    if let Some(offset) = unterminated_block(source.text()) {
        return Err(ThemeError::UnterminatedBlock { offset });
    }
    // Framework imports are compiler directives rather than browser CSS. Parse
    // the directive-stripped view so Tailwind's optional `prefix(...)` import
    // modifier (and other entrypoint modifiers) does not become a CSS syntax
    // error before we extract the semantic registries below.
    StyleSheet::parse(
        &strip_framework_directives(source.text()),
        ParserOptions::default(),
    )
    .map_err(|error| ThemeError::Parse(error.into_owned()))?;
    let imports = framework_imports(source.text());
    let mut tokens = if imports.theme_layer.is_some() {
        extract_theme_tokens(FileId(0), TAILWIND_V4_THEME)?.tokens
    } else {
        Vec::new()
    };
    tokens.extend(extract_theme_tokens(source.id(), source.text())?.tokens);
    if let Some(prefix) = imports.prefix.as_deref() {
        let prefix = prefix.trim_end_matches('-');
        for token in &mut tokens {
            if let Some(name) = token.name.value.strip_prefix("--") {
                token.name.value = format!("--{prefix}-{name}");
            }
        }
    }
    let mut custom_css = strip_framework_directives(source.text());
    custom_css.push_str(&extract_keyframe_blocks(source.text()));
    Ok(ThemeIndex {
        output_template: None,
        registry: std::sync::Arc::new(crate::BuiltinDefinitions::from_css(&if imports.builtins {
            format!("{}\n{}", crate::BUILTIN_REGISTRY_CSS, source.text())
        } else {
            source.text().to_owned()
        })),
        tokens,
        custom_css,
        utilities: extract_custom_utilities(source.text()),
        variants: extract_custom_variants(source.text()),
        properties: extract_property_definitions(source.text()),
        marker_utilities: extract_marker_utilities(source.text()),
        declaration_formats: extract_declaration_formats(source.text()),
        relationship_patterns: extract_relationship_patterns(source.text()),
        authored_properties: extract_authored_property_names(source.text()),
        imports,
        config: extract_stillwind_config(source.text()),
    })
}

/// Parses only host-supplied definitions and retains their authored output positions.
/// No package names, default theme, reset, or registry are injected here.
///
/// # Errors
/// Returns invalid CSS, unterminated blocks, or unsupported output slots.
pub fn parse_resolved_theme(source: &SourceFile) -> Result<ThemeIndex, ThemeError> {
    if let Some(offset) = unterminated_block(source.text()) {
        return Err(ThemeError::UnterminatedBlock { offset });
    }
    let mut theme = extract_theme_tokens(source.id(), source.text())?;
    let template = crate::CssOutputTemplate::parse(source.text(), &theme.tokens)?;
    theme.custom_css = template.authored_css();
    StyleSheet::parse(&theme.custom_css, ParserOptions::default())
        .map_err(|error| ThemeError::Parse(error.into_owned()))?;
    theme.output_template = Some(template);
    theme.registry = std::sync::Arc::new(crate::BuiltinDefinitions::from_css(source.text()));
    theme.utilities = extract_custom_utilities(source.text());
    theme.variants = extract_custom_variants(source.text());
    theme.properties = extract_property_definitions(source.text());
    theme.marker_utilities = extract_marker_utilities(source.text());
    theme.declaration_formats = extract_declaration_formats(source.text());
    theme.relationship_patterns = extract_relationship_patterns(source.text());
    theme.authored_properties = extract_authored_property_names(source.text());
    theme.config = extract_stillwind_config(source.text());
    theme.imports = FrameworkImports::explicit();
    theme.imports.builtins = true;
    Ok(theme)
}

pub(crate) fn extract_keyframe_blocks(css: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    while let Some(relative) = css[cursor..].find("@keyframes ") {
        let start = cursor + relative;
        let Some(open_rel) = css[start..].find('{') else {
            break;
        };
        let open = start + open_rel;
        let mut depth = 0_u32;
        let mut close = None;
        for (offset, ch) in css[open..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        close = Some(open + offset);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(close) = close else { break };
        output.push_str(&css[start..=close]);
        cursor = close + 1;
    }
    output
}

pub(crate) fn offset(value: usize) -> Result<u32, ThemeError> {
    u32::try_from(value).map_err(|_| ThemeError::InvalidOffset(value))
}

pub(crate) fn extract_theme_tokens(file: FileId, css: &str) -> Result<ThemeIndex, ThemeError> {
    let mut tokens = Vec::new();
    for (range, modifier) in crate::output::theme_blocks(css)? {
        let mode = ThemeTokenMode::parse(&modifier);
        tokens.extend(extract_theme_declarations(
            file,
            &css[range.clone()],
            range.start,
            mode,
        )?);
    }
    Ok(ThemeIndex {
        output_template: None,
        registry: std::sync::Arc::default(),
        tokens,
        custom_css: String::new(),
        utilities: Vec::new(),
        variants: Vec::new(),
        properties: Vec::new(),
        marker_utilities: BTreeSet::new(),
        declaration_formats: BTreeMap::new(),
        relationship_patterns: BTreeSet::new(),
        authored_properties: BTreeSet::new(),
        imports: FrameworkImports::default(),
        config: StillwindConfig::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_css_uses_only_supplied_definitions_and_ignores_directive_text() {
        let source = SourceFile::new(FileId(1), "entry.css", format!("{}\n/* @theme {{ --fake: red; }} */ .x{{content:'@theme';}} @theme {{ --color-local: blue; }} @stillwind-slot utilities;", crate::BUILTIN_REGISTRY_CSS)).unwrap();
        let theme = parse_resolved_theme(&source).unwrap();
        assert_eq!(theme.tokens.len(), 1);
        assert_eq!(theme.tokens[0].name.value, "--color-local");
        assert!(theme.registry.builtin_exact.contains_key("flex"));
        assert!(theme.custom_css.contains("content:'@theme'"));
        assert!(
            !crate::blocks::strip_css_comments(&theme.custom_css).contains("@stillwind"),
            "{}",
            theme.custom_css
        );
    }

    #[test]
    fn keyframe_extraction_keeps_nested_blocks_and_ignores_other_rules() {
        assert_eq!(
            extract_keyframe_blocks("a{}@keyframes spin {from{x:1}to{x:2}}b{}"),
            "@keyframes spin {from{x:1}to{x:2}}"
        );
        assert_eq!(extract_keyframe_blocks("@keyframes broken {"), "");
    }
}

#[cfg(test)]
mod panic_regressions {
    use super::*;

    #[test]
    fn unclosed_definition_blocks_return_errors() {
        for directive in ["@utility x", "@custom-variant x", "@stillwind-variant x"] {
            for body in ["", " color:red;", " &:hover {color:red;}"] {
                let css = format!("{directive} {{{body}");
                let source = SourceFile::new(FileId(1), "unclosed.css", css.as_str()).unwrap();
                for result in [parse_theme(&source), parse_tailwind_theme(&source)] {
                    assert!(
                        matches!(result, Err(ThemeError::UnterminatedBlock { offset })
                        if offset == directive.len() + 1),
                        "{css}: {result:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn closed_blocks_with_braces_in_comments_and_strings_are_accepted() {
        let source = SourceFile::new(
            FileId(1),
            "closed.css",
            r#"/* { */ .x { content: "{"; /* } */ &:hover { content: "}"; } }"#,
        )
        .unwrap();
        assert!(parse_theme(&source).is_ok());
        assert!(parse_tailwind_theme(&source).is_ok());
        let source = SourceFile::new(
            FileId(1),
            "utility.css",
            "@utility x { color:red; &:hover { color:blue; } }",
        )
        .unwrap();
        assert_eq!(parse_tailwind_theme(&source).unwrap().utilities.len(), 1);
    }
}
