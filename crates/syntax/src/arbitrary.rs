//! Tailwind's decoding of arbitrary values, such as the `calc(100%_-_1rem)` of
//! `w-[calc(100%_-_1rem)]`.
//!
//! The rules follow Tailwind CSS 4.3.3 (`utils/decode-arbitrary-value.ts`,
//! `value-parser.ts`, and `utils/math-operators.ts`). Keep them aligned with upstream:
//! candidates written for Tailwind must decode to the same CSS.

use lean_string::LeanString;

/// One node of Tailwind's value tokenization.
enum Node {
    /// Text between separators, including quoted strings and escapes as written.
    Word(String),
    /// A run of `:`, `,`, `=`, `<`, `>`, newline, space, or tab.
    Separator(String),
    /// A function name, which may be empty for plain parentheses, and its arguments.
    Function(String, Vec<Node>),
}

/// Decodes an arbitrary value as Tailwind does.
///
/// An unescaped `_` becomes a space and `\_` an underscore. Every other backslash
/// escape is kept as written. Underscores stay in the arguments of `url()`, and in the
/// first argument of `var()` and `theme()`, so `var(--my_width)` keeps its name. Binary
/// operators and commas inside math functions such as `calc()` receive spaces:
/// `calc(1px+2px)` becomes `calc(1px + 2px)`.
#[must_use]
pub fn decode_arbitrary_value(input: &str) -> LeanString {
    if !input.contains('(') {
        return underscores_to_spaces(input, false).into();
    }
    let mut ast = parse(input);
    decode_nodes(&mut ast);
    let mut css = String::with_capacity(input.len());
    write_nodes(&ast, &mut css);
    add_whitespace_around_math_operators(&css).into()
}

/// Converts `_` to a space and `\_` to `_`. With `keep_underscores`, only `\_` changes.
fn underscores_to_spaces(input: &str, keep_underscores: bool) -> String {
    let mut output = String::with_capacity(input.len());
    let mut characters = input.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\\' && characters.peek() == Some(&'_') {
            characters.next();
            output.push('_');
        } else if character == '_' && !keep_underscores {
            output.push(' ');
        } else {
            output.push(character);
        }
    }
    output
}

