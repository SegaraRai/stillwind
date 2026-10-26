//! Converts CSS values back to the short spelling of an arbitrary value in a candidate.

use lean_string::LeanString;

use super::value::{Node, parse, write_nodes};

/// Converts the CSS value `value` to its short spelling as an arbitrary value, such as
/// `calc(100%-1rem)` for `calc(100% - 1rem)`.
///
/// Spaces become `_`, and spaces that CSS does not need are removed.
/// `decode_arbitrary_value` adds back the spaces that math functions need. The contents
/// of strings and `url()` keep their value, and variable names keep their underscores.
#[must_use]
pub fn print_arbitrary_value(value: &str) -> LeanString {
    let mut nodes = parse(value);
    compact(&mut nodes);
    encode(&mut nodes, false);
    let mut output = String::with_capacity(value.len());
    write_nodes(&nodes, &mut output);
    output.into()
}

/// Checks whether `node` is one of the combinators or operators `~`, `>`, `+`, `-`,
/// `*`, and `/`.
fn symbol(node: &Node) -> bool {
    matches!(node, Node::Word(word) if matches!(word.as_str(), "~" | ">" | "+" | "-" | "*" | "/"))
}

/// Checks whether `node` is a separator that contains exactly one space.
fn space(node: &Node) -> bool {
    matches!(node, Node::Separator(value) if value == " ")
}

/// Removes the spaces that CSS does not need from `nodes`, inside every function except
/// `url()`.
///
/// The method removes the spaces around an operator between two operands, whitespace at
/// the start and the end, and the spaces around commas. When it removes the space
/// before a `--name()` call, it wraps the call in parentheses, so that the operator
/// before it does not become part of the name.
fn compact(nodes: &mut Vec<Node>) {
    let mut dropped = vec![false; nodes.len()];
    for (index, node) in nodes.iter().enumerate() {
        if symbol(node)
            && index >= 2
            && index + 2 < nodes.len()
            && space(&nodes[index - 1])
            && space(&nodes[index + 1])
            && !symbol(&nodes[index - 2])
            && !symbol(&nodes[index + 2])
        {
            dropped[index - 1] = true;
            dropped[index + 1] = true;
        }
        if matches!(node, Node::Separator(value) if !value.is_empty() && value.trim().is_empty())
            && (index == 0 || index + 1 == nodes.len())
        {
            dropped[index] = true;
        }
    }
    for (index, node) in nodes.iter_mut().enumerate() {
        match node {
            Node::Separator(value) if value.trim() == "," => *value = ",".into(),
            Node::Function(name, arguments) if name != "url" && !name.ends_with("_url") => {
                compact(arguments);
                if name.starts_with("--") && index > 1 && dropped[index - 1] {
                    let function = std::mem::replace(node, Node::Word(String::new()));
                    *node = Node::Function(String::new(), vec![function]);
                }
            }
            _ => {}
        }
    }
    let mut index = 0;
    nodes.retain(|_| {
        let keep = !dropped[index];
        index += 1;
        keep
    });
}

/// Escapes `nodes` for brackets: a space becomes `_`, and `_` becomes `\_`. When
/// `preserve_name` is true, the first node keeps its underscores, because the first
/// argument of `var()` and `theme()` is a name. The arguments of `url()` stay as
/// written.
fn encode(nodes: &mut [Node], preserve_name: bool) {
    for (index, node) in nodes.iter_mut().enumerate() {
        match node {
            Node::Word(value) | Node::Separator(value) => {
                let keep = preserve_name && index == 0;
                *value = value.chars().fold(String::new(), |mut encoded, character| {
                    match character {
                        ' ' => encoded.push('_'),
                        '_' if !keep => encoded.push_str("\\_"),
                        _ => encoded.push(character),
                    }
                    encoded
                });
            }
            Node::Function(name, arguments) => {
                let url = name == "url" || name.ends_with("_url");
                let reference = matches!(name.as_str(), "var" | "theme");
                *name = name.replace('_', "\\_").replace(' ', "_");
                if !url {
                    encode(arguments, reference);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode_arbitrary_value;

    #[test]
    fn prints_optional_separators_and_keeps_literal_content() {
        for (raw, expected) in [
            ("calc(100%_-_1rem)", "calc(100%-1rem)"),
            ("rgb(10,_20,_30)", "rgb(10,20,30)"),
            ("min(50vw,_500px)", "min(50vw,500px)"),
            ("1fr_2fr", "1fr_2fr"),
            ("'hello_world'", "'hello_world'"),
            (r"'hello\_world'", r"'hello\_world'"),
            ("url('/a_b.png')", "url('/a_b.png')"),
            ("var(--my_width,_10px)", "var(--my_width,10px)"),
            ("calc(100%_-_--spacing(2))", "calc(100%-(--spacing(2)))"),
            ("&:has(~_*_*:checked)", "&:has(~_*_*:checked)"),
        ] {
            assert_eq!(
                print_arbitrary_value(&decode_arbitrary_value(raw)),
                expected,
                "{raw}"
            );
        }
    }
}
