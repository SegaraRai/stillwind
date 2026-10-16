//! Lexical discovery of `--name` occurrences in CSS text.
//!
//! The scan is purely textual: it does not parse declarations or functions, so
//! a `--name` inside a comment, a string or a declaration is reported the same
//! way. Callers that need statement context strip comments or blocks first.
//! This is the cheap dependency set used for activation decisions, not a
//! substitute for the AST-based classification in [`super::usage`].

use lean_string::{LeanStr, ToLeanStr};

use std::collections::BTreeSet;

/// Returns every distinct `--name` occurrence in `css`, in `BTreeSet` order.
///
/// Names continue with ASCII letters, digits, `-`, `_`, and CSS escapes, so bare `--`
/// and `--` followed by anything else are skipped. An escape keeps its authored
/// spelling, such as `--size-2\.5`. The scan never fails and never decodes escapes.
pub fn custom_property_references(css: &str) -> BTreeSet<LeanStr> {
    let mut references = BTreeSet::new();
    let mut cursor = 0;
    while let Some(relative) = css[cursor..].find("--") {
        let start = cursor + relative;
        let end = name_end(css, start + 2);
        if end > start + 2 {
            let name = &css[start..end];
            // Check borrowed text first to avoid allocating a LeanStr for duplicate long names.
            if !references.contains(name) {
                references.insert(name.to_lean_str());
            }
        }
        cursor = end.max(start + 2);
    }
    references
}

/// Returns the end of the name characters and CSS escapes that start at `start`.
///
/// A hexadecimal escape such as `\31 ` includes up to six digits and one following
/// whitespace character. Another escape includes the one character after `\`, except a
/// newline.
fn name_end(css: &str, start: usize) -> usize {
    let bytes = css.as_bytes();
    let mut end = start;
    while let Some(&byte) = bytes.get(end) {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_') {
            end += 1;
            continue;
        }
        if byte != b'\\' {
            break;
        }
        let Some(escaped) = css[end + 1..].chars().next() else {
            break;
        };
        if matches!(escaped, '\n' | '\r' | '\u{c}') {
            break;
        }
        end += 1;
        if escaped.is_ascii_hexdigit() {
            let digits = bytes[end..]
                .iter()
                .take(6)
                .take_while(|byte| byte.is_ascii_hexdigit())
                .count();
            end += digits;
            if bytes.get(end).is_some_and(u8::is_ascii_whitespace) {
                end += 1;
            }
        } else {
            end += escaped.len_utf8();
        }
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicates_names_and_stops_at_css_delimiters() {
        assert_eq!(
            custom_property_references("var(--x, --fallback_2) --x --"),
            BTreeSet::from(["--x".into(), "--fallback_2".into()])
        );
        assert!(custom_property_references("color:red").is_empty());
        assert_eq!(
            custom_property_references(r"var(--size-2\.5) var(--a\31 b) --end\"),
            BTreeSet::from([r"--size-2\.5".into(), r"--a\31 b".into(), "--end".into()])
        );
    }
}
