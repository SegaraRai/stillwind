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

use cssparser::{ParseError, Parser, Token};

use stillwind_css::{CssOutputNode, CssOutputTemplate};
use stillwind_diagnostics::CompileErrorKind;
use stillwind_source::{FileId, Span};

use crate::ThemeToken;
use crate::parser::extract_keyframe_blocks;

/// Deepest group nesting accepted in one authored buffer.
///
/// Parsing recurses once per nested group, so this bounds the stack one stylesheet can
/// consume; deeper input is reported as [`CompileErrorKind::CssNestingTooDeep`] instead
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
) -> Result<CssOutputTemplate, CompileErrorKind> {
    let nodes = parse_nodes(css, 0, tokens, &mut Vec::new(), false, 0)?;
    validate_slots(&nodes, false)?;
    Ok(CssOutputTemplate {
        source_identity: String::new(),
        nodes,
    })
}

/// Collects the inner byte range and the modifier header of every `@theme` block, in
/// source order.
///
/// Callers use this to extract declarations per block, so a block's mode (`inline`,
/// `static`, or default) can be attributed to its own tokens.
///
/// # Errors
///
/// Returns a block error for unparsable or unterminated `@theme` syntax.
pub(crate) fn theme_blocks(
    css: &str,
) -> Result<Vec<(std::ops::Range<usize>, String)>, CompileErrorKind> {
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
            "theme" | "stillwind" | "utility" | "custom-variant" | "variant" | "tailwind"
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
) -> Result<Option<(usize, usize)>, CompileErrorKind> {
    loop {
        match parser.next_including_whitespace_and_comments() {
            Ok(Token::CurlyBracketBlock) => {
                let position = parser.position().byte_index() - 1;
                parser
                    .parse_nested_block(|nested| {
                        while nested.next_including_whitespace_and_comments().is_ok() {}
                        Ok::<_, ParseError<()>>(())
                    })
                    .map_err(|_| CompileErrorKind::InvalidCssOutputBlock {
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
    theme_blocks: &mut Vec<(std::ops::Range<usize>, String)>,
    conditional: bool,
    depth: usize,
) -> Result<Vec<CssOutputNode>, CompileErrorKind> {
    if depth > MAX_BLOCK_NESTING {
        return Err(CompileErrorKind::CssNestingTooDeep {
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
            Token::AtKeyword(name) => Some(name.to_string()),
            _ => None,
        };
        let header_end = parser.position().byte_index();
        if conditional
            && let Some(name) = name.as_deref()
            && is_definition(name)
            && name != "stillwind-slot"
        {
            return Err(CompileErrorKind::ConditionalDefinition {
                offset: base + start,
                directive: name.into(),
            });
        }
        let block = consume_block(&mut parser, base)?;
        let end = parser.position().byte_index();
        let origin = Some(Span::new(
            FileId(0),
            u32::try_from(base + start).map_err(|_| CompileErrorKind::InvalidCssOutputBlock {
                offset: base + start,
            })?,
            u32::try_from(base + end)
                .map_err(|_| CompileErrorKind::InvalidCssOutputBlock { offset: base + end })?,
        ));
        match (name.as_deref(), block) {
            (Some("layer"), None) => nodes.push(CssOutputNode::LayerOrder {
                css: css[start..end].into(),
                origin,
            }),
            (Some("stillwind-slot"), Some(_)) | (Some("theme"), None) => {
                return Err(CompileErrorKind::InvalidCssOutputBlock {
                    offset: base + start,
                });
            }
            (Some("stillwind-slot"), None) => {
                let value = css[header_end..end].trim().trim_end_matches(';').trim();
                let node = match value {
                    "utilities" => CssOutputNode::Utilities,
                    "theme" => CssOutputNode::ThemeVariables,
                    "keyframes" => CssOutputNode::Keyframes,
                    "properties" => CssOutputNode::Properties,
                    "property-initials" => CssOutputNode::PropertyInitials,
                    _ => {
                        return Err(CompileErrorKind::UnsupportedOutputSlot {
                            offset: base + start,
                            slot: value.into(),
                        });
                    }
                };
                nodes.push(node);
            }
            (Some("theme"), Some((open, close))) => {
                theme_blocks.push((
                    base + open + 1..base + close,
                    css[header_end..open].trim().into(),
                ));
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
                    keyframes: extract_keyframe_blocks(&css[open + 1..close]),
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
                            | CssOutputNode::PropertyInitials
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
                                | CssOutputNode::PropertyInitials
                        )
                    }) {
                        return Err(CompileErrorKind::InvalidCssOutputBlock {
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
/// Returns [`CompileErrorKind::InvalidOutputSlotContext`] at the first mismatched slot.
fn validate_slots(nodes: &[CssOutputNode], declarations: bool) -> Result<(), CompileErrorKind> {
    for node in nodes {
        let (name, needs_declarations) = match node {
            CssOutputNode::ThemeVariables => ("theme", true),
            CssOutputNode::PropertyInitials => ("property-initials", true),
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
            return Err(CompileErrorKind::InvalidOutputSlotContext { slot: name.into() });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
                css: "@layer a, b;".to_owned(),
                origin: Some(Span::new(
                    FileId(0),
                    0,
                    u32::try_from("@layer a, b;".len()).unwrap(),
                )),
            }
        );
        // Declaration slots require a style rule and keep it as a Style node.
        for slot in ["theme", "property-initials"] {
            let css = format!("@media print{{.scope{{before:1;@stillwind-slot {slot};after:2;}}}}");
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
            let css = format!("@stillwind-slot {slot};");
            let nodes = parse_output_template(&css, &[])
                .unwrap_or_else(|error| panic!("{slot}: {error:?}"));
            assert_eq!(nodes.nodes, [expected], "{slot}");
        }
        // A declaration slot outside a style rule is a context error.
        assert!(matches!(
            parse_output_template("@stillwind-slot theme;", &[]),
            Err(CompileErrorKind::InvalidOutputSlotContext { slot }) if slot == "theme"
        ));
    }

    /// Unknown slots, block-form slots, statement-form themes, and definitions under a
    /// condition are rejected with the offset of the offending directive.
    #[test]
    fn parse_nodes_rejects_invalid_directive_forms() {
        let css = "@stillwind-slot unknown;";
        assert!(matches!(
            parse_output_template(css, &[]),
            Err(CompileErrorKind::UnsupportedOutputSlot { offset, slot })
                if offset == 0 && slot == "unknown"
        ));
        for (css, expected) in [
            ("@stillwind-slot theme {}", "InvalidCssOutputBlock"),
            ("@theme ;", "InvalidCssOutputBlock"),
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
            assert!(format!("{error:?}").contains(expected), "{css}: {error:?}");
        }
        assert!(matches!(
            parse_output_template("@media print { @theme { --a: 1; } }", &[]),
            Err(CompileErrorKind::ConditionalDefinition { directive, .. }) if directive == "theme"
        ));
    }

    /// Group nesting is bounded, and the limit is reported as a nesting error.
    #[test]
    fn parse_nodes_bounds_group_nesting() {
        let shallow = format!(
            "{}.x{{color:red}}{}",
            "@media a{".repeat(MAX_BLOCK_NESTING),
            "}".repeat(2)
        );
        assert!(parse_output_template(&shallow, &[]).is_ok());
        let deep = format!(
            "{}.x{{color:red}}{}",
            "@media a{".repeat(MAX_BLOCK_NESTING + 1),
            "}".repeat(MAX_BLOCK_NESTING + 1)
        );
        assert!(matches!(
            parse_output_template(&deep, &[]),
            Err(CompileErrorKind::CssNestingTooDeep { limit, .. }) if limit == MAX_BLOCK_NESTING
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
            Err(CompileErrorKind::UnsupportedOutputSlot { .. })
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
                Err(CompileErrorKind::ConditionalDefinition { .. })
            ));
        }
    }
}
