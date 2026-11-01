//! Conditions on a shadow host.
//!
//! Inside its shadow tree, the host matches only `:host`, `:host()`, and
//! `:host-context()`, so `:host:hover` matches nothing and `:host(:hover)` is the form
//! that tests the host. [`fold_host_conditions`] moves such conditions into `:host()`.
//! [`fold_nested_host_conditions`] does the same for a rule nested under a host, and
//! [`selector_cannot_match_host`] finds selectors that no such move can repair.

use std::ops::Range;

use cssparser::{Parser, Token};
use lean_string::LeanString;

use crate::parser::consume_css_block;

/// One simple selector of a [`Compound`], before its pseudo-elements.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Part {
    /// `:host`, or `:host()` with the range of its argument.
    Host(Option<Range<usize>>),
    /// `:host-context()`, which stays in the compound as written.
    HostContext(Range<usize>),
    /// `&`, which stays in the compound as written.
    Nesting(Range<usize>),
    /// A selector that tests the element, such as `:hover`, `.open`, or `[open]`.
    Condition(Range<usize>),
}

/// A compound selector of a selector list.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Compound {
    /// Range of the parts, which ends before the first pseudo-element.
    range: Range<usize>,
    /// Simple selectors in `range`, in authored order.
    parts: Vec<Part>,
    /// Whether the compound contains a type selector, a universal selector, a comment,
    /// or a token that this module does not interpret. `parts` is then incomplete.
    opaque: bool,
    /// Whether an earlier compound of the same complex selector precedes this one.
    has_predecessor: bool,
    /// Whether this is the last compound of its complex selector.
    last: bool,
}

impl Compound {
    fn selects_host(&self) -> bool {
        self.parts
            .iter()
            .any(|part| matches!(part, Part::Host(_) | Part::HostContext(_)))
    }

    fn uses_nesting(&self) -> bool {
        self.parts
            .iter()
            .any(|part| matches!(part, Part::Nesting(_)))
    }

    fn conditions(&self) -> impl Iterator<Item = &Range<usize>> {
        self.parts.iter().filter_map(|part| match part {
            Part::Condition(range) => Some(range),
            _ => None,
        })
    }
}

/// Returns the compounds of `selector` in authored order. A compound without simple
/// selectors, such as one that starts with a pseudo-element, is included when it ends
/// a complex selector.
fn compounds(selector: &str) -> Vec<Compound> {
    let mut parser = Parser::new(selector);
    let mut output = Vec::new();
    let mut current: Option<Compound> = None;
    let mut pseudo_element = false;
    let mut has_predecessor = false;
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            finish(&mut output, &mut current, true);
            return output;
        };
        let token = token.clone();
        if matches!(
            token,
            Token::WhiteSpace(_) | Token::Comma | Token::Delim('>' | '+' | '~' | '|')
        ) {
            let branch_end = matches!(token, Token::Comma);
            has_predecessor = !branch_end && (has_predecessor || current.is_some());
            finish(&mut output, &mut current, branch_end);
            pseudo_element = false;
            continue;
        }
        let compound = current.get_or_insert_with(|| Compound {
            range: start..start,
            has_predecessor,
            ..Compound::default()
        });
        if pseudo_element {
            skip_block(&mut parser, &token);
            continue;
        }
        let part = match token {
            Token::Delim('&') => Some(Part::Nesting(start..start + 1)),
            Token::Delim('.') => parser
                .next_including_whitespace_and_comments()
                .is_ok()
                .then(|| Part::Condition(start..parser.position().byte_index())),
            Token::IDHash(_) => Some(Part::Condition(start..parser.position().byte_index())),
            Token::SquareBracketBlock => {
                skip_block(&mut parser, &token);
                Some(Part::Condition(start..parser.position().byte_index()))
            }
            Token::Colon => match parser.next_including_whitespace_and_comments() {
                Ok(Token::Colon) => {
                    pseudo_element = true;
                    continue;
                }
                Ok(Token::Ident(name)) if is_legacy_pseudo_element(name) => {
                    pseudo_element = true;
                    continue;
                }
                Ok(Token::Ident(name)) if name.eq_ignore_ascii_case("host") => {
                    Some(Part::Host(None))
                }
                Ok(Token::Ident(_)) => Some(Part::Condition(start..parser.position().byte_index())),
                Ok(Token::Function(name)) => {
                    let name = name.clone();
                    let arguments = parser.position().byte_index();
                    let closed = consume_css_block(&mut parser).is_ok();
                    let end = parser.position().byte_index();
                    if !closed {
                        None
                    } else if name.eq_ignore_ascii_case("host") {
                        Some(Part::Host(Some(arguments..end - 1)))
                    } else if name.eq_ignore_ascii_case("host-context") {
                        Some(Part::HostContext(start..end))
                    } else {
                        Some(Part::Condition(start..end))
                    }
                }
                _ => None,
            },
            token => {
                skip_block(&mut parser, &token);
                None
            }
        };
        match part {
            Some(part) => compound.parts.push(part),
            None => compound.opaque = true,
        }
        compound.range.end = parser.position().byte_index();
    }
}

