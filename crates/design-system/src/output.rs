//! Parsing of the host's Stillwind CSS into a structured output template.
//!
//! [`parse_output_template`] turns one CSS buffer into source-ordered authored nodes,
//! definition references, wrappers, and output slots. The caller supplies tokens
//! extracted from the same buffer; template theme nodes refer to that token list by
//! index. Origins address the input buffer and can be remapped by the source loader.
//!
//! Theme and property-initial slots accept declarations inside style rules. Utilities,
//! keyframes, and registration slots accept rules. Parsing checks those contexts but
//! does not select live tokens or serialize a producer's generated CSS.
//!
//! Definitions are compile-time only: an `@theme`, `@utility`, or `@custom-variant`
//! nested inside a conditional is rejected, because emitting it would make the registry
//! depend on runtime conditions. `@stillwind-slot` is the exception and may be nested.

use std::collections::BTreeMap;
use std::ops::Range;

use cssparser::{ParseError, Parser, Token};
use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{
    CssOutputNode, CssOutputTemplate, PropertyInheritance, extract_named_blocks_with_offsets,
    extract_named_statements, matching_brace,
};
use stillwind_diagnostics::CompileDiagnostic;
use stillwind_source::{CssSourceName, FileId, Span};

use crate::theme::extract_keyframe_blocks;
use crate::{ThemeToken, ThemeTokenPrecedence};

/// Deepest group nesting accepted in one authored buffer.
///
/// Parsing recurses once per nested group, so this bounds the stack one stylesheet can
/// consume; deeper input is reported as [`CompileDiagnostic::CssNestingTooDeep`] instead
/// of exhausting the stack.
const MAX_BLOCK_NESTING: usize = 64;

/// Parses one CSS buffer into a source-ordered output template.
///
/// `tokens` are the already extracted theme tokens; each `@theme` node records the
/// indices of the tokens whose name spans fall inside that block. Their offsets must
/// address `css`. Returned origins use `FileId(0)` and UTF-8 byte offsets into `css`;
/// the caller assigns the template's initially empty source identity and remaps origins.
///
/// # Errors
///
/// Returns an error for malformed blocks, offsets outside the 32-bit source range,
/// conditional definitions, unknown slots, or slots in the wrong CSS context.
pub fn parse_output_template(
    css: &str,
    tokens: &[ThemeToken],
) -> Result<CssOutputTemplate, CompileDiagnostic> {
    let nodes = parse_nodes(css, 0, tokens, &mut Vec::new(), false, 0)?;
    validate_slots(&nodes, false)?;
    Ok(CssOutputTemplate {
        source_identity: CssSourceName::default(),
        nodes,
    })
}

