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
use std::ops::Range;
use std::sync::Arc;

use lean_string::{LeanStr, LeanString, ToLeanStr};
use lightningcss::stylesheet::{ParserOptions, StyleSheet};

use stillwind_css::{
    CssOutputTemplate, MAX_VALUE_NESTING, extract_authored_property_names,
    extract_authored_property_syntaxes, extract_named_blocks_with_offsets,
    extract_named_statements, extract_property_definitions, unterminated_block,
    value_nesting_overflow,
};
use stillwind_diagnostics::{CompileDiagnostic, CompileError};
use stillwind_source::{CssSourceName, FileId, SourceFile, Span};

use super::builder::ThemeBuilder;
use super::declarations::{ThemeTokenMode, extract_theme_declarations};
use super::index::ThemeIndex;
use crate::output::{parse_css_templates, parse_output_template, theme_blocks};
use crate::registry::{
    extract_custom_utilities, extract_custom_variants, extract_declaration_formats,
    extract_marker_utilities, extract_relationship_patterns, registration_positions,
};
use crate::{BrowserTargets, BuiltinDefinitions, UtilityOrderMode};

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
    parse_theme_builder(source, source_identity, &[]).map(ThemeBuilder::finish)
}

/// Collects declarations from `source` without preparing evaluation data. The loader
/// can map token and template spans before freezing the result.
///
/// `plugin_ranges` contains the byte ranges of `source` that answer `@plugin`
/// statements. The `@utility` definitions inside them become plugin definitions, as
/// [`crate::CustomUtility::plugin`] describes.
///
/// # Errors
///
/// Returns the same CSS and definition failures as [`parse_theme`].
pub(crate) fn parse_theme_builder(
    source: &SourceFile,
    source_identity: &CssSourceName,
    plugin_ranges: &[Range<usize>],
) -> Result<ThemeBuilder, CompileError> {
    parse_theme_contents(source, source_identity, plugin_ranges)
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
    plugin_ranges: &[Range<usize>],
) -> Result<ThemeBuilder, CompileDiagnostic> {
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
    theme.custom_css = (template.authored_css()).into();
    StyleSheet::parse(&theme.custom_css, ParserOptions::default()).map_err(|error| {
        CompileDiagnostic::CssParse {
            detail: error.to_lean_str(),
        }
    })?;
    theme.output_template = template;
    theme.css_templates = parse_css_templates(source.text())?;
    theme.dynamic_utilities = extract_dynamic_utilities(source.text())?;
    theme.important = extract_important(source.text());
    for template in theme.css_templates.values_mut() {
        template.map_origins(|span| Some(Span::new(source.id(), span.start.0, span.end.0)));
        template.source_identity = source_identity.clone();
    }
    theme.registry = Arc::new(BuiltinDefinitions::from_css(source.text()));
    let in_plugin = |offset: usize| plugin_ranges.iter().any(|range| range.contains(&offset));
    let mut plugin_positions = BTreeMap::<LeanStr, usize>::new();
    theme.utilities = extract_custom_utilities(source.text())
        .into_iter()
        .map(|(offset, mut utility)| {
            if in_plugin(offset) {
                utility.plugin = true;
                plugin_positions
                    .entry(utility.name.clone())
                    .and_modify(|position| *position = (*position).min(offset))
                    .or_insert(offset);
            }
            utility
        })
        .collect();
    theme.variants = extract_custom_variants(source.text());
    theme.properties = extract_property_definitions(source.text());
    theme.marker_utilities = extract_marker_utilities(source.text());
    theme.declaration_formats = extract_declaration_formats(source.text());
    theme.relationship_patterns = extract_relationship_patterns(source.text());
    theme.authored_properties = extract_authored_property_names(source.text());
    theme.authored_property_syntaxes = extract_authored_property_syntaxes(source.text());
    theme.variant_positions = registration_positions(
        source.text(),
        &["@stillwind-variant", "@custom-variant", "@variant"],
    );
    theme.utility_positions =
        registration_positions(source.text(), &["@stillwind-utility", "@utility"]);
    // A plugin supplies defaults that built-ins and the stylesheet can override, so its
    // utilities take positions before every other utility. Offsets stay below the
    // length of the text, which therefore separates the two groups.
    if !plugin_positions.is_empty() {
        theme.utility_position_base = source.text().len();
        for (name, position) in &mut theme.utility_positions {
            *position = plugin_positions
                .get(name)
                .copied()
                .unwrap_or_else(|| position.saturating_add(theme.utility_position_base));
        }
    }

    Ok(theme)
}

