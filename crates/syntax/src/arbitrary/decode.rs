//! Converts the spelling of an arbitrary value to CSS: underscores become spaces, and
//! operators in math functions receive spaces.

use lean_string::LeanString;

use super::value::{Node, parse, write_nodes};

/// Converts the arbitrary value `input` from its spelling in a candidate to CSS, as
/// Tailwind does.
///
/// An `_` becomes a space, and `\_` becomes `_`. Other backslash escapes stay as
/// written. Underscores stay in the arguments of `url()` and in the first argument of
/// `var()` and `theme()`, so `var(--my_width)` keeps its name. Operators and commas
/// inside math functions such as `calc()` receive spaces, so `calc(1px+2px)` becomes
/// `calc(1px + 2px)`.
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

/// Returns `input` with each `_` replaced by a space and each `\_` replaced by `_`.
/// When `keep_underscores` is true, only `\_` changes.
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

/// Replaces underscores with spaces in every node of `nodes`. Underscores stay in the
/// arguments of `url()` and in the first argument of `var()` and `theme()`.
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
                            _ => {
                                decode_nodes(std::slice::from_mut(argument));
                            }
                        }
                    }
                } else {
                    decode_nodes(arguments);
                }
            }
        }
    }
}

/// The CSS math functions whose arguments receive spaces around operators.
const MATH_FUNCTIONS: [&str; 19] = [
    "calc", "min", "max", "clamp", "mod", "rem", "sin", "cos", "tan", "asin", "acos", "atan",
    "atan2", "pow", "sqrt", "hypot", "log", "exp", "round",
];

/// Returns `input` with spaces around the operators of math functions and a space after
/// their commas, as Tailwind's `addWhitespaceAroundMathOperators()` does.
///
/// An operator gets no spaces when it starts an argument, when it follows another
/// operator, or when it is the sign of an exponent such as in `1e-3`. Otherwise it gets
/// spaces when a number, a value with a unit, a parenthesis, or another operator is
/// next to it. A `-` between two identifiers gets no spaces, because it may be part of
/// a name.
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
            ',' if inside_math => {
                result.push_str(", ");
            }
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
            _ => {
                result.push(character);
            }
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