/// Parses every `@stillwind-css-template <name> { ... }` rule in one CSS buffer.
///
/// Each body uses the slot syntax of the output template and describes where the
/// output of one `style.css()` call goes. A later rule with the same name replaces an
/// earlier one, so a project can redefine a built-in template. Origins use `FileId(0)`
/// and UTF-8 byte offsets into `css`, as [`parse_output_template`] describes.
///
/// # Errors
///
/// Returns [`CompileDiagnostic::InvalidCssTemplate`] for a rule without one name and a
/// block, [`CompileDiagnostic::DefinitionInCssTemplate`] for a definition such as
/// `@theme` inside a body, and [`CompileDiagnostic::CssTemplateWithoutUtilities`] for a
/// body without a utilities slot. Body syntax fails as [`parse_output_template`] does.
pub fn parse_css_templates(
    css: &str,
) -> Result<BTreeMap<LeanStr, CssOutputTemplate>, CompileDiagnostic> {
    if let Some((offset, _)) = extract_named_statements(css, CSS_TEMPLATE).first() {
        return Err(CompileDiagnostic::InvalidCssTemplate { offset: *offset });
    }
    let mut templates = BTreeMap::new();
    for (offset, header, _) in extract_named_blocks_with_offsets(css, CSS_TEMPLATE) {
        let mut parser = Parser::new(&header);
        let name = match parser.next() {
            Ok(Token::Ident(name)) => name.to_lean_str(),
            _ => return Err(CompileDiagnostic::InvalidCssTemplate { offset }),
        };
        if !parser.is_exhausted() {
            return Err(CompileDiagnostic::InvalidCssTemplate { offset });
        }
        let open = css[offset..]
            .find('{')
            .map(|position| offset + position)
            .ok_or(CompileDiagnostic::InvalidCssTemplate { offset })?;
        let close = matching_brace(css, open)
            .map_err(|_| CompileDiagnostic::InvalidCssBlock { offset: open })?;
        let body = &css[open + 1..close];
        if let Some((position, directive)) = first_definition(body) {
            return Err(CompileDiagnostic::DefinitionInCssTemplate {
                offset: open + 1 + position,
                directive,
            });
        }
        let nodes = parse_nodes(body, open + 1, &[], &mut Vec::new(), false, 0)?;
        validate_slots(&nodes, false)?;
        if !contains_utilities(&nodes) {
            return Err(CompileDiagnostic::CssTemplateWithoutUtilities { name });
        }
        templates.insert(
            name,
            CssOutputTemplate {
                source_identity: CssSourceName::default(),
                nodes,
            },
        );
    }
    Ok(templates)
}

/// At-rule that defines a named template for `style.css()`.
const CSS_TEMPLATE: &str = "@stillwind-css-template";

