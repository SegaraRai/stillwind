//! Input conversion for the vendored Tailwind parity fixtures.
//!
//! Call [`fixture_css`] before expanding package imports with [`crate::css_source`].
//! The conversion selects Stillwind's registry for upstream fixture imports.

use cssparser::{Parser, Token};

/// Returns `css` with top-level quoted Tailwind package imports mapped to Stillwind.
/// Supports `tailwindcss` and its `theme`, `preflight`, and `utilities` entry points.
/// Removes a lone `source(none)` tail from the full package import because fixture
/// candidates are supplied explicitly. Other import tails are preserved.
///
/// When a Tailwind import is converted, adds `stillwind/builtins.css` unless a full
/// Stillwind package import or explicit builtin import already selects it.
/// CSS without supported Tailwind imports is returned unchanged. Import text inside
/// comments, strings, or nested blocks is ignored. Unrelated imports and other CSS
/// are preserved.
pub fn fixture_css(css: &str) -> String {
    let mut parser = Parser::new(css);
    let mut output = String::new();
    let mut copied = 0;
    let mut converted = false;
    let mut has_registry = false;
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            break;
        };
        if !matches!(token, Token::AtKeyword(name) if name.eq_ignore_ascii_case("import")) {
            continue;
        }
        let Ok(specifier) = parser.expect_string().map(ToString::to_string) else {
            continue;
        };
        let tail_start = parser.position().byte_index();
        while let Ok(token) = parser.next_including_whitespace_and_comments() {
            if matches!(token, Token::Semicolon) {
                break;
            }
        }
        let end = parser.position().byte_index();
        let replacement = match specifier.as_str() {
            "tailwindcss" => Some("stillwind"),
            "tailwindcss/theme" => Some("stillwind/theme.css"),
            "tailwindcss/preflight" => Some("stillwind/preflight.css"),
            "tailwindcss/utilities" => Some("stillwind/utilities.css"),
            _ => None,
        };
        let selected = replacement.unwrap_or(&specifier);
        has_registry |= matches!(selected, "stillwind" | "stillwind/builtins.css");
        if let Some(replacement) = replacement {
            let tail = &css[tail_start..end];
            let tail = tail.strip_suffix(';').unwrap_or(tail);
            let tail = if specifier == "tailwindcss" && tail.trim() == "source(none)" {
                ""
            } else {
                tail
            };
            output.push_str(&css[copied..start]);
            output.push_str("@import \"");
            output.push_str(replacement);
            output.push('"');
            output.push_str(tail);
            output.push(';');
            copied = end;
            converted = true;
        }
    }
    output.push_str(&css[copied..]);
    if converted && !has_registry {
        output.push_str("\n@import \"stillwind/builtins.css\";");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_imports_select_the_registry_and_keep_layer_tails() {
        for quote in ['\'', '"'] {
            for (entry, target) in [
                ("theme", "theme.css"),
                ("preflight", "preflight.css"),
                ("utilities", "utilities.css"),
            ] {
                assert_eq!(
                    fixture_css(&format!(
                        "@import {quote}tailwindcss/{entry}{quote} layer(theme);"
                    )),
                    format!(
                        "@import \"stillwind/{target}\" layer(theme);\n@import \"stillwind/builtins.css\";"
                    )
                );
            }
            assert_eq!(
                fixture_css(&format!("@import {quote}tailwindcss{quote} source(none);")),
                "@import \"stillwind\";"
            );
            assert_eq!(
                fixture_css(&format!("@import {quote}tailwindcss{quote};")),
                "@import \"stillwind\";"
            );
        }
    }

    #[test]
    fn explicit_registry_selection_does_not_add_another_import() {
        for entry in ["stillwind", "stillwind/builtins.css"] {
            let css = format!("@import 'tailwindcss/utilities';\n@import '{entry}';");
            assert_eq!(
                fixture_css(&css),
                format!("@import \"stillwind/utilities.css\";\n@import '{entry}';")
            );
        }
    }

    #[test]
    fn ordinary_css_and_import_text_in_other_tokens_are_preserved() {
        for css in [
            "@theme { --color-brand: red; }",
            "@import 'stillwind' source(none);",
            "@import 'tailwindcss/unknown';",
            "/* @import 'tailwindcss'; */ .x { content: \"@import 'tailwindcss';\"; }",
            "@media print { @import 'tailwindcss'; }",
        ] {
            assert_eq!(fixture_css(css), css);
        }
    }
}
