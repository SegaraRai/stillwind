//! Normalizes scoped declarations and retains style ancestors when lifting scopes.

use std::{fmt::Write as _, ops::Range};

use cssparser::{ParseError, Parser, Token};
use lightningcss::selector::SelectorList;
use lightningcss::stylesheet::{ParserOptions, PrinterOptions};
use lightningcss::traits::{ParseWithOptions, ToCss};

use crate::{
    CssBlock, CssNode, CssNodeKind, absolutize_nested_selector, instantiate_selector,
    parse_structure, selector_is_element_condition, selector_list_branches, selector_uses_subject,
    validate_css_syntax,
};

/// Selectors delimiting a scope. Missing selectors retain the implicit root or limit.
#[derive(Clone, Copy, Debug)]
pub struct ScopePrelude<'a> {
    /// Scope-start selector list, without parentheses.
    pub start: Option<&'a str>,
    /// Scope-end selector list, without parentheses.
    pub end: Option<&'a str>,
}

impl ScopePrelude<'_> {
    /// Whether the start selector refers to the surrounding style rule.
    pub fn uses_subject(self) -> bool {
        self.start.is_some_and(selector_uses_subject)
    }

    /// Binds a variant's explicit nesting selectors to `subject` and its scope root.
    /// Selectors without nesting selectors keep their document-relative meaning.
    /// `subject` must be a valid selector list for the surrounding style rule.
    pub fn bind_variant(self, subject: &str) -> String {
        let mut result = String::new();
        if let Some(start) = self.start {
            let selector = instantiate_selector(start, &format!(":is({subject})"));
            let _ = write!(result, " ({selector})");
        }
        if let Some(end) = self.end {
            let selector = instantiate_selector(end, ":where(:scope)");
            let _ = write!(result, " to ({selector})");
        }
        result
    }
}

/// Parses a complete scope prelude, including empty preludes and end-only scopes.
/// Returns `None` for malformed grammar or selector lists.
pub fn parse_scope_prelude(prelude: &str) -> Option<ScopePrelude<'_>> {
    validate_css_syntax(prelude).ok()?;
    let mut parser = Parser::new(prelude);
    let (mut start, mut end, mut limit) = (None, None, false);
    while let Ok(token) = parser.next() {
        match token {
            Token::Ident(name) if name.eq_ignore_ascii_case("to") && !limit => limit = true,
            Token::ParenthesisBlock => {
                let mut text = "";
                let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                    let position = inner.position();
                    while inner.next_including_whitespace_and_comments().is_ok() {}
                    text = inner.slice_from(position);
                    Ok(())
                });
                if !scope_selector_is_valid(text)
                    || (limit && end.is_some())
                    || (!limit && start.is_some())
                {
                    return None;
                }
                if limit {
                    end = Some(text);
                } else {
                    start = Some(text);
                }
            }
            _ => return None,
        }
    }
    (!(limit && end.is_none())).then_some(ScopePrelude { start, end })
}

/// Checks real-element selectors after parsing removes invalid forgiving branches.
fn scope_selector_is_valid(selector: &str) -> bool {
    SelectorList::parse_string_with_options(selector, ParserOptions::default())
        .ok()
        .and_then(|selector| selector.to_css_string(PrinterOptions::default()).ok())
        .is_some_and(|selector| selector_is_element_condition(&selector, false))
}

/// CSS normalized for parsers that do not accept declarations directly in `@scope`.
pub struct ScopedCss {
    /// Complete transformed stylesheet.
    pub css: String,
    /// Original byte offset for each transformed byte, including inserted syntax.
    /// Empty when no scope normalization is needed.
    pub origins: Vec<usize>,
    /// A style ancestor remains around a scope whose root depends on stylesheet
    /// placement.
    pub implicit_style_scope: bool,
}

impl ScopedCss {
    fn copy(&mut self, css: &str, range: Range<usize>) {
        self.css.push_str(&css[range.clone()]);
        self.origins.extend(range);
    }

    fn insert(&mut self, text: &str, origin: usize) {
        self.css.push_str(text);
        self.origins.extend(std::iter::repeat_n(origin, text.len()));
    }
}

/// Wraps scoped declarations and lifts scopes through style ancestors without losing
/// their selectors. Nested scopes retain their order and boundaries. Ordinary rules
/// without scopes retain their CSS nesting. Input must be balanced CSS.
/// Scopes with implicit roots retain their style ancestors.
/// Compiler `@stillwind-scope` wrappers instead retain the utility selector inside
/// absolute roots and bind explicit nesting selectors to the root.
pub fn normalize_scopes(css: &str) -> ScopedCss {
    if !css
        .as_bytes()
        .windows(6)
        .any(|text| text.eq_ignore_ascii_case(b"@scope"))
        && !css.contains("@stillwind-scope")
        || validate_css_syntax(css).is_err()
    {
        return ScopedCss {
            css: css.to_string(),
            origins: Vec::new(),
            implicit_style_scope: false,
        };
    }
    let mut output = ScopedCss {
        css: String::new(),
        origins: Vec::new(),
        implicit_style_scope: false,
    };
    walk(
        css,
        &parse_structure(css),
        0..css.len(),
        None,
        false,
        &mut output,
    );
    output
}

