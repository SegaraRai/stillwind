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
    CssOutputNode, CssOutputTemplate, PropertyInheritance, consume_css_block,
    extract_directive_blocks, extract_named_statements,
};
use stillwind_diagnostics::CompileDiagnostic;
use stillwind_source::{CssSourceName, FileId, Span};

use crate::skipped::SkippedRange;
use crate::theme::extract_keyframes_blocks;
use crate::{SkipReason, ThemeValue, ThemeValuePrecedence};

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
/// Invalid directives and slots are left out and appended to `skipped`, with ranges in
/// `css`. A conditional definition is kept without its condition and also appended.
///
/// # Errors
///
/// Returns an error for malformed blocks, offsets outside the 32-bit source range, or
/// groups nested past [`MAX_BLOCK_NESTING`].
pub(crate) fn parse_output_template(
    css: &str,
    tokens: &[ThemeValue],
    skipped: &mut Vec<SkippedRange>,
) -> Result<CssOutputTemplate, CompileDiagnostic> {
    let nodes = parse_nodes(css, 0, tokens, &mut Vec::new(), Context::TOP, skipped)?;
    Ok(CssOutputTemplate {
        source_identity: CssSourceName::default(),
        nodes,
    })
}

/// Parses every `@stillwind-css-template <name> { ... }` rule in one CSS buffer.
///
/// Each body uses the slot syntax of the output template and describes where the output
/// of one `style.css()` call goes. A later rule with the same name replaces an earlier
/// one, so a project can redefine a template of the package. Origins use `FileId(0)`
/// and UTF-8 byte offsets into `css`, as [`parse_output_template`] describes.
///
/// A template is left out and appended to `skipped` when its rule has no single name
/// and block, when its body contains a definition such as `@theme`, or when its body
/// has no utilities slot. Invalid slots in a body are left out as in
/// [`parse_output_template`].
///
/// # Errors
///
/// Body syntax fails as [`parse_output_template`] does.
pub(crate) fn parse_css_templates(
    css: &str,
    skipped: &mut Vec<SkippedRange>,
) -> Result<BTreeMap<LeanStr, CssOutputTemplate>, CompileDiagnostic> {
    for (offset, header) in extract_named_statements(css, CSS_TEMPLATE) {
        let end = offset + CSS_TEMPLATE.len() + header.len();
        skipped.push((offset..end.min(css.len()), SkipReason::CssTemplate));
    }
    let mut templates = BTreeMap::new();
    for definition in extract_directive_blocks(css, CSS_TEMPLATE) {
        let open = definition.body.start - 1;
        let directive = definition.range.start..open;
        let header = definition.header;
        let mut parser = Parser::new(&header);
        let name = match parser.next() {
            Ok(Token::Ident(name)) => Some(name.to_lean_str()),
            _ => None,
        };
        let Some(name) = name.filter(|_| parser.is_exhausted()) else {
            skipped.push((directive, SkipReason::CssTemplate));
            continue;
        };
        let body = &css[definition.body];
        if let Some((position, directive)) = first_definition(body) {
            let start = open + 1 + position;
            skipped.push((
                start..start + 1 + directive.len(),
                SkipReason::DefinitionInCssTemplate { directive },
            ));
            continue;
        }
        let nodes = parse_nodes(body, open + 1, &[], &mut Vec::new(), Context::TOP, skipped)?;
        if !contains_utilities(&nodes) {
            skipped.push((directive, SkipReason::CssTemplateWithoutUtilities { name }));
            continue;
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
pub(crate) type ThemeBlock = (Range<usize>, LeanString, ThemeValuePrecedence);

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
    // `parse_output_template` reads the same buffer and records its skipped statements.
    parse_nodes(css, 0, &[], &mut blocks, Context::TOP, &mut Vec::new())?;
    Ok(blocks)
}

/// Where [`parse_nodes`] reads one node list.
#[derive(Clone, Copy)]
struct Context {
    /// Whether an enclosing group is conditional, so a definition loses its condition.
    conditional: bool,
    /// Whether the node list is the body of a style rule, which accepts declarations.
    declarations: bool,
    /// Groups already open around this node list.
    depth: usize,
}

impl Context {
    /// The top level of a buffer.
    const TOP: Self = Self {
        conditional: false,
        declarations: false,
        depth: 0,
    };

    /// The children of a nested block.
    const fn nested(self, conditional: bool, declarations: bool) -> Self {
        Self {
            conditional: self.conditional || conditional,
            declarations,
            depth: self.depth + 1,
        }
    }
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
/// Returns the opening offset and content end for a `{}` block. An unclosed block ends
/// at EOF. Returns `None` when the construct ends at a semicolon or cannot be parsed,
/// which the caller treats as a statement.
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
                let block =
                    consume_css_block(parser).map_err(|_| CompileDiagnostic::InvalidCssBlock {
                        offset: base + position,
                    })?;
                return Ok(Some((position, block.contents.end)));
            }
            Ok(Token::Semicolon) | Err(_) => {
                return Ok(None);
            }
            Ok(_) => {}
        }
    }
}

