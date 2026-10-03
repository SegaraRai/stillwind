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
//! that a syntax error surfaces as a typed [`CompileError`].

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use lightningcss::stylesheet::{ParserOptions, StyleSheet};

use stillwind_css::{
    CssOutputTemplate, MAX_VALUE_NESTING, extract_authored_property_names,
    extract_property_definitions, unterminated_block, value_nesting_overflow,
};
use stillwind_diagnostics::{CompileDiagnostic, CompileError};
use stillwind_source::{CssSourceName, FileId, SourceFile, Span};

use super::config::{StillwindConfig, extract_stillwind_config};
use super::declarations::{ThemeTokenMode, extract_theme_declarations};
use super::index::ThemeIndex;
use crate::BuiltinDefinitions;
use crate::output::{parse_output_template, theme_blocks};
use crate::registry::formats::{extract_declaration_formats, extract_relationship_patterns};
use crate::registry::utilities::{extract_custom_utilities, extract_marker_utilities};
use crate::registry::variants::extract_custom_variants;

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
/// Returns malformed CSS, unterminated blocks, values or blocks nested past the
/// supported depth, invalid output slots or contexts, conditional compile-time
/// definitions, or positions outside the source-coordinate limit. The error retains the
/// cause and uses `source.full_span()` as its primary label. Cause offsets refer to
/// this buffer.
pub fn parse_theme(
    source: &SourceFile,
    source_identity: &CssSourceName,
) -> Result<ThemeIndex, CompileError> {
    parse_theme_contents(source, source_identity)
        .map_err(|kind| CompileError::new(kind, source.full_span()))
}

/// Parses definitions and output positions from `source` without a reporting wrapper.
///
/// # Errors
///
/// Returns the parser and output-template failures documented by [`parse_theme`].
fn parse_theme_contents(
    source: &SourceFile,
    source_identity: &CssSourceName,
) -> Result<ThemeIndex, CompileDiagnostic> {
    if let Some(offset) = unterminated_block(source.text()) {
        return Err(CompileDiagnostic::UnterminatedCssBlock { offset });
    }
    if let Some(offset) = value_nesting_overflow(source.text()) {
        return Err(CompileDiagnostic::CssNestingTooDeep {
            offset,
            limit: MAX_VALUE_NESTING,
        });
    }
    let mut theme = extract_theme_tokens(source.id(), source.text())?;
    let mut template = parse_output_template(source.text(), &theme.tokens)?;
    template.map_origins(|span| Some(Span::new(source.id(), span.start.0, span.end.0)));
    template.source_identity = source_identity.clone();
    theme.custom_css = template.authored_css();
    StyleSheet::parse(&theme.custom_css, ParserOptions::default()).map_err(|error| {
        CompileDiagnostic::CssParse {
            detail: error.to_string(),
        }
    })?;
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
    theme.variant_positions = crate::registry::order::registration_positions(
        source.text(),
        &["@stillwind-variant", "@custom-variant", "@variant"],
    );
    theme.utility_positions = crate::registry::order::registration_positions(
        source.text(),
        &["@stillwind-utility", "@utility"],
    );

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
pub(crate) fn offset(value: usize) -> Result<u32, CompileDiagnostic> {
    u32::try_from(value).map_err(|_| CompileDiagnostic::CssOffsetTooLarge { offset: value })
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
pub(crate) fn extract_theme_tokens(
    file: FileId,
    css: &str,
) -> Result<ThemeIndex, CompileDiagnostic> {
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
        variant_positions: BTreeMap::new(),
        utility_positions: BTreeMap::new(),
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
    use stillwind_css::strip_css_comments;
    use stillwind_test_support::defaults::BUILTIN_REGISTRY_CSS;

    use super::*;

    #[test]
    fn resolved_css_uses_only_supplied_definitions_and_ignores_directive_text() {
        let source = SourceFile::new(FileId(1), None, None, format!("{BUILTIN_REGISTRY_CSS}\n/* @theme {{ --fake: red; }} */ .x{{content:'@theme';}} @theme {{ --color-local: blue; }} @stillwind-slot utilities;")).unwrap();
        let theme = parse_theme(&source, &CssSourceName::from("test.css")).unwrap();
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
                let source = SourceFile::new(FileId(1), None, None, css.as_str()).unwrap();
                for result in [
                    parse_theme(&source, &CssSourceName::from("test.css")),
                    parse_theme(&source, &CssSourceName::from("test.css")),
                ] {
                    assert!(
                        matches!(result, Err(CompileError { diagnostic: CompileDiagnostic::UnterminatedCssBlock { offset }, .. })
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
            None,
            None,
            r#"/* { */ .x { content: "{"; /* } */ &:hover { content: "}"; } }"#,
        )
        .unwrap();
        assert!(parse_theme(&source, &CssSourceName::from("test.css")).is_ok());
        assert!(parse_theme(&source, &CssSourceName::from("test.css")).is_ok());
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            "@utility x { color:red; &:hover { color:blue; } }",
        )
        .unwrap();
        assert_eq!(
            parse_theme(&source, &CssSourceName::from("test.css"))
                .unwrap()
                .utilities
                .len(),
            1
        );
    }

    #[test]
    fn nested_values_and_groups_past_the_limit_return_errors() {
        let levels = MAX_VALUE_NESTING + 1;
        let css = format!(
            "@theme {{ --x: calc({}1{}); }}",
            "(".repeat(levels),
            ")".repeat(levels)
        );
        let source = SourceFile::new(FileId(1), None, None, css.as_str()).unwrap();
        assert!(
            matches!(
                parse_theme(&source, &CssSourceName::from("test.css")),
                Err(CompileError {
                    diagnostic: CompileDiagnostic::CssNestingTooDeep { .. },
                    ..
                })
            ),
            "{css}"
        );

        let depth = 200;
        let mut css = String::from("@theme { --x: red; }");
        css.push_str(&"@layer x {".repeat(depth));
        css.push_str(&"}".repeat(depth));
        let source = SourceFile::new(FileId(1), None, None, css.as_str()).unwrap();
        assert!(
            matches!(
                parse_theme(&source, &CssSourceName::from("test.css")),
                Err(CompileError {
                    diagnostic: CompileDiagnostic::CssNestingTooDeep { .. },
                    ..
                })
            ),
            "{css}"
        );
    }
}
