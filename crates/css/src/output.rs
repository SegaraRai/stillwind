//! Source-ordered CSS output, independent of the host's module/chunk policy.

use cssparser::{ParseError, Parser, Token};
use stillwind_source::{FileId, Span};

use crate::ThemeToken;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CssOutputNode {
    Authored {
        css: String,
        origin: Option<Span>,
    },
    LayerOrder {
        css: String,
        origin: Option<Span>,
    },
    Group {
        header: String,
        children: Vec<Self>,
        origin: Option<Span>,
    },
    Theme {
        tokens: Vec<usize>,
        keyframes: String,
        origin: Option<Span>,
    },
    Utilities,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CssOutputTemplate {
    /// Host identity of the entry, shared by every output carrier.
    pub source_identity: String,
    pub nodes: Vec<CssOutputNode>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CssOutputError {
    #[error(
        "compile-time @{name} definitions cannot be conditional (byte {offset}); import definitions unconditionally and put conditions around @stillwind-slot utilities;"
    )]
    ConditionalDefinition { offset: usize, name: String },
    #[error("unsupported Stillwind output slot at byte {offset}: {name}")]
    Slot { offset: usize, name: String },
    #[error("invalid CSS output block at byte {offset}")]
    Block { offset: usize },
}

impl CssOutputTemplate {
    /// # Errors
    /// Returns malformed blocks or unknown output slots.
    pub fn parse(css: &str, tokens: &[ThemeToken]) -> Result<Self, CssOutputError> {
        Ok(Self {
            source_identity: String::new(),
            nodes: parse_nodes(css, 0, tokens, &mut Vec::new(), false)?,
        })
    }

    pub fn authored_css(&self) -> String {
        authored_css(&self.nodes)
    }

    pub fn map_origins(&mut self, mut map: impl FnMut(Span) -> Option<Span>) {
        fn visit_nodes(nodes: &mut [CssOutputNode], map: &mut impl FnMut(Span) -> Option<Span>) {
            for node in nodes {
                match node {
                    CssOutputNode::Authored { origin, .. }
                    | CssOutputNode::LayerOrder { origin, .. }
                    | CssOutputNode::Theme { origin, .. } => *origin = origin.and_then(&mut *map),
                    CssOutputNode::Group {
                        origin, children, ..
                    } => {
                        *origin = origin.and_then(&mut *map);
                        visit_nodes(children, map);
                    }
                    CssOutputNode::Utilities => {}
                }
            }
        }
        visit_nodes(&mut self.nodes, &mut map);
    }
}

fn authored_css(nodes: &[CssOutputNode]) -> String {
    nodes
        .iter()
        .map(|node| match node {
            CssOutputNode::Authored { css, .. } | CssOutputNode::LayerOrder { css, .. } => {
                css.clone()
            }
            CssOutputNode::Group {
                header, children, ..
            } => {
                format!("{header}{{{}}}", authored_css(children))
            }
            CssOutputNode::Theme { .. } | CssOutputNode::Utilities => String::new(),
        })
        .collect()
}

pub(crate) fn theme_blocks(
    css: &str,
) -> Result<Vec<(std::ops::Range<usize>, String)>, CssOutputError> {
    let mut blocks = Vec::new();
    parse_nodes(css, 0, &[], &mut blocks, false)?;
    Ok(blocks)
}

fn is_definition(name: &str) -> bool {
    name.starts_with("stillwind-")
        || matches!(
            name,
            "theme"
                | "stillwind"
                | "utility"
                | "utility-family"
                | "custom-variant"
                | "variant"
                | "tailwind"
        )
}

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
        let template = CssOutputTemplate::parse("@stillwind-marker group; .before{content:'@theme { ; }';} @layer x { @supports (display:grid) { @stillwind-slot utilities; } .after{color:red} }", &[]).unwrap();
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
            CssOutputTemplate::parse("/* @theme { } */ .x{content:';{}';}", &[])
                .unwrap()
                .nodes
                .len(),
            1
        );
        assert!(matches!(
            CssOutputTemplate::parse("@stillwind-slot other;", &[]),
            Err(CssOutputError::Slot { .. })
        ));
    }

    #[test]
    fn conditional_slots_are_supported_but_conditional_semantics_are_diagnosed() {
        assert!(
            CssOutputTemplate::parse(
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
                CssOutputTemplate::parse(css, &[]),
                Err(CssOutputError::ConditionalDefinition { .. })
            ));
        }
    }
}
