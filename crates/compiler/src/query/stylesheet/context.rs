//! Cursor contexts from the structure and lexical tokens of incomplete CSS.
//!
//! [`context`] selects the original input range used by both completion and hover.
//! It accepts unclosed blocks and partial tokens without validating the stylesheet.
//! Registry lookup and candidate expansion belong to the consuming query.

use std::ops::Range;

use cssparser::{ParseError, Parser, Token};

use stillwind_css::{CssNode, CssNodeKind, find_css_functions, parse_structure};

use super::directives::Positions;
use crate::compiler::variant_sequences;

/// The syntax at a cursor, with the complete text range that a completion replaces.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct Context {
    /// UTF-8 byte range in the original stylesheet, including text after the cursor.
    /// The cursor can equal either boundary. An empty range inserts a new argument.
    pub(super) range: Range<usize>,
    /// Input role that selects completion labels and hover semantics.
    pub(super) kind: Kind,
    /// Enclosing rule and definition roles used only to rank and filter directive
    /// names.
    pub(super) positions: Positions,
    /// Variant sequences that already apply at a [`Kind::Variant`] input. Other kinds
    /// leave this empty.
    pub(super) applied: AppliedVariants,
}

/// Variant sequences that already apply to the element of an applied `@variant` input.
/// Every range is a trimmed, non-empty UTF-8 byte range in the original stylesheet.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct AppliedVariants {
    /// Comma-separated sequences of each enclosing applied `@variant` rule, outermost
    /// first. A nested style rule clears this list because its selector can select
    /// another element.
    pub(super) enclosing: Vec<Vec<Range<usize>>>,
    /// Other comma-separated sequences in the prelude that contains the cursor.
    pub(super) siblings: Vec<Range<usize>>,
}

/// A CSS input whose meaning can be resolved by an editor query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Kind {
    /// An at-rule name, including its leading `@`.
    Directive,
    /// A whitespace-separated argument of a directive with a finite semantic catalog.
    DirectiveArgument {
        /// Decoded name without `@`; definition names remain free authored text.
        directive: &'static str,
        /// True for the first argument, which selects the property of format metadata.
        first: bool,
    },
    /// One whitespace-separated utility candidate in `@apply`.
    Apply,
    /// One comma-separated sequence in an applied `@variant` inside a style rule.
    Variant,
    /// The entire prelude of `@stillwind-slot`, including any inheritance qualifier.
    Slot {
        /// True inside a style rule, including nested condition blocks. Definition
        /// blocks reset this flag because their bodies do not insert declarations.
        declarations: bool,
    },
    /// The first argument of `theme()`, `--theme()`, or `var()`, without surrounding
    /// whitespace. Fallback arguments are excluded.
    Theme {
        /// True for `var()`, whose value can differ from the active theme definition
        /// because the browser resolves it through the cascade.
        runtime: bool,
    },
}

/// Returns the input at the UTF-8 byte `offset` in the original `css`.
///
/// Invalid offsets and positions inside comments, strings, or URLs return `None`.
/// At other positions, the returned range covers a whole name, candidate, variant
/// sequence, or slot argument. Theme arguments take precedence over enclosing
/// candidates. Unsupported syntax and variant definition headers return `None`.
pub(super) fn context(css: &str, offset: usize) -> Option<Context> {
    if !css.is_char_boundary(offset) || protected(&mut Parser::new(css), css, offset) {
        return None;
    }
    for call in find_css_functions(css, &["theme", "--theme", "var"], 0)
        .into_iter()
        .rev()
    {
        let arguments = &css[call.arguments.clone()];
        let end = arguments.find(',').unwrap_or(arguments.len());
        let argument = &arguments[..end];
        let start = call.arguments.start + argument.len() - argument.trim_start().len();
        let end = (call.arguments.start + argument.trim_end().len()).max(start);
        if start <= offset && offset <= end {
            return Some(Context {
                range: start..end,
                kind: Kind::Theme {
                    runtime: call.name_index == 2,
                },
                positions: Positions::NONE,
                applied: AppliedVariants::default(),
            });
        }
    }
    let nodes = parse_structure(css);
    locate(
        css,
        &nodes,
        offset,
        Positions::RULES.union(Positions::UNCERTAIN),
        &[],
    )
}

