//! Canonicalization of vendored built-in declaration bodies.
//!
//! Vendored definitions are re-serialized so a built-in body is byte-identical after
//! removing selected whitespace. Preserved spacing and the raw registry source can
//! still affect serialization and snapshot fingerprints. The rules are conservative:
//! quoted content is preserved byte-for-byte, spacing that Tailwind's own serializer
//! keeps after commas in selected functions and custom-property declarations is
//! retained, and a separator before a `var()` fallback list is always kept.
//!
//! Only built-in CSS is canonicalized. User `@utility` bodies keep their authored
//! serialization, because rewriting them would change the CSS a project wrote on
//! purpose.

use std::collections::BTreeSet;

/// Removes non-semantic whitespace from vendored declarations.
///
/// This applies only to Stillwind-maintained CSS; user `@utility` bodies keep their
/// authored serialization. Quoted values are preserved byte-for-byte.
///
/// `function_formats` names the functions whose argument separators must keep their
/// authored spacing; the caller supplies it because the spacing rule is declared in the
/// same registry CSS. The result depends only on `value` and `function_formats`, so
/// repeated parsing yields identical bodies.
#[allow(clippy::too_many_lines)]
pub fn canonical_builtin_body(value: &str, function_formats: &BTreeSet<String>) -> String {
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
                let preserve_function_spacing =
                    functions.iter().any(|name| function_formats.contains(name));
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
                    // Keep a separator after commas. Tailwind's vendored utility
                    // serializer retains this spelling in function arguments (notably
                    // color-mix/gradient stop lists), and LightningCSS does not
                    // normalize it away in its output.
                    let preserve_function_spacing =
                        functions.iter().any(|name| function_formats.contains(name));
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

    // Fallback lists retain a separator before `var()` regardless of the surrounding
    // property; this is a value-level serialization rule.
    output.replace(",var(", ", var(")
}

/// Whether two adjacent characters may not be separated by a dropped space.
///
/// `%` counts as a word character so `10 %` does not collapse into `10%`.
pub(crate) fn is_word_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '%')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalization_keeps_quoted_content_and_token_boundaries() {
        assert_eq!(
            canonical_builtin_body("color: red; width: 1px;", &BTreeSet::new()),
            "color:red;width:1px;"
        );
        assert_eq!(
            canonical_builtin_body("content: 'a  b'; font: bold 12px serif;", &BTreeSet::new()),
            "content:'a  b';font:bold 12px serif;"
        );
        assert!(is_word_character('%'));
        assert!(!is_word_character(';'));
    }
}