/// Returns the byte offset and name of the first definition at-rule in `css`, searching
/// nested blocks. Output slots are not definitions.
fn first_definition(css: &str) -> Option<(usize, LeanStr)> {
    /// Returns the first definition at-rule from the parser's position, entering nested
    /// blocks. Positions of a nested parser index the same input as the outer parser.
    fn scan(parser: &mut Parser<'_>) -> Option<(usize, LeanStr)> {
        loop {
            let start = parser.position().byte_index();
            match parser.next_including_whitespace_and_comments().ok()? {
                Token::AtKeyword(name) if is_definition(name) && &**name != "stillwind-slot" => {
                    return Some((start, name.to_lean_str()));
                }
                Token::CurlyBracketBlock => {
                    // A nested block must be consumed entirely, or its result is lost.
                    if let Some(found) = parser
                        .parse_nested_block(|nested| {
                            let found = scan(nested);
                            while nested.next_including_whitespace_and_comments().is_ok() {}
                            Ok::<_, ParseError<()>>(found)
                        })
                        .ok()
                        .flatten()
                    {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
    }
    scan(&mut Parser::new(css))
}

/// Whether `nodes` contain a utilities slot at any depth.
fn contains_utilities(nodes: &[CssOutputNode]) -> bool {
    nodes.iter().any(|node| match node {
        CssOutputNode::Utilities => true,
        CssOutputNode::Group { children, .. } | CssOutputNode::Style { children, .. } => {
            contains_utilities(children)
        }
        _ => false,
    })
}

/// Inner byte range, modifier header, and token precedence of one theme block.
pub(crate) type ThemeBlock = (Range<usize>, LeanString, ThemeTokenPrecedence);

/// Collects the inner byte range, the modifier header, and the token precedence of
/// every `@theme` and `@stillwind-plugin-theme` block, in source order.
///
/// Callers use this to extract declarations per block, so a block's mode (`inline`,
/// `static`, or default) and precedence can be attributed to its own tokens. A plugin
/// theme block accepts the same mode modifiers and always has plugin precedence.
///
/// # Errors
///
/// Returns a block error for unparsable or unterminated `@theme` syntax.
pub(crate) fn theme_blocks(css: &str) -> Result<Vec<ThemeBlock>, CompileDiagnostic> {
    let mut blocks = Vec::new();
    parse_nodes(css, 0, &[], &mut blocks, false, 0)?;
    Ok(blocks)
}

/// Whether an at-keyword defines compile-time semantics and therefore may not appear
/// inside a conditional group.
fn is_definition(name: &str) -> bool {
    name.starts_with("stillwind-")
        || matches!(
            name,
            "theme" | "utility" | "custom-variant" | "variant" | "tailwind"
        )
}

/// Consumes the block or statement that follows an at-keyword.
///
/// Returns the inner range for a `{}` block and `None` when the construct ends at a
/// semicolon or cannot be parsed, which the caller treats as a statement.
///
/// # Errors
///
/// Returns a block error when a nested block cannot be parsed.
fn consume_block(
    parser: &mut Parser<'_>,
    base: usize,
) -> Result<Option<(usize, usize)>, CompileDiagnostic> {
    loop {
        match parser.next_including_whitespace_and_comments() {
            Ok(Token::CurlyBracketBlock) => {
                let position = parser.position().byte_index() - 1;
                parser
                    .parse_nested_block(|nested| {
                        while nested.next_including_whitespace_and_comments().is_ok() {}
                        Ok::<_, ParseError<()>>(())
                    })
                    .map_err(|_| CompileDiagnostic::InvalidCssBlock {
                        offset: base + position,
                    })?;
                return Ok(Some((position, parser.position().byte_index() - 1)));
            }
            Ok(Token::Semicolon) | Err(_) => return Ok(None),
            Ok(_) => {}
        }
    }
}

/// Parses one nesting level into output nodes.
///
/// `base` is the byte offset of `css` inside the root buffer, so every origin span and
/// error offset refers to the root buffer rather than to the slice. `conditional`
/// records whether an enclosing group is conditional; a definition below such a group
/// is rejected. `depth` counts the groups already open around this level.
///
/// # Errors
///
/// Returns an error for conditional definitions, malformed blocks, unknown slots, and
/// offsets that exceed the 32-bit source limit, or when the buffer nests groups deeper
/// than [`MAX_BLOCK_NESTING`].
#[allow(clippy::too_many_lines)] // One dispatch keeps source ranges aligned for authored rules, definitions, and slots.
fn parse_nodes(
    css: &str,
    base: usize,
    tokens: &[ThemeToken],
    theme_blocks: &mut Vec<ThemeBlock>,
    conditional: bool,
    depth: usize,
) -> Result<Vec<CssOutputNode>, CompileDiagnostic> {
    if depth > MAX_BLOCK_NESTING {
        return Err(CompileDiagnostic::CssNestingTooDeep {
            offset: base,
            limit: MAX_BLOCK_NESTING,
        });
    }
    let mut parser = Parser::new(css);
    let mut nodes = Vec::new();
    while !parser.is_exhausted() {
        let start = parser.position().byte_index();
        let Ok(first) = parser.next() else { break };
        let name = match first {
            Token::AtKeyword(name) => Some(name.to_lean_string()),
            _ => None,
        };
        let header_end = parser.position().byte_index();
        if conditional
            && let Some(name) = name.as_deref()
            && is_definition(name)
            && name != "stillwind-slot"
        {
            return Err(CompileDiagnostic::ConditionalDefinition {
                offset: base + start,
                directive: name.into(),
            });
        }
        let block = consume_block(&mut parser, base)?;
        let end = parser.position().byte_index();
        let origin = Some(Span::new(
            FileId(0),
            u32::try_from(base + start).map_err(|_| CompileDiagnostic::CssOffsetTooLarge {
                offset: base + start,
            })?,
            u32::try_from(base + end)
                .map_err(|_| CompileDiagnostic::CssOffsetTooLarge { offset: base + end })?,
        ));
        match (name.as_deref(), block) {
            (Some("layer"), None) => nodes.push(CssOutputNode::LayerOrder {
                css: css[start..end].into(),
                origin,
            }),
            (Some(directive @ ("stillwind-slot" | "theme")), block)
                if (directive == "theme") != block.is_some() =>
            {
                return Err(CompileDiagnostic::InvalidDirectiveForm {
                    offset: base + start,
                    directive: directive.into(),
                });
            }
            (Some("stillwind-slot"), None) => {
                let value = css[header_end..end].trim().trim_end_matches(';').trim();
                // Arguments may be separated by any CSS whitespace.
                let words = value.split_whitespace().collect::<Vec<_>>().join(" ");
                let node = match words.as_str() {
                    "utilities" => CssOutputNode::Utilities,
                    "theme" => CssOutputNode::ThemeVariables,
                    "keyframes" => CssOutputNode::Keyframes,
                    "properties" => CssOutputNode::Properties,
                    "property-initials" => {
                        CssOutputNode::PropertyInitials(PropertyInheritance::All)
                    }
                    "property-initials inherited" => {
                        CssOutputNode::PropertyInitials(PropertyInheritance::Inherited)
                    }
                    "property-initials non-inherited" => {
                        CssOutputNode::PropertyInitials(PropertyInheritance::NonInherited)
                    }
                    _ => {
                        return Err(CompileDiagnostic::UnsupportedOutputSlot {
                            offset: base + start,
                            slot: value.into(),
                        });
                    }
                };
                nodes.push(node);
            }
            (Some(directive @ ("theme" | "stillwind-plugin-theme")), Some((open, close))) => {
                let modifier = css[header_end..open].trim();
                let precedence = if directive == "stillwind-plugin-theme" {
                    ThemeTokenPrecedence::Plugin
                } else {
                    ThemeTokenPrecedence::parse(modifier)
                };
                let modifier = LeanString::from(modifier);
                theme_blocks.push((base + open + 1..base + close, modifier, precedence));
                nodes.push(CssOutputNode::Theme {
                    origin,
                    tokens: tokens
                        .iter()
                        .enumerate()
                        .filter_map(|(index, token)| {
                            let position = token.name.span.start.0 as usize;
                            (base + open < position && position < base + close).then_some(index)
                        })
                        .collect(),
                    keyframes: (extract_keyframe_blocks(&css[open + 1..close])).into(),
                });
            }
            (
                Some("layer" | "supports" | "media" | "container" | "scope" | "starting-style"),
                Some((open, close)),
            ) => {
                nodes.push(CssOutputNode::Group {
                    origin,
                    header: css[start..open].trim().into(),
                    children: parse_nodes(
                        &css[open + 1..close],
                        base + open + 1,
                        tokens,
                        theme_blocks,
                        conditional || name.as_deref() != Some("layer"),
                        depth + 1,
                    )?,
                });
            }
            (None, Some((open, close))) if css[open + 1..close].contains("@stillwind-slot") => {
                let children = parse_nodes(
                    &css[open + 1..close],
                    base + open + 1,
                    tokens,
                    theme_blocks,
                    conditional,
                    depth + 1,
                )?;
                if children.iter().any(|node| {
                    matches!(
                        node,
                        CssOutputNode::ThemeVariables
                            | CssOutputNode::PropertyInitials(_)
                            | CssOutputNode::Utilities
                            | CssOutputNode::Keyframes
                            | CssOutputNode::Properties
                            | CssOutputNode::Style { .. }
                    )
                }) {
                    if children.iter().any(|node| {
                        !matches!(
                            node,
                            CssOutputNode::Authored { .. }
                                | CssOutputNode::ThemeVariables
                                | CssOutputNode::PropertyInitials(_)
                        )
                    }) {
                        return Err(CompileDiagnostic::InvalidSlotStyleRule {
                            offset: base + start,
                        });
                    }
                    nodes.push(CssOutputNode::Style {
                        header: css[start..open].trim().into(),
                        children,
                        origin,
                    });
                } else {
                    nodes.push(CssOutputNode::Authored {
                        css: css[start..end].into(),
                        origin,
                    });
                }
            }
            (Some(name), _) if is_definition(name) => {}
            _ => nodes.push(CssOutputNode::Authored {
                css: css[start..end].into(),
                origin,
            }),
        }
    }
    Ok(nodes)
}

/// Recursively checks that each slot emits the kind of CSS accepted by its context.
///
/// `declarations` means the current node list accepts declarations. Style-rule children
/// enter declaration context; group children inherit their parent context. Nodes are not modified.
///
/// # Errors
///
/// Returns [`CompileDiagnostic::InvalidOutputSlotContext`] at the first mismatched slot.
fn validate_slots(nodes: &[CssOutputNode], declarations: bool) -> Result<(), CompileDiagnostic> {
    for node in nodes {
        let (name, needs_declarations) = match node {
            CssOutputNode::ThemeVariables => ("theme", true),
            CssOutputNode::PropertyInitials(_) => ("property-initials", true),
            CssOutputNode::Utilities => ("utilities", false),
            CssOutputNode::Keyframes => ("keyframes", false),
            CssOutputNode::Properties => ("properties", false),
            CssOutputNode::Style { children, .. } => {
                validate_slots(children, true)?;
                continue;
            }
            CssOutputNode::Group { children, .. } => {
                validate_slots(children, declarations)?;
                continue;
            }
            _ => continue,
        };
        if declarations != needs_declarations {
            return Err(CompileDiagnostic::InvalidOutputSlotContext { slot: name.into() });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use lean_string::ToLeanStr;

    use super::*;

    #[test]
    fn declaration_slots_require_a_style_rule_and_ignore_quoted_markers() {
        for css in [
            "@stillwind-slot theme;",
            "@layer a{@stillwind-slot property-initials;}",
            ".x{@stillwind-slot utilities;}",
        ] {
            assert!(parse_output_template(css, &[]).is_err(), "{css}");
        }
        let css = ".x{content:'@stillwind-slot theme;'}";
        assert_eq!(parse_output_template(css, &[]).unwrap().authored_css(), css);
        let nodes = parse_output_template(
            "@media print{.scope{before:1;@stillwind-slot theme;after:2;}}",
            &[],
        )
        .unwrap();
        assert!(
            matches!(&nodes.nodes[0], CssOutputNode::Group { children, .. } if matches!(&children[0], CssOutputNode::Style { .. }))
        );
    }

    /// Each slot keyword maps to its node, statements keep their own form, and the
    /// origin span covers the whole directive.
    #[test]
    fn parse_nodes_maps_every_slot_and_statement_form() {
        let nodes = parse_output_template("@layer a, b; .x{color:red}", &[]).unwrap();
        assert!(matches!(&nodes.nodes[0], CssOutputNode::LayerOrder { .. }));
        assert!(matches!(&nodes.nodes[1], CssOutputNode::Authored { .. }));
        // The origin covers the whole directive, not just its header.
        assert_eq!(
            nodes.nodes[0],
            CssOutputNode::LayerOrder {
                css: "@layer a, b;".to_lean_str(),
                origin: Some(Span::new(
                    FileId(0),
                    0,
                    u32::try_from("@layer a, b;".len()).unwrap(),
                )),
            }
        );
        // Declaration slots require a style rule and keep it as a Style node.
        for slot in ["theme", "property-initials"] {
            let css =
                format_args!("@media print{{.scope{{before:1;@stillwind-slot {slot};after:2;}}}}")
                    .to_lean_string();
            let nodes = parse_output_template(&css, &[])
                .unwrap_or_else(|error| panic!("{slot}: {error:?}"));
            assert!(
                matches!(
                    &nodes.nodes[0],
                    CssOutputNode::Group { children, .. } if matches!(&children[0], CssOutputNode::Style { .. })
                ),
                "{slot}"
            );
        }
        // Structural slots are document-level statements with their own node kind.
        for (slot, expected) in [
            ("utilities", CssOutputNode::Utilities),
            ("keyframes", CssOutputNode::Keyframes),
            ("properties", CssOutputNode::Properties),
        ] {
            let css = format_args!("@stillwind-slot {slot};").to_lean_string();
            let nodes = parse_output_template(&css, &[])
                .unwrap_or_else(|error| panic!("{slot}: {error:?}"));
            assert_eq!(nodes.nodes, [expected], "{slot}");
        }
        // A declaration slot outside a style rule is a context error.
        assert!(matches!(
            parse_output_template("@stillwind-slot theme;", &[]),
            Err(CompileDiagnostic::InvalidOutputSlotContext { slot }) if slot == "theme"
        ));
    }

    /// Unknown slots, block-form slots, statement-form themes, and definitions under a
    /// condition are rejected with the offset of the offending directive.
    #[test]
    fn parse_nodes_rejects_invalid_directive_forms() {
        let css = "@stillwind-slot unknown;";
        assert!(matches!(
            parse_output_template(css, &[]),
            Err(CompileDiagnostic::UnsupportedOutputSlot { offset, slot })
                if offset == 0 && slot == "unknown"
        ));
        for (css, expected) in [
            ("@stillwind-slot theme {}", "InvalidDirectiveForm"),
            ("@theme ;", "InvalidDirectiveForm"),
            (
                "@media print { @theme { --a: 1; } }",
                "ConditionalDefinition",
            ),
            (
                "@supports (display:grid) { @utility x { color: red; } }",
                "ConditionalDefinition",
            ),
        ] {
            let error = parse_output_template(css, &[]).expect_err(css);
            assert!(
                format_args!("{error:?}")
                    .to_lean_string()
                    .contains(expected),
                "{css}: {error:?}"
            );
        }
        assert!(matches!(
            parse_output_template("@media print { @theme { --a: 1; } }", &[]),
            Err(CompileDiagnostic::ConditionalDefinition { directive, .. }) if directive == "theme"
        ));
    }

    /// Group nesting is bounded, and the limit is reported as a nesting error.
    #[test]
    fn parse_nodes_bounds_group_nesting() {
        let shallow = format_args!(
            "{}.x{{color:red}}{}",
            "@media a{".repeat(MAX_BLOCK_NESTING),
            "}".repeat(2)
        )
        .to_lean_string();
        assert!(parse_output_template(&shallow, &[]).is_ok());
        let deep = format_args!(
            "{}.x{{color:red}}{}",
            "@media a{".repeat(MAX_BLOCK_NESTING + 1),
            "}".repeat(MAX_BLOCK_NESTING + 1)
        )
        .to_lean_string();
        assert!(matches!(
            parse_output_template(&deep, &[]),
            Err(CompileDiagnostic::CssNestingTooDeep { limit, .. }) if limit == MAX_BLOCK_NESTING
        ));
    }

    #[test]
    fn preserves_conditions_and_rules_after_metadata_statements() {
        let template = parse_output_template("@stillwind-marker group; .before{content:'@theme { ; }';} @layer x { @supports (display:grid) { @stillwind-slot utilities; } .after{color:red} }", &[]).unwrap();
        assert_eq!(template.nodes.len(), 2);
        assert!(
            matches!(&template.nodes[0], CssOutputNode::Authored {css,..} if css.contains("content:"))
        );
        let CssOutputNode::Group {
            header, children, ..
        } = &template.nodes[1]
        else {
            panic!("layer")
        };
        assert_eq!(header, "@layer x");
        assert_eq!(children.len(), 2);
        assert!(
            matches!(&children[0], CssOutputNode::Group { children, .. } if children == &[CssOutputNode::Utilities])
        );
        assert!(matches!(&children[1], CssOutputNode::Authored {css,..} if css.contains(".after")));
    }

    #[test]
    fn comments_and_strings_are_not_directives_and_unknown_slots_fail() {
        assert_eq!(
            parse_output_template("/* @theme { } */ .x{content:';{}';}", &[])
                .unwrap()
                .nodes
                .len(),
            1
        );
        assert!(matches!(
            parse_output_template("@stillwind-slot other;", &[]),
            Err(CompileDiagnostic::UnsupportedOutputSlot { .. })
        ));
    }

    #[test]
    fn conditional_slots_are_supported_but_conditional_semantics_are_diagnosed() {
        assert!(
            parse_output_template(
                "@supports (display:grid){@media print{@stillwind-slot utilities;}}",
                &[]
            )
            .is_ok()
        );
        for css in [
            "@supports (display:grid){@theme {--color-brand:blue;}}",
            "@media print{@layer test{@stillwind-utility flex{display:flex;}}}",
        ] {
            assert!(matches!(
                parse_output_template(css, &[]),
                Err(CompileDiagnostic::ConditionalDefinition { .. })
            ));
        }
    }

    #[test]
    fn css_templates_are_named_and_later_definitions_win() {
        let css = ".page{color:red} \
            @stillwind-css-template shadow { :host { @stillwind-slot theme; } @stillwind-slot utilities; } \
            @layer x { @stillwind-css-template plain { @stillwind-slot utilities; } } \
            @stillwind-css-template shadow { @layer a { @stillwind-slot utilities; } \
              :where(*) { @stillwind-slot property-initials   non-inherited; } }";
        let templates = parse_css_templates(css).unwrap();
        assert_eq!(
            templates.keys().map(LeanStr::as_str).collect::<Vec<_>>(),
            ["plain", "shadow"]
        );
        let shadow = &templates["shadow"].nodes;
        assert!(matches!(&shadow[0], CssOutputNode::Group { header, .. } if header == "@layer a"));
        assert!(matches!(
            &shadow[1],
            CssOutputNode::Style { children, .. }
                if children == &[CssOutputNode::PropertyInitials(PropertyInheritance::NonInherited)]
        ));
        let CssOutputNode::Group {
            origin: Some(origin),
            ..
        } = &shadow[0]
        else {
            panic!("{shadow:?}");
        };
        assert_eq!(
            css[origin.start.0 as usize..origin.end.0 as usize].trim(),
            "@layer a { @stillwind-slot utilities; }"
        );
        // The entry template ignores template definitions.
        assert_eq!(
            parse_output_template(css, &[]).unwrap().authored_css(),
            ".page{color:red}@layer x{}"
        );
    }

    #[test]
    fn css_templates_reject_definitions_missing_utilities_and_malformed_rules() {
        for (css, expected) in [
            (
                "@stillwind-css-template a { @theme { --x: 1; } @stillwind-slot utilities; }",
                "DefinitionInCssTemplate",
            ),
            (
                "@stillwind-css-template a { @layer b { @utility u { color: red; } } @stillwind-slot utilities; }",
                "DefinitionInCssTemplate",
            ),
            (
                "@stillwind-css-template a { :host { @stillwind-slot theme; } }",
                "CssTemplateWithoutUtilities",
            ),
            (
                "@stillwind-css-template { @stillwind-slot utilities; }",
                "InvalidCssTemplate",
            ),
            (
                "@stillwind-css-template a b { @stillwind-slot utilities; }",
                "InvalidCssTemplate",
            ),
            ("@stillwind-css-template a;", "InvalidCssTemplate"),
        ] {
            let error = parse_css_templates(css).expect_err(css);
            assert!(
                format!("{error:?}").starts_with(expected),
                "{css}: {error:?}"
            );
        }
        let error =
            parse_css_templates("@stillwind-css-template a { @theme { --x: 1; } }").unwrap_err();
        assert_eq!(
            error,
            CompileDiagnostic::DefinitionInCssTemplate {
                offset: 28,
                directive: "theme".into()
            }
        );
    }
}
