//! Moves element conditions out of nested pseudo-element rules.
//!
//! CSS Nesting binds `&` in `:is(.group:hover &)` to the whole parent selector. When
//! the parent selects a pseudo-element, as `&::before` does, the result contains a
//! pseudo-element inside `:is()` and never matches. The main entry point is
//! [`hoist_pseudo_element_conditions`].

use std::ops::Range;

use super::list::{selector_list_branches, split_pseudo_element};
use super::subject::is_wrapped_subject;
use crate::{CssBlock, CssNode, CssNodeKind, parse_structure, validate_css_syntax};

/// Rewrites nested rules so that wrapper conditions such as `:is(.group:hover &)` test
/// the element that originates a pseudo-element.
///
/// A wrapper rule directly inside a pseudo-element rule moves after it, and the
/// pseudo-element moves inside the wrapper:
/// `&::before{content:"";:is(.g &){color:red}}` becomes
/// `&::before{content:"";}&{:is(.g &){&::before{color:red}}}`. The wrapper may sit
/// inside at-rules or `&` rules that contain nothing else; those rules move with it.
/// The other contents of the pseudo-element rule keep their order around the moved
/// rule. Each branch of a pseudo-element selector list gets its own copy.
///
/// Returns `None` when `css` is not balanced CSS or no rule changes. A wrapper that
/// shares an at-rule with other contents stays in place.
pub fn hoist_pseudo_element_conditions(css: &str) -> Option<String> {
    if !css.contains(":is(") || validate_css_syntax(css).is_err() {
        return None;
    }
    let mut output = String::with_capacity(css.len());
    write_nodes(css, &parse_structure(css), 0..css.len(), &mut output).then_some(output)
}

/// A wrapper rule and the rules with one child around it, outermost first.
struct Chain<'a> {
    containers: Vec<&'a CssNode>,
    rule: &'a CssNode,
}

/// Writes `nodes` and the text between them. Returns whether any rule changed.
fn write_nodes(css: &str, nodes: &[CssNode], range: Range<usize>, out: &mut String) -> bool {
    let mut cursor = range.start;
    let mut changed = false;
    for node in nodes {
        out.push_str(&css[cursor..node.range.start]);
        changed |= write_node(css, node, out);
        cursor = node.range.end;
    }
    out.push_str(&css[cursor..range.end]);
    changed
}

fn write_node(css: &str, node: &CssNode, out: &mut String) -> bool {
    let Some(block) = block(node) else {
        out.push_str(&css[node.range.clone()]);
        return false;
    };
    if let CssNodeKind::StyleRule { selector, .. } = &node.kind
        && let Some(branches) = selector_list_branches(&css[selector.clone()])
            .into_iter()
            .map(split_pseudo_element)
            .collect::<Option<Vec<_>>>()
        && block
            .children
            .iter()
            .any(|child| chain(css, child).is_some())
    {
        split_rule(css, node, block, &branches, out);
        return true;
    }
    out.push_str(&css[node.range.start..block.contents.start]);
    let changed = write_nodes(css, &block.children, block.contents.clone(), out);
    out.push_str(&css[block.contents.end..node.range.end]);
    changed
}

/// Writes a pseudo-element rule as segments of its other contents, with the moved
/// wrapper rules between them.
fn split_rule(
    css: &str,
    node: &CssNode,
    block: &CssBlock,
    branches: &[(String, &str)],
    out: &mut String,
) {
    let header = &css[node.range.start..block.contents.start];
    let footer = &css[block.contents.end..node.range.end];
    let flush = |segment: &mut String, out: &mut String| {
        if !segment.trim().is_empty() {
            out.push_str(header);
            out.push_str(segment);
            out.push_str(footer);
        }
        segment.clear();
    };
    let mut segment = String::new();
    let mut cursor = block.contents.start;
    for child in &block.children {
        segment.push_str(&css[cursor..child.range.start]);
        cursor = child.range.end;
        let Some(chain) = chain(css, child) else {
            write_node(css, child, &mut segment);
            continue;
        };
        flush(&mut segment, out);
        for (element, pseudo_element) in branches {
            out.push_str(element);
            out.push('{');
            write_chain(css, &chain, pseudo_element, out);
            out.push('}');
        }
    }
    segment.push_str(&css[cursor..block.contents.end]);
    flush(&mut segment, out);
}

/// Writes the containers and the wrapper rule of `chain`, with the wrapper's contents
/// inside a rule that selects `pseudo_element` of the wrapper's element.
fn write_chain(css: &str, chain: &Chain<'_>, pseudo_element: &str, out: &mut String) {
    for container in &chain.containers {
        if let Some(block) = block(container) {
            out.push_str(&css[container.range.start..block.contents.start]);
        }
    }
    if let Some(block) = block(chain.rule) {
        out.push_str(&css[chain.rule.range.start..block.contents.start]);
        out.push('&');
        out.push_str(pseudo_element);
        out.push('{');
        write_nodes(css, &block.children, block.contents.clone(), out);
        out.push('}');
        out.push_str(&css[block.contents.end..chain.rule.range.end]);
    }
    for container in chain.containers.iter().rev() {
        if let Some(block) = block(container) {
            out.push_str(&css[block.contents.end..container.range.end]);
        }
    }
}

/// Returns the wrapper rule that `node` is, or contains through rules with one child.
fn chain<'a>(css: &str, node: &'a CssNode) -> Option<Chain<'a>> {
    match &node.kind {
        CssNodeKind::StyleRule { selector, .. } if is_wrapped_subject(&css[selector.clone()]) => {
            Some(Chain {
                containers: Vec::new(),
                rule: node,
            })
        }
        CssNodeKind::StyleRule { selector, block } if css[selector.clone()].trim() == "&" => {
            contained_chain(css, node, block)
        }
        CssNodeKind::AtRule {
            block: Some(block), ..
        } => contained_chain(css, node, block),
        _ => None,
    }
}

fn contained_chain<'a>(css: &str, node: &'a CssNode, block: &'a CssBlock) -> Option<Chain<'a>> {
    let [child] = block.children.as_slice() else {
        return None;
    };
    let mut chain = chain(css, child)?;
    chain.containers.insert(0, node);
    Some(chain)
}

fn block(node: &CssNode) -> Option<&CssBlock> {
    match &node.kind {
        CssNodeKind::StyleRule { block, .. }
        | CssNodeKind::AtRule {
            block: Some(block), ..
        } => Some(block),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapper_conditions_test_the_originating_element() {
        for (css, expected) in [
            (
                r#".a{&::before{content:"";:is(.g &){color:red}color:blue}}"#,
                r#".a{&::before{content:"";}&{:is(.g &){&::before{color:red}}}&::before{color:blue}}"#,
            ),
            (
                ".a{&::before{@media (hover:hover){:is(.g &){color:red}}}}",
                ".a{&{@media (hover:hover){:is(.g &){&::before{color:red}}}}}",
            ),
            (
                ".a::marker, .a ::marker{:is(.g &){color:red}}",
                ".a{:is(.g &){&::marker{color:red}}}.a *{:is(.g &){&::marker{color:red}}}",
            ),
        ] {
            assert_eq!(
                hoist_pseudo_element_conditions(css).as_deref(),
                Some(expected),
                "{css}"
            );
        }
        for css in [
            ".a{&::before{&:is(.g *){color:red}}}",
            ".a{&::before{@media (hover:hover){color:blue;:is(.g &){color:red}}}}",
            ".a{&:hover{:is(.g &){color:red}}}",
        ] {
            assert_eq!(hoist_pseudo_element_conditions(css), None, "{css}");
        }
    }
}