fn contains_scope(node: &CssNode) -> bool {
    matches!(&node.kind, CssNodeKind::AtRule { name, .. } if name == "scope" || name == "stillwind-scope")
        || node.children().iter().any(contains_scope)
}

fn nested_selector(selector: &str, parent: &str) -> String {
    instantiate_selector(
        &absolutize_nested_selector(selector),
        &format!(":is({parent})"),
    )
    .to_string()
}

fn scoped_selector(selector: &str) -> String {
    // A relative branch such as `> .b` in `.a, > .b` is invalid once the selector
    // becomes a parent inside `:is()`, so any relative branch absolutizes the list.
    let relative = selector_list_branches(selector)
        .iter()
        .any(|branch| branch.starts_with(['>', '+', '~']));
    let text = if relative {
        absolutize_nested_selector(selector).to_string()
    } else {
        selector.to_string()
    };
    instantiate_selector(&text, ":where(:scope)").to_string()
}

fn walk(
    css: &str,
    nodes: &[CssNode],
    range: Range<usize>,
    parent: Option<&str>,
    scoped: bool,
    out: &mut ScopedCss,
) {
    let mut cursor = range.start;
    let mut index = 0;
    while let Some(node) = nodes.get(index) {
        index += 1;
        out.copy(css, cursor..node.range.start);
        match &node.kind {
            CssNodeKind::AtRule {
                name,
                prelude,
                block: Some(block),
            } if name == "stillwind-scope" => {
                variant_scope_rule(css, node, prelude, block, parent, out);
            }
            CssNodeKind::AtRule {
                name,
                prelude,
                block: Some(block),
            } if name == "scope" => {
                scope_rule(css, node, prelude, block, parent, out);
            }
            CssNodeKind::StyleRule { selector, block } if contains_scope(node) => {
                let raw = &css[selector.clone()];
                let resolved = parent.map_or_else(|| raw.to_string(), |p| nested_selector(raw, p));
                let resolved = if parent.is_none() && scoped {
                    scoped_selector(&resolved)
                } else {
                    resolved
                };
                walk(
                    css,
                    &block.children,
                    block.contents.clone(),
                    Some(&resolved),
                    scoped,
                    out,
                );
            }
            CssNodeKind::AtRule {
                name,
                block: Some(block),
                ..
            } if (contains_scope(node) || scoped || parent.is_some())
                && matches!(
                    name.as_str(),
                    "media"
                        | "supports"
                        | "container"
                        | "layer"
                        | "starting-style"
                        | "-stillwind-rule-boundary"
                ) =>
            {
                out.copy(css, node.range.start..block.contents.start);
                walk(
                    css,
                    &block.children,
                    block.contents.clone(),
                    parent,
                    scoped,
                    out,
                );
                out.insert("}", node.range.end.saturating_sub(1));
            }
            CssNodeKind::Declaration { .. } if scoped || parent.is_some() => {
                // Consecutive declarations share one rule. A nested rule between
                // declarations starts a new rule, which keeps the authored order.
                let mut end = node.range.end;
                while let Some(next) = nodes
                    .get(index)
                    .filter(|next| matches!(next.kind, CssNodeKind::Declaration { .. }))
                {
                    end = next.range.end;
                    index += 1;
                }
                out.insert(parent.unwrap_or(":where(:scope)"), node.range.start);
                out.insert("{", node.range.start);
                out.copy(css, node.range.start..end);
                if !css[..end].ends_with(';') {
                    out.insert(";", end);
                }
                out.insert("}", end);
                cursor = end;
                continue;
            }
            CssNodeKind::StyleRule { selector, .. } if scoped && parent.is_none() => {
                out.insert(&scoped_selector(&css[selector.clone()]), selector.start);
                out.copy(css, selector.end..node.range.end);
            }
            _ => {
                if let Some(parent) = parent {
                    out.insert(parent, node.range.start);
                    out.insert("{", node.range.start);
                }
                out.copy(css, node.range.clone());
                if parent.is_some() {
                    out.insert("}", node.range.end);
                }
            }
        }
        cursor = node.range.end;
    }
    out.copy(css, cursor..range.end);
}