/// Tokenizes a value into words, separators, and functions, as Tailwind's value parser
/// does. Text after an unclosed function stays in the outermost list.
fn parse(input: &str) -> Vec<Node> {
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
            '(' => stack.push((std::mem::take(&mut buffer), Vec::new())),
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
            _ => buffer.push(character),
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

/// Whether a character separates words in Tailwind's value parser.
fn separator(character: char) -> bool {
    matches!(character, ':' | ',' | '=' | '>' | '<' | '\n' | ' ' | '\t')
}

/// Appends a node to the innermost open function, or to the outermost list.
fn push(stack: &mut [(String, Vec<Node>)], ast: &mut Vec<Node>, node: Node) {
    match stack.last_mut() {
        Some((_, nodes)) => nodes.push(node),
        None => ast.push(node),
    }
}

/// Decodes underscores in every node, keeping them where Tailwind keeps them.
fn decode_nodes(nodes: &mut [Node]) {
    for node in nodes {
        match node {
            Node::Word(value) | Node::Separator(value) => {
                *value = underscores_to_spaces(value, false);
            }
            Node::Function(name, arguments) => {
                let url = name == "url" || name.ends_with("_url");
                let reference = ["var", "theme"]
                    .iter()
                    .any(|function| name == function || name.ends_with(&format!("_{function}")));
                *name = underscores_to_spaces(name, false);
                if url {
                    continue;
                }
                if reference {
                    for (index, argument) in arguments.iter_mut().enumerate() {
                        match argument {
                            Node::Word(value) if index == 0 => {
                                *value = underscores_to_spaces(value, true);
                            }
                            _ => decode_nodes(std::slice::from_mut(argument)),
                        }
                    }
                } else {
                    decode_nodes(arguments);
                }
            }
        }
    }
}

/// Serializes nodes back to CSS text.
fn write_nodes(nodes: &[Node], css: &mut String) {
    for node in nodes {
        match node {
            Node::Word(value) | Node::Separator(value) => css.push_str(value),
            Node::Function(name, arguments) => {
                css.push_str(name);
                css.push('(');
                write_nodes(arguments, css);
                css.push(')');
            }
        }
    }
}

/// CSS math functions whose arguments Tailwind formats.
const MATH_FUNCTIONS: [&str; 19] = [
    "calc", "min", "max", "clamp", "mod", "rem", "sin", "cos", "tan", "asin", "acos", "atan",
    "atan2", "pow", "sqrt", "hypot", "log", "exp", "round",
];

/// Adds whitespace around the operators and after the commas of math functions, as
/// Tailwind's `addWhitespaceAroundMathOperators()` does.
///
/// An operator keeps its spelling when it starts an argument, follows another operator,
/// or is the exponent sign of a number such as `1e-3`. Otherwise it receives spaces
/// when a number, a closing parenthesis, a value with a unit, an opening parenthesis,
/// or another operator is next to it. A hyphen between two identifiers stays, because
/// it may belong to a name.
fn add_whitespace_around_math_operators(input: &str) -> String {
    if !MATH_FUNCTIONS
        .iter()
        .any(|function| input.contains(function))
    {
        return input.into();
    }
    let characters = input.chars().collect::<Vec<_>>();
    let mut result = String::with_capacity(input.len() + 8);
    // One entry per open parenthesis: whether operators directly inside it are math.
    let mut formattable: Vec<bool> = Vec::new();
    let mut value_position: Option<usize> = None;
    let mut last_value_position: Option<usize> = None;
    let operator = |character: char| matches!(character, '+' | '*' | '/' | '-');
    for (index, &character) in characters.iter().enumerate() {
        // A digit starts a value, and a unit or `%` after a digit continues it.
        if character.is_ascii_digit()
            || (value_position.is_some() && (character == '%' || character.is_ascii_alphabetic()))
        {
            value_position = Some(index);
        } else {
            last_value_position = value_position;
            value_position = None;
        }
        let inside_math = formattable.last() == Some(&true);
        match character {
            '(' => {
                result.push(character);
                let name_start = characters[..index]
                    .iter()
                    .rposition(|&previous| {
                        !(previous.is_ascii_digit() || previous.is_ascii_lowercase())
                    })
                    .map_or(0, |position| position + 1);
                let name = characters[name_start..index].iter().collect::<String>();
                formattable.push(
                    MATH_FUNCTIONS.contains(&name.as_str()) || (inside_math && name.is_empty()),
                );
            }
            ')' => {
                result.push(character);
                formattable.pop();
            }
            ',' if inside_math => result.push_str(", "),
            ' ' if inside_math && result.ends_with(' ') => {}
            _ if operator(character) && inside_math => {
                let trimmed = result.trim_end();
                let mut before = trimmed.chars().rev();
                let previous = before.next();
                let before_previous = before.next();
                let next = characters.get(index + 1).copied();
                if matches!(previous, Some('e' | 'E'))
                    && before_previous.is_some_and(|value| value.is_ascii_digit())
                    || previous.is_some_and(operator)
                    || matches!(previous, Some('(' | ','))
                {
                    result.push(character);
                } else if index > 0 && characters[index - 1] == ' ' {
                    result.push(character);
                    result.push(' ');
                } else if previous.is_some_and(|value| value.is_ascii_digit())
                    || next.is_some_and(|value| value.is_ascii_digit())
                    || previous == Some(')')
                    || next == Some('(')
                    || next.is_some_and(operator)
                    || last_value_position.is_some_and(|position| position + 1 == index)
                {
                    result.push(' ');
                    result.push(character);
                    result.push(' ');
                } else {
                    result.push(character);
                }
            }
            _ => result.push(character),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_as_tailwind_does() {
        for (authored, decoded) in [
            (r"one_two\_three", "one two_three"),
            (r"a\:b", r"a\:b"),
            ("var(--my_width)", "var(--my_width)"),
            ("var(--my_width,_1px)", "var(--my_width, 1px)"),
            ("theme(--spacing_x)", "theme(--spacing_x)"),
            ("url(/a_b.png)", "url(/a_b.png)"),
            ("'hello_world'", "'hello world'"),
            ("calc(1px+2px)", "calc(1px + 2px)"),
            ("calc(100%-1rem)", "calc(100% - 1rem)"),
            ("calc(100%_-_1rem)", "calc(100% - 1rem)"),
            ("calc(1+2)", "calc(1 + 2)"),
            ("calc(100%-2*var(--x))", "calc(100% - 2 * var(--x))"),
            (
                "min(10px,calc(1rem-(2px+3px)))",
                "min(10px, calc(1rem - (2px + 3px)))",
            ),
            ("calc(var(--a-b)*-1)", "calc(var(--a-b) * -1)"),
            ("calc(1e-3*1px)", "calc(1e-3 * 1px)"),
            ("calc(-1px+2px)", "calc(-1px + 2px)"),
            ("translate(1px-2px)", "translate(1px-2px)"),
            ("nth-child(2n+1)", "nth-child(2n+1)"),
        ] {
            assert_eq!(decode_arbitrary_value(authored), decoded, "{authored}");
        }
    }

    proptest::proptest! {
        #[test]
        fn ordinary_identifiers_survive_decoding(name in "[a-zA-Z0-9-]{0,40}") {
            proptest::prop_assert_eq!(decode_arbitrary_value(&name), name);
        }
    }
}
