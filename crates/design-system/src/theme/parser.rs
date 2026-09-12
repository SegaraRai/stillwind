//! Parsing one supplied CSS buffer into a [`ThemeIndex`].
//!
//! This is the syntactic entry point used both for a single file
//! ([`crate::DesignSystemSnapshot::from_source`]) and for each source the loader
//! accepts. It reads declared semantics only: no package name, default theme, reset, or
//! built-in registry is injected, so the definitions a snapshot ends up with are
//! exactly the ones the host supplied, expanded in authored import order.
//!
//! Because directives are found textually, the buffer is first checked for an
//! unterminated block, and the retained authored CSS is re-parsed by Lightning CSS so
//! that a syntax error surfaces as a typed [`ThemeError`].

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use lightningcss::error::{Error, ParserError};
use lightningcss::stylesheet::{ParserOptions, StyleSheet};

use stillwind_css::blocks::unterminated_block;
use stillwind_css::{
    CssOutputError, CssOutputTemplate, extract_authored_property_names,
    extract_property_definitions,
};
use stillwind_source::{FileId, SourceFile, Span, normalized_module_path};

use super::config::{StillwindConfig, extract_stillwind_config};
use super::declarations::{ThemeTokenMode, extract_theme_declarations};
use super::index::ThemeIndex;
use crate::BuiltinDefinitions;
use crate::output::{parse_output_template, theme_blocks};
use crate::registry::formats::{extract_declaration_formats, extract_relationship_patterns};
use crate::registry::utilities::{extract_custom_utilities, extract_marker_utilities};
use crate::registry::variants::extract_custom_variants;

/// Why one supplied CSS buffer could not be parsed as Stillwind CSS.
#[derive(Clone, Debug, thiserror::Error)]
pub enum ThemeError {
    /// The output template is invalid.
    #[error("{0}")]
    Output(#[from] CssOutputError),
    /// A CSS offset does not fit the 32-bit coordinate space.
    #[error("CSS source offset {0} exceeds the 32-bit source limit")]
    InvalidOffset(usize),
    /// The buffer is not valid CSS.
    #[error("CSS parse error: {0}")]
    Parse(#[source] Error<ParserError<'static>>),
    /// A `@theme` block reached the end of the buffer.
    #[error("unterminated @theme block")]
    UnterminatedTheme,
    /// A block reached the end of the buffer.
    #[error("unterminated CSS block at byte {offset}")]
    UnterminatedBlock {
        /// Byte offset where the block starts.
        offset: usize,
    },
}

/// Parses only host-supplied definitions and retains their authored output positions.
/// No package names, default theme, reset, or registry are injected here.
///
/// Directive text inside comments and strings is not a definition, and spans in the
/// returned template and tokens refer to `source`. The selected built-ins are taken
/// from the same buffer, so a caller that supplies no registry CSS receives an empty
/// registry.
///
/// # Errors
///
/// Returns invalid CSS, unterminated blocks, or unsupported output slots.
pub fn parse_theme(source: &SourceFile) -> Result<ThemeIndex, ThemeError> {
    if let Some(offset) = unterminated_block(source.text()) {
        return Err(ThemeError::UnterminatedBlock { offset });
    }
    let mut theme = extract_theme_tokens(source.id(), source.text())?;
    let mut template = parse_output_template(source.text(), &theme.tokens)?;
    template.map_origins(|span| Some(Span::new(source.id(), span.start.0, span.end.0)));
    template.source_identity = normalized_module_path(source.path());
    theme.custom_css = template.authored_css();
    StyleSheet::parse(&theme.custom_css, ParserOptions::default())
        .map_err(|error| ThemeError::Parse(error.into_owned()))?;
    theme.output_template = template;
    theme.registry = Arc::new(BuiltinDefinitions::from_css(source.text()));
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

/// Extracts the top-level `@keyframes` rules of a `@theme` body, in authored order.
///
/// Keyframes cannot be represented as custom properties, so they are copied textually
/// and emitted with the theme layer. An unterminated keyframes block ends the scan
/// rather than being emitted partially.
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

/// Converts a byte offset to the 32-bit source coordinate space.
///
/// # Errors
///
/// Returns an invalid-offset error when the value does not fit, so no caller can
/// truncate a span silently.
pub(crate) fn offset(value: usize) -> Result<u32, ThemeError> {
    u32::try_from(value).map_err(|_| ThemeError::InvalidOffset(value))
}

/// Collects the tokens of every `@theme` block, applying each block's mode to the
/// declarations it contains.
///
/// Only tokens are filled in; the remaining `ThemeIndex` fields are defaults, because
/// they depend on the whole buffer and are assembled by [`parse_theme`].
///
/// # Errors
///
/// Returns a block or offset error, or an unterminated `@theme` block error.
pub(crate) fn extract_theme_tokens(file: FileId, css: &str) -> Result<ThemeIndex, ThemeError> {
    let mut tokens = Vec::new();
    for (range, modifier) in theme_blocks(css)? {
        let mode = ThemeTokenMode::parse(&modifier);
        tokens.extend(extract_theme_declarations(
            file,
            &css[range.clone()],
            range.start,
            mode,
        )?);
    }
    Ok(ThemeIndex {
        output_template: CssOutputTemplate::default(),
        registry: Arc::default(),
        tokens,
        custom_css: String::new(),
        utilities: Vec::new(),
        variants: Vec::new(),
        properties: Vec::new(),
        marker_utilities: BTreeSet::new(),
        declaration_formats: BTreeMap::new(),
        relationship_patterns: BTreeSet::new(),
        authored_properties: BTreeSet::new(),

        config: StillwindConfig::default(),
    })
}

#[cfg(test)]
mod tests {
    use stillwind_css::blocks::strip_css_comments;

    use super::*;

    #[test]
    fn resolved_css_uses_only_supplied_definitions_and_ignores_directive_text() {
        let source = SourceFile::new(FileId(1), "entry.css", format!("{}\n/* @theme {{ --fake: red; }} */ .x{{content:'@theme';}} @theme {{ --color-local: blue; }} @stillwind-slot utilities;", crate::test_defaults::BUILTIN_REGISTRY_CSS)).unwrap();
        let theme = parse_theme(&source).unwrap();
        assert_eq!(theme.tokens.len(), 1);
        assert_eq!(theme.tokens[0].name.value, "--color-local");
        assert!(theme.registry.builtin_exact.contains_key("flex"));
        assert!(theme.custom_css.contains("content:'@theme'"));
        assert!(
            !strip_css_comments(&theme.custom_css).contains("@stillwind"),
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
                for result in [parse_theme(&source), parse_theme(&source)] {
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
        assert!(parse_theme(&source).is_ok());
        let source = SourceFile::new(
            FileId(1),
            "utility.css",
            "@utility x { color:red; &:hover { color:blue; } }",
        )
        .unwrap();
        assert_eq!(parse_theme(&source).unwrap().utilities.len(), 1);
    }
}