/// A utility family that a host plugin defines on demand, declared by
/// `@stillwind-dynamic <prefix>*;` for named candidates such as `icon-mdi--close`, and
/// by `@stillwind-dynamic <prefix>[*];` for bracket candidates such as
/// `icon-[mdi--close]`.
///
/// A family that accepts both forms resolves a bracket candidate like its named
/// spelling, so both spellings share one definition. A family that accepts only the
/// bracket form leaves named candidates to other definitions, and the plugin defines
/// each bracket candidate under its bracket name, such as `@utility icon-[mdi--close]`.
///
/// The Tailwind plugin adapter declares its families with `@stillwind-plugin-family
/// utility <root>;` and `@stillwind-plugin-family variant <root>;`. Their prefix is the
/// root without a trailing `-`, and they accept both forms.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicFamily {
    /// Registration protocol and whether the host resolves utilities or variants.
    pub kind: DynamicFamilyKind,
    /// Nonempty name prefix without whitespace, `*`, `[`, or `]`, such as `icon-`.
    pub prefix: LeanStr,
    /// Whether named candidates belong to the family.
    pub named: bool,
    /// Whether bracket candidates belong to the family.
    pub bracket: bool,
}

/// The `important` setting of a Tailwind plugin configuration, declared by
/// `@stillwind-plugin-important;` for `true` and by `@stillwind-plugin-important
/// <selector>;` for a selector. It applies to the class utilities that the compiler
/// generates, including built-ins, and not to `@apply`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UtilityImportance {
    /// Every declaration of a class utility is `!important`.
    Declarations,
    /// Each class utility rule is nested in a rule with this selector, so `#app`
    /// selects `.x` as `#app .x`. The text is trimmed and nonempty.
    Selector(LeanStr),
}

/// Reads the last `@stillwind-plugin-important` statement, because a later Tailwind
/// configuration replaces the `important` setting of an earlier one.
fn extract_important(css: &str) -> Option<UtilityImportance> {
    extract_named_statements(css, "@stillwind-plugin-important")
        .pop()
        .map(|(_, header)| {
            let selector = header.trim();
            if selector.is_empty() {
                UtilityImportance::Declarations
            } else {
                UtilityImportance::Selector(selector.to_lean_str())
            }
        })
}

/// Host protocols for dynamically registered candidates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DynamicFamilyKind {
    /// Native plugin utility names, normalized according to the family's accepted forms.
    Native,
    /// Tailwind utility keys with negative values and modifiers retained.
    Utility,
    /// Tailwind variant names with values and modifiers retained.
    Variant,
}

/// Extracts the families of `@stillwind-dynamic` statements in the order of their first
/// statements, followed by one family for each `@stillwind-plugin-family` statement.
/// `@stillwind-dynamic` statements with the same prefix merge into one family.
///
/// # Errors
///
/// Returns [`CompileDiagnostic::InvalidDirectiveForm`] for a `@stillwind-dynamic`
/// block, for a `@stillwind-dynamic` header that is not one name ending in `*` or `[*]`
/// after a nonempty prefix, and for a `@stillwind-plugin-family` header that is not
/// `utility` or `variant` followed by one root.
fn extract_dynamic_utilities(css: &str) -> Result<Vec<DynamicFamily>, CompileDiagnostic> {
    if let Some((offset, _, _)) = extract_named_blocks_with_offsets(css, DYNAMIC).first() {
        return Err(CompileDiagnostic::InvalidDirectiveForm {
            offset: *offset,
            directive: "stillwind-dynamic".into(),
        });
    }
    let mut families: Vec<DynamicFamily> = Vec::new();
    for (offset, header) in extract_named_statements(css, DYNAMIC) {
        let (prefix, bracket) = header
            .strip_suffix("[*]")
            .map(|prefix| (prefix, true))
            .or_else(|| header.strip_suffix('*').map(|prefix| (prefix, false)))
            .filter(|(prefix, _)| {
                !prefix.is_empty()
                    && !prefix.contains(|character: char| {
                        character.is_whitespace() || matches!(character, '*' | '[' | ']')
                    })
            })
            .ok_or(CompileDiagnostic::InvalidDirectiveForm {
                offset,
                directive: "stillwind-dynamic".into(),
            })?;
        let index = families
            .iter()
            .position(|family| family.prefix == prefix)
            .unwrap_or_else(|| {
                families.push(DynamicFamily {
                    kind: DynamicFamilyKind::Native,
                    prefix: prefix.to_lean_str(),
                    named: false,
                    bracket: false,
                });
                families.len() - 1
            });
        if bracket {
            families[index].bracket = true;
        } else {
            families[index].named = true;
        }
    }
    for (offset, header) in extract_named_statements(css, "@stillwind-plugin-family") {
        let words = header.split_whitespace().collect::<Vec<_>>();
        let kind = match words.first().copied() {
            Some("utility") => DynamicFamilyKind::Utility,
            Some("variant") => DynamicFamilyKind::Variant,
            _ => {
                return Err(CompileDiagnostic::InvalidDirectiveForm {
                    offset,
                    directive: "stillwind-plugin-family".into(),
                });
            }
        };
        let prefix = words.get(1).filter(|_| words.len() == 2).ok_or(
            CompileDiagnostic::InvalidDirectiveForm {
                offset,
                directive: "stillwind-plugin-family".into(),
            },
        )?;
        families.push(DynamicFamily {
            kind,
            prefix: (*prefix).into(),
            named: true,
            bracket: true,
        });
    }
    Ok(families)
}