/// Writes a compiler scope wrapper as `@scope` with its nesting selectors bound to
/// `parent`. Omits the wrapper and its contents when the bound prelude cannot select an
/// element, such as `(.card :is(.x::before))`, because a browser ignores that rule.
fn variant_scope_rule(
    css: &str,
    node: &CssNode,
    prelude: &Range<usize>,
    block: &CssBlock,
    parent: Option<&str>,
    out: &mut ScopedCss,
) {
    let Some(scope) = parse_scope_prelude(&css[prelude.clone()]) else {
        return;
    };
    let bound = scope.bind_variant(parent.unwrap_or("&"));
    if parse_scope_prelude(&bound).is_none() {
        return;
    }
    out.insert("@scope", node.range.start);
    out.insert(&bound, prelude.start);
    out.insert("{", block.contents.start);
    let inner = Some(if scope.uses_subject() {
        ":where(:scope)"
    } else {
        parent.unwrap_or("&")
    });
    walk(
        css,
        &block.children,
        block.contents.clone(),
        inner,
        true,
        out,
    );
    out.insert("}", node.range.end.saturating_sub(1));
}

fn scope_rule(
    css: &str,
    node: &CssNode,
    prelude: &Range<usize>,
    block: &CssBlock,
    parent: Option<&str>,
    out: &mut ScopedCss,
) {
    let implicit = parent.is_some()
        && !matches!(
            Parser::new(&css[prelude.clone()]).next(),
            Ok(Token::ParenthesisBlock)
        );
    if implicit {
        out.implicit_style_scope = true;
        out.insert(parent.unwrap_or(""), node.range.start);
        out.insert("{", node.range.start);
    }
    out.insert("@scope", node.range.start);
    scope_prelude(
        &css[prelude.clone()],
        if implicit { None } else { parent },
        prelude.start,
        out,
    );
    out.insert("{", block.contents.start);
    walk(
        css,
        &block.children,
        block.contents.clone(),
        None,
        true,
        out,
    );
    out.insert("}", node.range.end.saturating_sub(1));
    if implicit {
        out.insert("}", node.range.end);
    }
}

fn scope_prelude(prelude: &str, parent: Option<&str>, origin: usize, out: &mut ScopedCss) {
    let mut parser = Parser::new(prelude);
    let mut start = None;
    let mut end = None;
    let mut limit = false;
    let mut invalid = false;
    while let Ok(token) = parser.next() {
        match token {
            Token::Ident(name) if name.eq_ignore_ascii_case("to") && !limit => limit = true,
            Token::ParenthesisBlock => {
                let mut text = "";
                let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                    let position = inner.position();
                    while inner.next_including_whitespace_and_comments().is_ok() {}
                    text = inner.slice_from(position);
                    Ok(())
                });
                if (limit && end.is_some()) || (!limit && start.is_some()) {
                    invalid = true;
                }
                if limit {
                    end = Some(text);
                } else {
                    start = Some(text);
                }
            }
            _ => invalid = true,
        }
    }
    if invalid || (limit && end.is_none()) {
        out.insert(" ", origin);
        out.insert(prelude, origin);
        return;
    }
    if let Some(selector) = start {
        let selector =
            parent.map_or_else(|| selector.to_string(), |p| nested_selector(selector, p));
        out.insert(&format!(" ({selector})"), origin);
    } else if let Some(parent) = parent {
        out.insert(&format!(" ({parent})"), origin);
    }
    if let Some(selector) = end {
        let selector = if selector_uses_subject(selector) {
            instantiate_selector(selector, ":where(:scope)").to_string()
        } else {
            selector.to_string()
        };
        out.insert(&format!(" to ({selector})"), origin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_scopes_bind_only_explicit_subjects() {
        for (prelude, expected) in [
            ("(:is(.card,::before))", " (:is(.card,::before))"),
            ("(.card) to (& .stop)", " (.card) to (:where(:scope) .stop)"),
            ("(.card &)", " (.card :is(.target))"),
            ("", ""),
            ("to (.stop)", " to (.stop)"),
        ] {
            assert_eq!(
                parse_scope_prelude(prelude)
                    .unwrap()
                    .bind_variant(".target"),
                expected
            );
        }
        for prelude in [
            "()",
            "to",
            "(.a) (.b)",
            "(.a) to (.b) to (.c)",
            "(.a,)",
            "(.a",
            "(::before)",
        ] {
            assert!(parse_scope_prelude(prelude).is_none(), "{prelude}");
        }
    }

    #[test]
    fn scope_declarations_and_ancestors_keep_their_order() {
        let css = ".parent{@scope (.from) to (& .stop){color:red;margin:0;:where(:scope){color:blue}color:green}}";
        let result = normalize_scopes(css);
        assert_eq!(
            result.css,
            "@scope (:is(.parent) .from) to (:where(:scope) .stop){:where(:scope){color:red;margin:0;}:where(:scope){color:blue}:where(:scope){color:green;}}"
        );
        assert_eq!(result.css.len(), result.origins.len());
    }

    #[test]
    fn relative_branches_in_scoped_parents_bind_to_the_scope_root() {
        let css = "@scope (.s){.a, > .b{@scope (.c){color:red}}}";
        assert_eq!(
            normalize_scopes(css).css,
            "@scope (.s){@scope (:is(:where(:scope) .a, :where(:scope) > .b) .c){:where(:scope){color:red;}}}"
        );
    }
}