fn finish(output: &mut Vec<Compound>, current: &mut Option<Compound>, branch_end: bool) {
    if let Some(compound) = current.take() {
        output.push(compound);
    }
    if branch_end && let Some(last) = output.last_mut() {
        last.last = true;
    }
}

fn skip_block(parser: &mut Parser<'_>, token: &Token<'_>) {
    if matches!(
        token,
        Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock
    ) {
        let _ = consume_css_block(parser);
    }
}

fn is_legacy_pseudo_element(name: &str) -> bool {
    ["before", "after", "first-line", "first-letter"]
        .iter()
        .any(|pseudo| name.eq_ignore_ascii_case(pseudo))
}

fn mentions_host(selector: &str) -> bool {
    selector
        .as_bytes()
        .windows(5)
        .any(|window| window.eq_ignore_ascii_case(b":host"))
}

/// Moves each condition in a compound that selects a shadow host into `:host()`.
///
/// `:host:hover` becomes `:host(:hover)`, `:host(.a):hover` becomes `:host(.a:hover)`,
/// and `:host-context(.a):hover` becomes `:host-context(.a):host(:hover)`.
/// Pseudo-elements, combinators, and later compounds stay after the result. The
/// specificity of the selector does not change. The arguments of `:is()`, `:where()`,
/// and `:not()` receive the same treatment. A compound with a comment or a type
/// selector outside `:host()` stays as written.
pub fn fold_host_conditions(selector: &str) -> LeanString {
    if mentions_host(selector) {
        fold(selector, &Compound::selects_host)
    } else {
        selector.into()
    }
}

/// Moves the conditions of each compound with `&` or a host into `:host()`, for a rule
/// nested under a selector that selects a shadow host.
///
/// `&:hover` becomes `&:host(:hover)`. The caller must establish the parent with
/// [`selector_selects_host`]. See [`fold_host_conditions`] for the other forms.
pub fn fold_nested_host_conditions(selector: &str) -> LeanString {
    fold(selector, &|compound| {
        compound.selects_host() || compound.uses_nesting()
    })
}

fn fold(selector: &str, applies: &dyn Fn(&Compound) -> bool) -> LeanString {
    let selector = fold_arguments(selector, applies);
    let selector = selector.as_str();
    let mut output = LeanString::new();
    let mut cursor = 0;
    for compound in compounds(selector) {
        if compound.opaque || compound.conditions().next().is_none() || !applies(&compound) {
            continue;
        }
        output.push_str(&selector[cursor..compound.range.start]);
        cursor = compound.range.end;
        let mut arguments = LeanString::new();
        let mut conditions = LeanString::new();
        for part in &compound.parts {
            match part {
                Part::Host(Some(range)) => arguments.push_str(selector[range.clone()].trim()),
                Part::Host(None) => {}
                Part::HostContext(range) | Part::Nesting(range) => {
                    output.push_str(&selector[range.clone()]);
                }
                Part::Condition(range) => conditions.push_str(&selector[range.clone()]),
            }
        }
        output.push_str(":host(");
        output.push_str(&arguments);
        output.push_str(&conditions);
        output.push(')');
    }
    output.push_str(&selector[cursor..]);
    output
}

/// Applies [`fold`] to the arguments of each `:is()`, `:where()`, and `:not()` in
/// `selector`. A compound stays whole because these arguments keep their positions.
fn fold_arguments(selector: &str, applies: &dyn Fn(&Compound) -> bool) -> LeanString {
    let mut output = LeanString::new();
    let mut cursor = 0;
    for compound in compounds(selector) {
        for range in compound.conditions() {
            let condition = &selector[range.clone()];
            let Some(name) = [":is(", ":where(", ":not("].iter().find(|name| {
                condition
                    .get(..name.len())
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(name))
            }) else {
                continue;
            };
            let start = range.start + name.len();
            let end = range.end - 1;
            output.push_str(&selector[cursor..start]);
            output.push_str(&fold(&selector[start..end], applies));
            cursor = end;
        }
    }
    output.push_str(&selector[cursor..]);
    output
}

/// Whether every branch of `selector` ends with a compound that selects a shadow host.
///
/// A nested rule with `&` then refers to the host in every branch. A branch that ends
/// with a pseudo-element does not qualify, because `&` cannot represent it.
pub fn selector_selects_host(selector: &str) -> bool {
    if !mentions_host(selector) {
        return false;
    }
    let compounds = compounds(selector);
    let mut finals = compounds.iter().filter(|compound| compound.last).peekable();
    finals.peek().is_some()
        && finals.all(|compound| {
            compound.selects_host() && !selector[compound.range.end..].starts_with(':')
        })
}

