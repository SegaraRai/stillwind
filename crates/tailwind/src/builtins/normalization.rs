use super::metadata::builtin_function_formats;

/// Removes non-semantic whitespace from vendored declarations.
///
/// This applies only to Stillwind-maintained CSS; user `@utility` bodies keep
/// their authored serialization. Quoted values are preserved byte-for-byte.
#[allow(clippy::too_many_lines)]
pub(crate) fn canonical_builtin_body(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut quote = None;
    let mut escaped = false;
    let mut whitespace = false;
    let mut functions = Vec::<String>::new();
    let mut custom_property_declaration = false;

    for character in value.chars() {
        if let Some(current_quote) = quote {
            output.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == current_quote {
                quote = None;
            }
            continue;
        }

        match character {
            '\'' | '"' => {
                if whitespace && output.chars().last().is_some_and(is_word_character) {
                    output.push(' ');
                }
                whitespace = false;
                quote = Some(character);
                output.push(character);
            }
            ';' => {
                whitespace = false;
                custom_property_declaration = false;
                functions.clear();
                output.push(character);
            }
            '(' => {
                let name = output
                    .chars()
                    .rev()
                    .take_while(|character| {
                        character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
                    })
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>();
                functions.push(name);
                output.push(character);
            }
            ')' => {
                functions.pop();
                output.push(character);
            }
            ',' => {
                output.push(character);
                let preserve_function_spacing = functions
                    .iter()
                    .any(|name| builtin_function_formats().contains(name));
                if custom_property_declaration || preserve_function_spacing {
                    output.push(' ');
                }
                whitespace = false;
            }
            ':' if output
                .rsplit_once(';')
                .map_or(output.as_str(), |(_, declaration)| declaration)
                .trim_start()
                .starts_with("--") =>
            {
                custom_property_declaration = true;
                output.push(character);
            }
            character if character.is_whitespace() => whitespace = true,
            character => {
                if whitespace && let Some(previous) = output.chars().last() {
                    let adjacent_words = is_word_character(previous)
                        && (is_word_character(character) || character == '#');
                    let function_followed_by_word = previous == ')' && is_word_character(character);
                    // Keep a separator after commas. Tailwind's vendored
                    // utility serializer retains this spelling in function
                    // arguments (notably color-mix/gradient stop lists), and
                    // LightningCSS does not normalize it away in its output.
                    let preserve_function_spacing = functions
                        .iter()
                        .any(|name| builtin_function_formats().contains(name));
                    if adjacent_words
                        || function_followed_by_word
                        || (previous == ','
                            && (custom_property_declaration || preserve_function_spacing))
                    {
                        output.push(' ');
                    }
                }
                whitespace = false;
                output.push(character);
            }
        }
    }

    // Fallback lists retain a separator before `var()` regardless of the
    // surrounding property; this is a value-level serialization rule.
    output.replace(",var(", ", var(")
}

pub(crate) fn is_word_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '%')
}
