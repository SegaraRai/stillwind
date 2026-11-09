//! Normalizes scoped declarations and retains style ancestors when lifting scopes.

use std::ops::Range;

use cssparser::{ParseError, Parser, Token};

use crate::{
    CssBlock, CssNode, CssNodeKind, absolutize_nested_selector, instantiate_selector,
    parse_structure, selector_uses_subject, validate_css_syntax,
};

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
pub fn normalize_scopes(css: &str) -> ScopedCss {
    if !css
        .as_bytes()
        .windows(6)
        .any(|text| text.eq_ignore_ascii_case(b"@scope"))
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
    matches!(&node.kind, CssNodeKind::AtRule { name, .. } if name == "scope")
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
    let text = if selector.trim_start().starts_with(['>', '+', '~']) {
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
    for node in nodes {
        out.copy(css, cursor..node.range.start);
        match &node.kind {
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
                    "media" | "supports" | "container" | "layer" | "starting-style"
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
                out.insert(parent.unwrap_or(":where(:scope)"), node.range.start);
                out.insert("{", node.range.start);
                out.copy(css, node.range.clone());
                if !css[node.range.clone()].ends_with(';') {
                    out.insert(";", node.range.end);
                }
                out.insert("}", node.range.end);
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
    fn scope_declarations_and_ancestors_keep_their_order() {
        let css = ".parent{@scope (.from) to (& .stop){color:red;:where(:scope){color:blue}color:green;}}";
        let result = normalize_scopes(css);
        assert_eq!(
            result.css,
            "@scope (:is(.parent) .from) to (:where(:scope) .stop){:where(:scope){color:red;}:where(:scope){color:blue}:where(:scope){color:green;}}"
        );
        assert_eq!(result.css.len(), result.origins.len());
    }
}