/// Returns the input at `offset` among nodes parsed from the original `css`.
///
/// Node ranges and the UTF-8 byte `offset` must address `css`. `positions` records the
/// enclosing rule and definition roles. Grouping rules retain those roles, and new
/// definitions replace them. `enclosing` lists the sequences of enclosing applied
/// `@variant` rules, as [`AppliedVariants::enclosing`] describes. An unterminated node
/// can include trailing whitespace through the cursor so an empty argument remains
/// completable.
fn locate(
    css: &str,
    nodes: &[CssNode],
    offset: usize,
    positions: Positions,
    enclosing: &[Vec<Range<usize>>],
) -> Option<Context> {
    for node in nodes {
        let tail = offset > node.range.end
            && !css[node.range.clone()].ends_with([';', '}'])
            && css[node.range.end..offset].trim().is_empty();
        if offset < node.range.start || (offset > node.range.end && !tail) {
            continue;
        }
        match &node.kind {
            CssNodeKind::StyleRule { block, .. } => {
                if block.contents.start <= offset && offset <= block.contents.end {
                    return locate(
                        css,
                        &block.children,
                        offset,
                        positions
                            .without(
                                Positions::RULES
                                    .union(Positions::UNCERTAIN)
                                    .union(Positions::INTRINSIC_VARIANT),
                            )
                            .union(Positions::STYLE),
                        &[],
                    );
                }
            }
            CssNodeKind::Declaration { .. } => {}
            CssNodeKind::Apply { candidates } => {
                let end = name_end(css, node.range.start);
                if offset <= end {
                    return Some(Context {
                        range: node.range.start..end,
                        kind: Kind::Directive,
                        positions,
                        applied: AppliedVariants::default(),
                    });
                }
                if css.as_bytes().get(offset.wrapping_sub(1)) == Some(&b';') {
                    continue;
                }
                return Some(Context {
                    range: candidates
                        .iter()
                        .find(|range| range.start <= offset && offset <= range.end)
                        .cloned()
                        .unwrap_or(offset..offset),
                    kind: Kind::Apply,
                    positions,
                    applied: AppliedVariants::default(),
                });
            }
            CssNodeKind::AtRule { .. } => {
                if let Some(context) =
                    at_rule_context(css, node, offset, positions, enclosing, tail)
                {
                    return Some(context);
                }
            }
        }
    }
    None
}