/// Parses one nesting level into output nodes.
///
/// `base` is the byte offset of `css` inside the root buffer, so every origin span and
/// skipped range refers to the root buffer rather than to the slice. `context` records
/// whether an enclosing group is conditional, whether the level accepts declarations,
/// and how many groups are open around it.
///
/// A definition below a conditional group is kept without its condition and appended to
/// `skipped`. A directive in the wrong form, an unknown slot, a slot in a context that
/// cannot contain its output, and a style rule that mixes slots with other rules are
/// left out or kept as authored CSS, and also appended.
///
/// # Errors
///
/// Returns an error for malformed blocks and offsets that exceed the 32-bit source
/// limit, or when the buffer nests groups deeper than [`MAX_BLOCK_NESTING`].
#[allow(clippy::too_many_lines)] // One dispatch keeps source ranges aligned for authored rules, definitions, and slots.
fn parse_nodes(
    css: &str,
    base: usize,
    tokens: &[ThemeValue],
    theme_blocks: &mut Vec<ThemeBlock>,
    context: Context,
    skipped: &mut Vec<SkippedRange>,
) -> Result<Vec<CssOutputNode>, CompileDiagnostic> {
    if context.depth > MAX_BLOCK_NESTING {
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
        // `start` includes the whitespace and comments before the token.
        let token_start = css[start..header_end]
            .rfind('@')
            .map_or(start, |position| start + position);
        let keyword = base + token_start..base + header_end;
        if context.conditional
            && let Some(name) = name.as_deref()
            && is_definition(name)
            && name != "stillwind-slot"
        {
            skipped.push((
                keyword.clone(),
                SkipReason::ConditionalDefinition {
                    directive: name.into(),
                },
            ));
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
            (Some("layer"), None) => {
                nodes.push(CssOutputNode::LayerOrder {
                    css: css[start..end].into(),
                    origin,
                });
            }
            (Some(directive @ ("stillwind-slot" | "theme")), block)
                if (directive == "theme") != block.is_some() =>
            {
                skipped.push((
                    keyword,
                    SkipReason::DirectiveForm {
                        directive: directive.into(),
                    },
                ));
            }
            (Some("stillwind-slot"), None) => {
                let value = css[header_end..end].trim().trim_end_matches(';').trim();
                // Arguments may be separated by any CSS whitespace.
                let words = value.split_whitespace().collect::<Vec<_>>().join(" ");
                let (node, slot, needs_declarations) = match words.as_str() {
                    "utilities" => (CssOutputNode::Utilities, "utilities", false),
                    "theme" => (CssOutputNode::ThemeVariables, "theme", true),
                    "keyframes" => (CssOutputNode::Keyframes, "keyframes", false),
                    "properties" => (CssOutputNode::Properties, "properties", false),
                    "property-initials" => (
                        CssOutputNode::PropertyInitials(PropertyInheritance::All),
                        "property-initials",
                        true,
                    ),
                    "property-initials inherited" => (
                        CssOutputNode::PropertyInitials(PropertyInheritance::Inherited),
                        "property-initials",
                        true,
                    ),
                    "property-initials non-inherited" => (
                        CssOutputNode::PropertyInitials(PropertyInheritance::NonInherited),
                        "property-initials",
                        true,
                    ),
                    _ => {
                        skipped.push((
                            base + token_start..base + end,
                            SkipReason::UnsupportedSlot { slot: value.into() },
                        ));
                        continue;
                    }
                };
                if context.declarations != needs_declarations {
                    skipped.push((
                        base + token_start..base + end,
                        SkipReason::SlotContext { slot: slot.into() },
                    ));
                    continue;
                }
                nodes.push(node);
            }
            (Some(directive @ ("theme" | "stillwind-plugin-theme")), Some((open, close))) => {
                let modifier = css[header_end..open].trim();
                let precedence = if directive == "stillwind-plugin-theme" {
                    ThemeValuePrecedence::Plugin
                } else {
                    ThemeValuePrecedence::parse(modifier)
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
                    keyframes: (extract_keyframes_blocks(&css[open + 1..close])).into(),
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
                        context.nested(name.as_deref() != Some("layer"), context.declarations),
                        skipped,
                    )?,
                });
            }
            (None, Some((open, close))) if css[open + 1..close].contains("@stillwind-slot") => {
                let mut rule_skipped = Vec::new();
                let children = parse_nodes(
                    &css[open + 1..close],
                    base + open + 1,
                    tokens,
                    theme_blocks,
                    context.nested(false, true),
                    &mut rule_skipped,
                )?;
                let slots = children.iter().any(|node| {
                    matches!(
                        node,
                        CssOutputNode::ThemeVariables
                            | CssOutputNode::PropertyInitials(_)
                            | CssOutputNode::Utilities
                            | CssOutputNode::Keyframes
                            | CssOutputNode::Properties
                            | CssOutputNode::Style { .. }
                    )
                });
                let mixed = children.iter().any(|node| {
                    !matches!(
                        node,
                        CssOutputNode::Authored { .. }
                            | CssOutputNode::ThemeVariables
                            | CssOutputNode::PropertyInitials(_)
                    )
                });
                if slots && mixed {
                    // The rule stays authored CSS, so the statements that its children
                    // skipped are not reported separately.
                    skipped.push((base + token_start..base + open, SkipReason::SlotStyleRule));
                    nodes.push(CssOutputNode::Authored {
                        css: css[start..end].into(),
                        origin,
                    });
                } else if slots {
                    skipped.append(&mut rule_skipped);
                    nodes.push(CssOutputNode::Style {
                        header: css[start..open].trim().into(),
                        children,
                        origin,
                    });
                } else {
                    skipped.append(&mut rule_skipped);
                    nodes.push(CssOutputNode::Authored {
                        css: css[start..end].into(),
                        origin,
                    });
                }
            }
            (Some(name), _) if is_definition(name) => {}
            _ => {
                nodes.push(CssOutputNode::Authored {
                    css: css[start..end].into(),
                    origin,
                });
            }
        }
    }
    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use lean_string::ToLeanStr;

    use super::*;

    /// Parses `css` as an output template and returns it with its skipped statements.
    fn template(css: &str) -> (CssOutputTemplate, Vec<SkippedRange>) {
        let mut skipped = Vec::new();
        let template = parse_output_template(css, &[], &mut skipped)
            .unwrap_or_else(|error| panic!("{css}: {error:?}"));
        (template, skipped)
    }

    /// Parses the templates of `css` and returns them with the skipped statements.
    fn templates(css: &str) -> (BTreeMap<LeanStr, CssOutputTemplate>, Vec<SkippedRange>) {
        let mut skipped = Vec::new();
        let templates = parse_css_templates(css, &mut skipped)
            .unwrap_or_else(|error| panic!("{css}: {error:?}"));
        (templates, skipped)
    }

    #[test]
    fn declaration_slots_require_a_style_rule_and_ignore_quoted_markers() {
        for (css, slot) in [
            ("@stillwind-slot theme;", "theme"),
            (
                "@layer a{@stillwind-slot property-initials;}",
                "property-initials",
            ),
            (".x{@stillwind-slot utilities;}", "utilities"),
        ] {
            let (_, skipped) = template(css);
            assert!(
                matches!(skipped.as_slice(), [(_, SkipReason::SlotContext { slot: skipped })] if skipped == slot),
                "{css}: {skipped:?}"
            );
        }
        let css = ".x{content:'@stillwind-slot theme;'}";
        assert_eq!(template(css).0.authored_css(), css);
        let (nodes, _) = template("@media print{.scope{before:1;@stillwind-slot theme;after:2;}}");
        assert!(
            matches!(&nodes.nodes[0], CssOutputNode::Group { children, .. } if matches!(&children[0], CssOutputNode::Style { .. }))
        );
    }

    /// Each slot keyword maps to its node, statements keep their own form, and the
    /// origin span covers the whole directive.
    #[test]
    fn parse_nodes_maps_every_slot_and_statement_form() {
        let (nodes, _) = template("@layer a, b; .x{color:red}");
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
            let (nodes, skipped) = template(&css);
            assert_eq!(skipped, [], "{slot}");
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
            let (nodes, skipped) = template(&css);
            assert_eq!(skipped, [], "{slot}");
            assert_eq!(nodes.nodes, [expected], "{slot}");
        }
    }

    /// Unknown slots and directives in the wrong form are left out, and definitions
    /// under a condition lose the condition. Each is recorded with its range.
    #[test]
    fn parse_nodes_skips_invalid_directive_forms() {
        let css = "@stillwind-slot unknown; .a{}";
        let (nodes, skipped) = template(css);
        assert_eq!(nodes.authored_css().trim(), ".a{}");
        assert_eq!(
            skipped,
            [(
                0..css.find(';').unwrap() + 1,
                SkipReason::UnsupportedSlot {
                    slot: "unknown".into()
                }
            )]
        );
        for (css, expected) in [
            (
                "@stillwind-slot theme {}",
                SkipReason::DirectiveForm {
                    directive: "stillwind-slot".into(),
                },
            ),
            (
                "@theme ;",
                SkipReason::DirectiveForm {
                    directive: "theme".into(),
                },
            ),
            (
                "@media print { @theme { --a: 1; } }",
                SkipReason::ConditionalDefinition {
                    directive: "theme".into(),
                },
            ),
            (
                "@supports (display:grid) { @utility x { color: red; } }",
                SkipReason::ConditionalDefinition {
                    directive: "utility".into(),
                },
            ),
        ] {
            let (_, skipped) = template(css);
            assert!(
                matches!(skipped.as_slice(), [(range, reason)] if *reason == expected && css[range.clone()].starts_with('@')),
                "{css}: {skipped:?}"
            );
        }
        let (_, skipped) = template(".a { .b { @stillwind-slot theme; } }");
        assert!(
            matches!(skipped.as_slice(), [(_, SkipReason::SlotStyleRule)]),
            "{skipped:?}"
        );
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
        assert!(parse_output_template(&shallow, &[], &mut Vec::new()).is_ok());
        let deep = format_args!(
            "{}.x{{color:red}}{}",
            "@media a{".repeat(MAX_BLOCK_NESTING + 1),
            "}".repeat(MAX_BLOCK_NESTING + 1)
        )
        .to_lean_string();
        assert!(matches!(
            parse_output_template(&deep, &[], &mut Vec::new()),
            Err(CompileDiagnostic::CssNestingTooDeep { limit, .. }) if limit == MAX_BLOCK_NESTING
        ));
    }

    #[test]
    fn preserves_conditions_and_rules_after_metadata_statements() {
        let (template, _) = template(
            "@stillwind-marker group; .before{content:'@theme { ; }';} @layer x { @supports (display:grid) { @stillwind-slot utilities; } .after{color:red} }",
        );
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
    fn comments_and_strings_are_not_directives() {
        let (nodes, skipped) = template("/* @theme { } */ .x{content:';{}';}");
        assert_eq!(nodes.nodes.len(), 1);
        assert_eq!(skipped, []);
    }

    #[test]
    fn conditional_slots_are_supported_but_conditional_definitions_are_recorded() {
        assert_eq!(
            template("@supports (display:grid){@media print{@stillwind-slot utilities;}}").1,
            []
        );
        for css in [
            "@supports (display:grid){@theme {--color-brand:blue;}}",
            "@media print{@layer test{@stillwind-utility flex{display:flex;}}}",
        ] {
            let (_, skipped) = template(css);
            assert!(
                matches!(
                    skipped.as_slice(),
                    [(_, SkipReason::ConditionalDefinition { .. })]
                ),
                "{css}: {skipped:?}"
            );
        }
    }

    #[test]
    fn css_templates_are_named_and_later_definitions_win() {
        let css = ".page{color:red} \
            @stillwind-css-template shadow { :host { @stillwind-slot theme; } @stillwind-slot utilities; } \
            @layer x { @stillwind-css-template plain { @stillwind-slot utilities; } } \
            @stillwind-css-template shadow { @layer a { @stillwind-slot utilities; } \
              :where(*) { @stillwind-slot property-initials   non-inherited; } }";
        let (templates, skipped) = templates(css);
        assert_eq!(skipped, []);
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
        assert_eq!(template(css).0.authored_css(), ".page{color:red}@layer x{}");
    }

    #[test]
    fn css_templates_skip_definitions_missing_utilities_and_malformed_rules() {
        for (css, expected) in [
            (
                "@stillwind-css-template a { @theme { --x: 1; } @stillwind-slot utilities; }",
                SkipReason::DefinitionInCssTemplate {
                    directive: "theme".into(),
                },
            ),
            (
                "@stillwind-css-template a { @layer b { @utility u { color: red; } } @stillwind-slot utilities; }",
                SkipReason::DefinitionInCssTemplate {
                    directive: "utility".into(),
                },
            ),
            (
                "@stillwind-css-template a { :host { @stillwind-slot theme; } }",
                SkipReason::CssTemplateWithoutUtilities { name: "a".into() },
            ),
            (
                "@stillwind-css-template { @stillwind-slot utilities; }",
                SkipReason::CssTemplate,
            ),
            (
                "@stillwind-css-template a b { @stillwind-slot utilities; }",
                SkipReason::CssTemplate,
            ),
            ("@stillwind-css-template a;", SkipReason::CssTemplate),
        ] {
            let (templates, skipped) = templates(css);
            assert!(templates.is_empty(), "{css}");
            assert!(
                matches!(skipped.as_slice(), [(_, reason)] if *reason == expected),
                "{css}: {skipped:?}"
            );
        }
        let css = "@stillwind-css-template a { @theme { --x: 1; } }";
        assert_eq!(
            templates(css).1,
            [(
                28..34,
                SkipReason::DefinitionInCssTemplate {
                    directive: "theme".into()
                }
            )]
        );
    }
}