/// At-rule that declares a utility family defined on demand by a host plugin.
const DYNAMIC: &str = "@stillwind-dynamic";

/// Extracts the top-level `@keyframes` rules of a `@theme` body, in authored order.
///
/// Keyframes cannot be represented as custom properties, so they are copied textually
/// and emitted with the theme layer. An unterminated keyframes block ends the scan
/// rather than being emitted partially.
pub(crate) fn extract_keyframe_blocks(css: &str) -> LeanString {
    let mut output = LeanString::new();
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
/// Only tokens are filled in; the remaining builder fields are defaults, because
/// they depend on the whole buffer and are assembled by [`parse_theme`].
///
/// # Errors
///
/// Returns a block or offset error, or an unterminated `@theme` block error.
fn extract_theme_tokens(file: FileId, css: &str) -> Result<ThemeBuilder, CompileDiagnostic> {
    let mut tokens = Vec::new();
    for (range, modifier, precedence) in theme_blocks(css)? {
        let mode = ThemeTokenMode::parse(&modifier);
        let mut declarations =
            extract_theme_declarations(file, &css[range.clone()], range.start, mode)?;
        for token in &mut declarations {
            token.precedence = precedence;
        }
        tokens.extend(declarations);
    }
    Ok(ThemeBuilder {
        variant_positions: BTreeMap::new(),
        utility_positions: BTreeMap::new(),
        utility_position_base: 0,
        output_template: CssOutputTemplate::default(),
        css_templates: BTreeMap::new(),
        dynamic_utilities: Vec::new(),
        important: None,
        registry: Arc::default(),
        tokens,
        custom_css: (LeanString::new()).into(),
        utilities: Vec::new(),
        variants: Vec::new(),
        properties: Vec::new(),
        marker_utilities: BTreeSet::new(),
        declaration_formats: BTreeMap::new(),
        relationship_patterns: BTreeSet::new(),
        authored_properties: BTreeSet::new(),
        authored_property_syntaxes: BTreeMap::new(),
        utility_order: UtilityOrderMode::default(),
        target: BrowserTargets::default(),
    })
}

#[cfg(test)]
mod tests {
    use stillwind_css::strip_css_comments;
    use stillwind_test_support::BUILTIN_REGISTRY_CSS;

    use super::*;

    #[test]
    fn resolved_css_uses_only_supplied_definitions_and_ignores_directive_text() {
        let source = SourceFile::new(FileId(1), None, None, std::format!("{BUILTIN_REGISTRY_CSS}\n/* @theme {{ --fake: red; }} */ .x{{content:'@theme';}} @theme {{ --color-local: blue; }} @stillwind-slot utilities;")).unwrap();
        let theme = parse_theme(&source, &CssSourceName::from("test.css")).unwrap();
        assert_eq!(theme.tokens().len(), 1);
        assert_eq!(theme.tokens()[0].name.value, "--color-local");
        assert!(
            theme
                .registry()
                .utilities()
                .iter()
                .any(|utility| utility.name == "flex")
        );
        assert!(theme.custom_css().contains("content:'@theme'"));
        assert!(
            !strip_css_comments(theme.custom_css()).contains("@stillwind"),
            "{}",
            theme.custom_css()
        );
    }

    #[test]
    fn dynamic_families_merge_their_forms_and_normalize_only_shared_brackets() {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            "@stillwind-dynamic icon-*; @stillwind-dynamic i-[*]; @stillwind-dynamic icon-[*]; @stillwind-dynamic icon-*; @stillwind-slot utilities;",
        )
        .unwrap();
        let theme = parse_theme(&source, &CssSourceName::from("test.css")).unwrap();
        let family = |prefix: &str, named, bracket| DynamicFamily {
            kind: DynamicFamilyKind::Native,
            prefix: prefix.into(),
            named,
            bracket,
        };
        assert_eq!(
            theme.dynamic_utilities(),
            [family("icon-", true, true), family("i-", false, true)]
        );
        assert!(!theme.custom_css().contains("@stillwind-dynamic"));
        assert_eq!(
            theme.dynamic_named_utility("icon-[mdi--close]").as_deref(),
            Some("icon-mdi--close")
        );
        assert_eq!(
            theme.dynamic_family("i-[mdi--close]"),
            Some((&family("i-", false, true), true))
        );
        for name in [
            "i-[mdi--close]",
            "icon-mdi--close",
            "icon-[]",
            "icon-[url:x]",
            "icon-[a_b c]",
            "other-[mdi--close]",
        ] {
            assert_eq!(theme.dynamic_named_utility(name), None, "{name}");
        }
        for name in ["i-mdi--close", "i-", "icon-", "i-[]"] {
            assert_eq!(theme.dynamic_family(name), None, "{name}");
        }
    }

    #[test]
    fn token_precedence_outranks_source_order() {
        let value = |css: &str| {
            let source = SourceFile::new(FileId(1), None, None, css).unwrap();
            parse_theme(&source, &CssSourceName::from("test.css"))
                .unwrap()
                .token("color", "brand")
                .map(|token| token.value.value.to_string())
        };
        assert_eq!(
            value("@theme { --color-brand: red; } @theme default { --color-brand: blue; }"),
            Some("red".into())
        );
        assert_eq!(
            value(
                "@theme default { --color-brand: blue; } @stillwind-plugin-theme { --color-brand: green; }"
            ),
            Some("green".into())
        );
        assert_eq!(
            value(
                "@theme { --color-brand: red; } @stillwind-plugin-theme { --color-brand: green; }"
            ),
            Some("red".into())
        );
        // A reset removes definitions of its own and lower precedence only.
        assert_eq!(
            value("@theme { --color-brand: red; } @theme default { --color-*: initial; }"),
            Some("red".into())
        );
        assert_eq!(
            value("@theme default { --color-brand: blue; } @theme { --color-*: initial; }"),
            None
        );
    }

    #[test]
    fn malformed_dynamic_directives_are_rejected() {
        for css in [
            "@stillwind-dynamic icon-;",
            "@stillwind-dynamic *;",
            "@stillwind-dynamic [*];",
            "@stillwind-dynamic icon-[x*];",
            "@stillwind-dynamic icon- *;",
            "@stillwind-dynamic icon-* { }",
        ] {
            let source = SourceFile::new(FileId(1), None, None, css).unwrap();
            let error = parse_theme(&source, &CssSourceName::from("test.css")).unwrap_err();
            assert!(
                matches!(
                    error.diagnostic,
                    CompileDiagnostic::InvalidDirectiveForm { .. }
                ),
                "{css}"
            );
        }
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
    use lean_string::ToLeanString;

    use super::*;

    #[test]
    fn unclosed_definition_blocks_return_errors() {
        for directive in ["@utility x", "@custom-variant x", "@stillwind-variant x"] {
            for body in ["", " color:red;", " &:hover {color:red;}"] {
                let css = format_args!("{directive} {{{body}").to_lean_string();
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
                .utilities()
                .len(),
            1
        );
    }

    #[test]
    fn nested_values_and_groups_past_the_limit_return_errors() {
        let levels = MAX_VALUE_NESTING + 1;
        let css = format_args!(
            "@theme {{ --x: calc({}1{}); }}",
            "(".repeat(levels),
            ")".repeat(levels)
        )
        .to_lean_string();
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
        let mut css = LeanString::from("@theme { --x: red; }");
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