/// Locates a name, argument, or child context in an at-rule from the same original CSS.
/// `positions` describes its parent, and `enclosing` lists the sequences of enclosing
/// applied `@variant` rules. `tail` permits trailing whitespace in an unterminated
/// header. Returns no context for other node kinds or unsupported headers.
fn at_rule_context(
    css: &str,
    node: &CssNode,
    offset: usize,
    positions: Positions,
    enclosing: &[Vec<Range<usize>>],
    tail: bool,
) -> Option<Context> {
    let CssNodeKind::AtRule {
        name,
        prelude,
        block,
    } = &node.kind
    else {
        return None;
    };
    let declarations = positions.intersects(Positions::STYLE);
    let applied = name == "variant" && declarations;
    let name_end = name_end(css, node.range.start);
    if offset <= name_end {
        return Some(Context {
            range: node.range.start..name_end,
            kind: Kind::Directive,
            positions,
            applied: AppliedVariants::default(),
        });
    }
    if let Some(block) = block
        && block.contents.start <= offset
        && offset <= block.contents.end
    {
        let mut nested = enclosing.to_vec();
        if applied {
            nested.push(sequences(css, prelude));
        }
        let positions = match name.as_str() {
            "utility" | "stillwind-utility" => Positions::UTILITY,
            "custom-variant" => Positions::VARIANT,
            "stillwind-variant" => Positions::VARIANT.union(Positions::INTRINSIC_VARIANT),
            "variant" if !declarations => Positions::VARIANT,
            "stillwind-animation-option" => Positions::ANIMATION,
            "stillwind-css-template" => Positions::TEMPLATE,
            "theme" | "stillwind-property" | "plugin" => Positions::NONE,
            _ => positions.without(Positions::UNCERTAIN.union(Positions::INTRINSIC_VARIANT)),
        };
        if !positions.intersects(Positions::STYLE) {
            nested.clear();
        }
        return locate(css, &block.children, offset, positions, &nested);
    }
    let header_end = block.as_ref().map_or_else(
        || {
            node.range.end.max(if tail { offset } else { 0 })
                - usize::from(css.as_bytes().get(node.range.end.wrapping_sub(1)) == Some(&b';'))
        },
        |block| block.contents.start - 1,
    );
    if offset > header_end {
        return None;
    }
    if name == "stillwind-slot" {
        return Some(Context {
            range: prelude.start.min(offset)..prelude.end.max(offset),
            kind: Kind::Slot { declarations },
            positions,
            applied: AppliedVariants::default(),
        });
    }
    if applied {
        return Some(variant_context(css, prelude, offset, positions, enclosing));
    }
    let directive = match name.as_str() {
        "stillwind-require-properties" => "stillwind-require-properties",
        "stillwind-intrinsic" => "stillwind-intrinsic",
        "stillwind-format" => "stillwind-format",
        _ => return None,
    };
    let start = prelude.start.min(offset);
    let arguments = &css[start..prelude.end.max(offset)];
    let range = argument_range(arguments, offset - start);
    Some(Context {
        range: start + range.start..start + range.end,
        kind: Kind::DirectiveArgument {
            directive,
            first: arguments[..range.start.min(arguments.len())]
                .trim()
                .is_empty(),
        },
        positions,
        applied: AppliedVariants::default(),
    })
}

/// Returns the applied `@variant` input at `offset` in the header whose `prelude`
/// addresses the original `css`. `positions` and `enclosing` describe the parent, and
/// the cursor must lie inside the header.
fn variant_context(
    css: &str,
    prelude: &Range<usize>,
    offset: usize,
    positions: Positions,
    enclosing: &[Vec<Range<usize>>],
) -> Context {
    let sequences = sequences(css, prelude);
    // An empty sequence inserts at the cursor. Extending its range over adjacent
    // whitespace would make editors filter labels by that whitespace or reject a
    // range that crosses a line break.
    let current = sequences.iter().find(|range| {
        (range.start <= offset && offset <= range.end)
            || (offset < range.start && css[offset..range.start].trim().is_empty())
            || (range.end < offset && css[range.end..offset].trim().is_empty())
    });
    Context {
        range: current.map_or(offset..offset, |range| {
            range.start.min(offset)..range.end.max(offset)
        }),
        kind: Kind::Variant,
        positions,
        applied: AppliedVariants {
            enclosing: enclosing.to_vec(),
            siblings: sequences
                .iter()
                .filter(|range| Some(*range) != current)
                .cloned()
                .collect(),
        },
    }
}

/// Returns the trimmed, non-empty comma-separated sequences of a `@variant` `prelude`
/// as UTF-8 byte ranges in the original `css`.
fn sequences(css: &str, prelude: &Range<usize>) -> Vec<Range<usize>> {
    variant_sequences(&css[prelude.clone()])
        .into_iter()
        .filter(|range| !range.is_empty())
        .map(|range| prelude.start + range.start..prelude.start + range.end)
        .collect()
}

/// Returns the complete whitespace-separated argument around `offset`, including a
/// partial argument at EOF. `offset` may extend past trailing parsed whitespace.
fn argument_range(arguments: &str, offset: usize) -> Range<usize> {
    let before = &arguments[..offset.min(arguments.len())];
    let start = before
        .char_indices()
        .rev()
        .find_map(|(index, character)| {
            character
                .is_whitespace()
                .then_some(index + character.len_utf8())
        })
        .unwrap_or(0);
    let end = arguments[offset.min(arguments.len())..]
        .find(char::is_whitespace)
        .map_or(arguments.len(), |length| offset + length);
    start.min(offset)..end.max(offset)
}

