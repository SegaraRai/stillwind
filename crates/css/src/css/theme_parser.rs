use super::{
    BTreeMap, BTreeSet, Error, FileId, FrameworkImports, ParserOptions, SourceFile,
    StillwindConfig, StyleSheet, TAILWIND_V4_THEME, ThemeIndex, ThemeTokenMode,
    extract_authored_property_names, extract_custom_utilities, extract_custom_variants,
    extract_declaration_formats, extract_marker_utilities, extract_property_definitions,
    extract_relationship_patterns, extract_stillwind_config, extract_theme_declarations,
    framework_imports, matching_brace, strip_framework_directives,
};

#[derive(Clone, Debug, Error)]
pub enum ThemeError {
    #[error("CSS parse error: {0}")]
    Parse(#[source] lightningcss::error::Error<lightningcss::error::ParserError<'static>>),
    #[error("unterminated @theme block")]
    UnterminatedTheme,
}

/// Parses and indexes source-spanned `@theme` token declarations.
///
/// # Errors
///
/// Returns an error when Lightning CSS rejects the stylesheet or a discovered
/// `@theme` block has no closing brace.
pub fn parse_theme(source: &SourceFile) -> Result<ThemeIndex, ThemeError> {
    StyleSheet::parse(source.text(), ParserOptions::default())
        .map_err(|error| ThemeError::Parse(error.into_owned()))?;

    let mut theme = extract_theme_tokens(source.id(), source.text())?;
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
/// The vendored Tailwind defaults form the base theme, then the project CSS
/// extends, overrides, or clears namespaces with the normal `@theme` syntax.
///
/// # Errors
///
/// Returns an error when the project stylesheet is invalid or contains an
/// unterminated `@theme` block.
pub fn parse_tailwind_theme(source: &SourceFile) -> Result<ThemeIndex, ThemeError> {
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

pub(super) fn extract_keyframe_blocks(css: &str) -> String {
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

pub(super) fn offset(value: usize) -> u32 {
    u32::try_from(value).expect("stillwind source files must be smaller than 4 GiB")
}

pub(super) fn extract_theme_tokens(file: FileId, css: &str) -> Result<ThemeIndex, ThemeError> {
    let mut tokens = Vec::new();
    let mut cursor = 0;

    while let Some(theme_relative) = css[cursor..].find("@theme") {
        let theme_start = cursor + theme_relative;
        let Some(open_relative) = css[theme_start..].find('{') else {
            return Err(ThemeError::UnterminatedTheme);
        };
        let open = theme_start + open_relative;
        let Some(close) = matching_brace(css, open) else {
            return Err(ThemeError::UnterminatedTheme);
        };

        let modifier = css[theme_start + "@theme".len()..open].trim();
        let mode = match modifier {
            "inline" => ThemeTokenMode::Inline,
            "static" => ThemeTokenMode::Static,
            _ => ThemeTokenMode::Default,
        };
        tokens.extend(extract_theme_declarations(
            file,
            &css[open + 1..close],
            open + 1,
            mode,
        ));

        cursor = close + 1;
    }

    Ok(ThemeIndex {
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