/// Whether `selector` tests a shadow host in a way that cannot match.
///
/// Elements outside a shadow tree are not visible to selectors inside it, so a compound
/// before the host, as in `.page :host`, prevents a match. So does a complex selector
/// in `:is()` or `:where()` inside `:host()`, as in `:host(:is(.page *))`.
/// Apply [`fold_host_conditions`] first, because this check reads `:host()` arguments.
pub fn selector_cannot_match_host(selector: &str) -> bool {
    if !mentions_host(selector) {
        return false;
    }
    compounds(selector).iter().any(|compound| {
        compound.selects_host()
            && (compound.has_predecessor
                || compound.parts.iter().any(|part| match part {
                    Part::Host(Some(range)) => tests_ancestors(&selector[range.clone()]),
                    _ => false,
                }))
    })
}

/// Whether a compound selector contains `:is()` or `:where()` with a complex selector.
fn tests_ancestors(selector: &str) -> bool {
    compounds(selector).iter().any(|compound| {
        compound.conditions().any(|range| {
            let condition = &selector[range.clone()];
            [":is(", ":where("].iter().any(|name| {
                condition
                    .get(..name.len())
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(name))
                    && compounds(&condition[name.len()..condition.len() - 1])
                        .iter()
                        .any(|inner| inner.has_predecessor)
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_conditions_move_into_the_functional_form() {
        for (selector, expected) in [
            (":host", ":host"),
            (":host(.a)", ":host(.a)"),
            (":host:hover", ":host(:hover)"),
            (":HOST:hover", ":host(:hover)"),
            (":host.open[data-x]", ":host(.open[data-x])"),
            (":host(.a):hover", ":host(.a:hover)"),
            (":host( article ):hover", ":host(article:hover)"),
            (":host:not(.a, .b)", ":host(:not(.a, .b))"),
            (":host:hover::before", ":host(:hover)::before"),
            (":host:hover:before", ":host(:hover):before"),
            (":host::before:hover", ":host::before:hover"),
            (":host:hover > .item", ":host(:hover) > .item"),
            (":host .item:hover", ":host .item:hover"),
            (":host:hover, :host:focus", ":host(:hover), :host(:focus)"),
            (".a:hover, :host:hover", ".a:hover, :host(:hover)"),
            (
                ":host-context(.dark):hover",
                ":host-context(.dark):host(:hover)",
            ),
            (
                ":host(.a):host-context(.b):hover",
                ":host-context(.b):host(.a:hover)",
            ),
            ("article:host:hover", "article:host:hover"),
            (":not(:host:hover)", ":not(:host(:hover))"),
            (":is(:host:hover > *)", ":is(:host(:hover) > *)"),
            (
                ":where(:is(:host:hover) > .a)",
                ":where(:is(:host(:hover)) > .a)",
            ),
            (":is(:host:hover > *):focus", ":is(:host(:hover) > *):focus"),
            (r#"[data-x=":host:hover"]"#, r#"[data-x=":host:hover"]"#),
        ] {
            assert_eq!(fold_host_conditions(selector), expected, "{selector}");
        }
    }

    #[test]
    fn nested_rules_keep_the_nesting_selector_before_the_host_conditions() {
        for (selector, expected) in [
            ("&:hover", "&:host(:hover)"),
            ("&:hover::before", "&:host(:hover)::before"),
            ("&:hover > .item", "&:host(:hover) > .item"),
            ("&:hover, &:focus", "&:host(:hover), &:host(:focus)"),
            ("&", "&"),
            ("& > .item", "& > .item"),
            ("&:host(:hover)", "&:host(:hover)"),
            (".item:hover", ".item:hover"),
        ] {
            assert_eq!(
                fold_nested_host_conditions(selector),
                expected,
                "{selector}"
            );
        }
    }

    #[test]
    fn a_selector_selects_the_host_only_in_every_final_compound() {
        for (selector, expected) in [
            (":host", true),
            (":host(.a)", true),
            (":host-context(.a)", true),
            ("&:host(:hover)", true),
            (":host, :host(.a)", true),
            (":host, .a", false),
            (":host .item", false),
            (":host::before", false),
            (":where(:root, :host)", false),
            (".a", false),
        ] {
            assert_eq!(selector_selects_host(selector), expected, "{selector}");
        }
    }

    #[test]
    fn selectors_that_look_outside_the_shadow_tree_cannot_match() {
        for (selector, expected) in [
            (":host", false),
            (":host(:hover)", false),
            (":host > .item", false),
            (":host(:is(.a, .b))", false),
            (":host(:not(.page *))", false),
            (".page", false),
            (".page :host", true),
            (":where(.page) :host(.a)", true),
            (".page > :host", true),
            (":host(:is(.page *))", true),
            (":host(:is(:where(.group):hover *))", true),
            (":host(.a:where(.page > *))", true),
            (":host, .page :host", true),
        ] {
            assert_eq!(selector_cannot_match_host(selector), expected, "{selector}");
        }
    }
}