/// Returns the exclusive UTF-8 byte end of the token beginning at `start` in `css`.
///
/// The caller must provide the start of an at-rule token at a valid byte boundary. CSS
/// escapes stay part of the original token range.
fn name_end(css: &str, start: usize) -> usize {
    let mut parser = Parser::new(&css[start..]);
    let _ = parser.next_including_whitespace_and_comments();
    start + parser.position().byte_index()
}

/// Consumes `parser` until it finds a comment, string, or URL containing `offset`, or
/// reaches its end. The parser must reference the same original `css`, and `offset`
/// must be a valid UTF-8 byte boundary in that text.
///
/// Returns whether the cursor lies inside such a token. The end of an unclosed token at
/// EOF counts as inside. Closed token boundaries remain available to the next input.
/// Recursive parsers retain absolute byte positions in `css`.
fn protected(parser: &mut Parser<'_>, css: &str, offset: usize) -> bool {
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            return false;
        };
        let (closer, nested, url) = match token {
            Token::QuotedString(_) | Token::BadString(_) => {
                (Some(&css[start..=start]), false, false)
            }
            Token::UnquotedUrl(_) | Token::BadUrl(_) => (Some(")"), false, false),
            Token::Comment(_) => (Some("*/"), false, false),
            Token::Function(name) => (None, true, name.eq_ignore_ascii_case("url")),
            Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                (None, true, false)
            }
            _ => (None, false, false),
        };
        let end = parser.position().byte_index();
        if let Some(closer) = closer
            && start < offset
            && (offset < end
                || (offset == end && end == css.len() && !closed_literal(&css[start..end], closer)))
        {
            return true;
        }
        if nested {
            let mut found = false;
            let _ = parser.parse_nested_block(|nested| {
                if url {
                    while nested.next_including_whitespace_and_comments().is_ok() {}
                    let end = nested.position().byte_index();
                    found =
                        start <= offset && (offset < end || (offset == end && end == css.len()));
                } else {
                    found = protected(nested, css, offset);
                }
                Ok::<_, ParseError<()>>(())
            });
            if found {
                return true;
            }
        }
    }
}

/// Returns whether the original token `text` ends with its matching `closer`. `closer`
/// must be a quote, `)`, or `*/`. An escaped quote does not close a string.
fn closed_literal(text: &str, closer: &str) -> bool {
    text.ends_with(closer)
        && (!matches!(closer, "'" | "\"")
            || (text.len() > 1
                && text.as_bytes()[..text.len() - 1]
                    .iter()
                    .rev()
                    .take_while(|&&byte| byte == b'\\')
                    .count()
                    .is_multiple_of(2)))
}

#[cfg(test)]
mod tests {
    use cssparser::Parser;

    use super::{Context, Kind, context, protected};

    fn cursor(marked: &str) -> (String, usize) {
        let offset = marked.find('|').unwrap();
        let mut css = marked.to_owned();
        css.remove(offset);
        (css, offset)
    }

    fn assert_context(marked: &str, expected: &str, kind: Kind) {
        let (css, offset) = cursor(marked);
        let Context {
            range,
            kind: actual,
            ..
        } = context(&css, offset).unwrap();
        assert_eq!(actual, kind, "{marked}");
        assert_eq!(&css[range.clone()], expected, "{marked}");
        assert!(range.start <= offset && offset <= range.end, "{marked}");
    }

    #[test]
    fn replacement_ranges_include_the_original_token_after_the_cursor() {
        assert_context(
            "/* 日本語 🦀 */ .a { @app|ly flex; }",
            "@apply",
            Kind::Directive,
        );
        assert_context(".a { @apply fl|ex p-4; }", "flex", Kind::Apply);
        assert_context(
            ".a { @variant dark:ho|ver, focus { color:red; } }",
            "dark:hover",
            Kind::Variant,
        );
        assert_context(
            ".a { @stillwind-slot property-initials inh|erited; }",
            "property-initials inherited",
            Kind::Slot { declarations: true },
        );
        assert_context(
            ".a { color:var(  --color-br|and  , red); }",
            "--color-brand",
            Kind::Theme { runtime: true },
        );
        assert_context(
            "@utility named { @stillwind-require-properties --ex|ample --next; }",
            "--example",
            Kind::DirectiveArgument {
                directive: "stillwind-require-properties",
                first: true,
            },
        );
        assert_context(
            "@stillwind-format --* compose-|space;",
            "compose-space",
            Kind::DirectiveArgument {
                directive: "stillwind-format",
                first: false,
            },
        );
    }

