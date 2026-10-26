//! Splits a CSS value into words, separators, and functions, as Tailwind's value parser
//! does.

/// One part of a CSS value.
pub(super) enum Node {
    /// Text between separators, including quoted strings and escapes as written.
    Word(String),
    /// A run of `:`, `,`, `=`, `<`, `>`, newlines, spaces, or tabs.
    Separator(String),
    /// A function name and its arguments. The name is empty for parentheses without a
    /// name.
    Function(String, Vec<Node>),
}

/// Splits the value `input` into words, separators, and functions, as Tailwind's value
/// parser does. When a function is not closed, the text after it stays in the outer
/// list.
pub(super) fn parse(input: &str) -> Vec<Node> {
    let input = input.replace("\r\n", "\n");
    let characters = input.chars().collect::<Vec<_>>();
    let mut ast = Vec::new();
    // Open functions, innermost last.
    let mut stack: Vec<(String, Vec<Node>)> = Vec::new();
    let mut buffer = String::new();
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        match character {
            '\\' => {
                buffer.push(character);
                if let Some(&next) = characters.get(index + 1) {
                    buffer.push(next);
                }
                index += 1;
            }
            '/' => {
                if !buffer.is_empty() {
                    push(
                        &mut stack,
                        &mut ast,
                        Node::Word(std::mem::take(&mut buffer)),
                    );
                }
                push(&mut stack, &mut ast, Node::Word("/".into()));
            }
            _ if separator(character) => {
                if !buffer.is_empty() {
                    push(
                        &mut stack,
                        &mut ast,
                        Node::Word(std::mem::take(&mut buffer)),
                    );
                }
                let end = characters[index..]
                    .iter()
                    .position(|&next| !separator(next))
                    .map_or(characters.len(), |offset| index + offset);
                push(
                    &mut stack,
                    &mut ast,
                    Node::Separator(characters[index..end].iter().collect()),
                );
                index = end - 1;
            }
            '\'' | '"' => {
                let start = index;
                let mut next = index + 1;
                while next < characters.len() {
                    if characters[next] == '\\' {
                        next += 1;
                    } else if characters[next] == character {
                        index = next;
                        break;
                    }
                    next += 1;
                }
                buffer.extend(&characters[start..=index]);
            }
            '(' => {
                stack.push((std::mem::take(&mut buffer), Vec::new()));
            }
            ')' => {
                if let Some((name, mut nodes)) = stack.pop() {
                    if !buffer.is_empty() {
                        nodes.push(Node::Word(std::mem::take(&mut buffer)));
                    }
                    push(&mut stack, &mut ast, Node::Function(name, nodes));
                } else {
                    buffer.clear();
                }
            }
            _ => {
                buffer.push(character);
            }
        }
        index += 1;
    }
    // Tailwind keeps an unclosed function in the tree when it opens, so its nodes
    // remain in order.
    while let Some((name, nodes)) = stack.pop() {
        push(&mut stack, &mut ast, Node::Function(name, nodes));
    }
    if !buffer.is_empty() {
        ast.push(Node::Word(buffer));
    }
    ast
}

/// Checks whether `character` separates words: `:`, `,`, `=`, `<`, `>`, a newline, a
/// space, or a tab.
fn separator(character: char) -> bool {
    matches!(character, ':' | ',' | '=' | '>' | '<' | '\n' | ' ' | '\t')
}

/// Appends `node` to the innermost open function in `stack`, or to `ast` when no
/// function is open.
fn push(stack: &mut [(String, Vec<Node>)], ast: &mut Vec<Node>, node: Node) {
    match stack.last_mut() {
        Some((_, nodes)) => {
            nodes.push(node);
        }
        None => {
            ast.push(node);
        }
    }
}

/// Appends the CSS text of `nodes` to `css`.
pub(super) fn write_nodes(nodes: &[Node], css: &mut String) {
    for node in nodes {
        match node {
            Node::Word(value) | Node::Separator(value) => {
                css.push_str(value);
            }
            Node::Function(name, arguments) => {
                css.push_str(name);
                css.push('(');
                write_nodes(arguments, css);
                css.push(')');
            }
        }
    }
}
