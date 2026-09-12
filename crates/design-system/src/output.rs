//! Parsing of the host's Stillwind CSS into a structured output template.
//!
//! An output template records where generated output may be placed: the `@theme`
//! blocks, the `@stillwind-slot utilities` slot, and the authored CSS that must survive
//! verbatim. Every node keeps its byte offsets, and theme tokens keep spans into the
//! buffer under parse, so a caller can map those spans back to the originally supplied
//! source.
//!
//! Definitions are compile-time only: an `@theme`, `@utility`, or `@custom-variant`
//! nested inside a conditional is rejected, because emitting it would make the registry
//! depend on runtime conditions. `@stillwind-slot` is the exception and may be nested.

use cssparser::{ParseError, Parser, Token};

use stillwind_css::{CssOutputError, CssOutputNode, CssOutputTemplate};
use stillwind_source::{FileId, Span};

use crate::ThemeToken;

/// Parses one CSS buffer into its output nodes and theme-block ranges.
///
/// `tokens` are the already extracted theme tokens; each `@theme` node records the
/// indices of the tokens whose name spans fall inside that block, so theme output can
/// be emitted per block in authored order.
///
/// # Errors
///
/// Returns malformed blocks, definitions inside a conditional group, or unknown output
/// slots.
pub fn parse_output_template(
    css: &str,
    tokens: &[ThemeToken],
) -> Result<CssOutputTemplate, CssOutputError> {
    Ok(CssOutputTemplate {
        source_identity: String::new(),
        nodes: parse_nodes(css, 0, tokens, &mut Vec::new(), false)?,
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
) -> Result<Vec<(std::ops::Range<usize>, String)>, CssOutputError> {
    let mut blocks = Vec::new();
    parse_nodes(css, 0, &[], &mut blocks, false)?;
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
) -> Result<Option<(usize, usize)>, CssOutputError> {
    loop {
        match parser.next_including_whitespace_and_comments() {
            Ok(Token::CurlyBracketBlock) => {
                let position = parser.position().byte_index() - 1;
                parser
                    .parse_nested_block(|nested| {
                        while nested.next_including_whitespace_and_comments().is_ok() {}
                        Ok::<_, ParseError<()>>(())
                    })
                    .map_err(|_| CssOutputError::Block {
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
/// is rejected.
///
/// # Errors
///
/// Returns an error for conditional definitions, malformed blocks, unknown slots, and
/// offsets that exceed the 32-bit source limit.
fn parse_nodes(
    css: &str,
    base: usize,
    tokens: &[ThemeToken],
    theme_blocks: &mut Vec<(std::ops::Range<usize>, String)>,
    conditional: bool,
) -> Result<Vec<CssOutputNode>, CssOutputError> {
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
            return Err(CssOutputError::ConditionalDefinition {
                offset: base + start,
                name: name.into(),
            });
        }
        let block = consume_block(&mut parser, base)?;
        let end = parser.position().byte_index();
        let origin = Some(Span::new(
            FileId(0),
            u32::try_from(base + start).map_err(|_| CssOutputError::Block {
                offset: base + start,
            })?,
            u32::try_from(base + end).map_err(|_| CssOutputError::Block { offset: base + end })?,
        ));
        match (name.as_deref(), block) {
            (Some("layer"), None) => nodes.push(CssOutputNode::LayerOrder {
                css: css[start..end].into(),
                origin,
            }),
            (Some("stillwind-slot"), Some(_)) | (Some("theme"), None) => {
                return Err(CssOutputError::Block {
                    offset: base + start,
                });
            }
            (Some("stillwind-slot"), None) => {
                let value = css[header_end..end].trim().trim_end_matches(';').trim();
                if value != "utilities" {
                    return Err(CssOutputError::Slot {
                        offset: base + start,
                        name: value.into(),
                    });
                }
                nodes.push(CssOutputNode::Utilities);
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
                    keyframes: crate::theme::parser::extract_keyframe_blocks(&css[open + 1..close]),
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
                    )?,
                });
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

#[cfg(test)]
mod tests {
    use super::*;

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
            Err(CssOutputError::Slot { .. })
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
                Err(CssOutputError::ConditionalDefinition { .. })
            ));
        }
    }
}