    #[test]
    fn cursor_whitespace_extends_incomplete_headers() {
        assert_context(".a { @apply flex |", "", Kind::Apply);
        assert_context(".a { @variant hover |", "hover ", Kind::Variant);
        assert_context(".a { @variant dark, |", "", Kind::Variant);
        assert_context(
            ".a {
  @variant |
}",
            "",
            Kind::Variant,
        );
        assert_context(".a { @variant  | hover {} }", " hover", Kind::Variant);
        assert_context(
            "@stillwind-slot |",
            "",
            Kind::Slot {
                declarations: false,
            },
        );
    }

    #[test]
    fn conditions_inherit_style_rule_context_and_definitions_reset_it() {
        assert_context(
            "@media print { .a { @supports (display:grid) { @variant ho|ver {} } } }",
            "hover",
            Kind::Variant,
        );
        assert_context(
            ".a { @media print { @stillwind-slot the|me; } }",
            "theme",
            Kind::Slot { declarations: true },
        );
        assert_context(
            "@stillwind-css-template card { @media print { @stillwind-slot util|ities; } }",
            "utilities",
            Kind::Slot {
                declarations: false,
            },
        );
        assert_context(
            ".a { @utility example { @stillwind-slot util|ities; } }",
            "utilities",
            Kind::Slot {
                declarations: false,
            },
        );
        for marked in [
            "@variant ho|ver { @slot; }",
            "@media print { @variant ho|ver { @slot; } }",
            ".a { @utility example { @variant ho|ver { @slot; } } }",
        ] {
            let (css, offset) = cursor(marked);
            assert_eq!(context(&css, offset), None, "{marked}");
        }
    }

    #[test]
    fn variant_ranges_keep_nested_commas_and_select_only_one_alternative() {
        assert_context(
            ".a { @variant [&:is(a,b)]:ho|ver, focus {} }",
            "[&:is(a,b)]:hover",
            Kind::Variant,
        );
        assert_context(
            ".a { @variant [&:is(a,b)]:hover, dark:fo|cus {} }",
            "dark:focus",
            Kind::Variant,
        );
    }

    #[test]
    fn nested_theme_references_have_their_own_argument_context() {
        assert_context(
            ".a { color:var(--outer, --theme( --color-br|and, red)); }",
            "--color-brand",
            Kind::Theme { runtime: false },
        );
        assert_context(
            ".a { color:theme(--ou|ter, var(--inner)); }",
            "--outer",
            Kind::Theme { runtime: false },
        );
        let (css, offset) = cursor(".a { color:var(--outer, re|d); }");
        assert_eq!(context(&css, offset), None);
    }

    #[test]
    fn literal_protection_distinguishes_closed_boundaries_from_incomplete_tokens() {
        for (marked, expected) in [
            ("/* theme(--co|lor) */", true),
            ("/* theme(--color)|", true),
            ("/* theme(--color)'|", true),
            ("/* theme(--color) */|", false),
            (".a { content:'theme(--co|lor)'; }", true),
            (".a { content:'theme(--color)|", true),
            (".a { content:'|", true),
            (r".a { content:'theme(--color)\'|", true),
            (".a { content:'theme(--color)'|", false),
            (".a { background:url('theme(--co|lor)'); }", true),
            (".a { background:url('theme(--color)|", true),
            (".a { background:url('x')|", false),
            (".a { background:url(theme(--co|lor)); }", true),
            (
                ".a { background:image-set(url('x'), var(--co|lor)); }",
                false,
            ),
        ] {
            let (css, offset) = cursor(marked);
            assert_eq!(
                protected(&mut Parser::new(&css), &css, offset),
                expected,
                "{marked}"
            );
        }
    }
}
