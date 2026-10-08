//! Byte-level scanning of authored CSS at-rules.
//!
//! These helpers locate `@name <prelude> { ... }` blocks in CSS text without building a parse
//! tree, skipping braces that appear inside strings, comments, or parenthesized functions. Every
//! returned index is a byte offset into the input text. A name only matches at a token boundary,
//! so `@apply-extra` is not mistaken for `@apply`, and a block whose braces never close reports
//! `None` rather than a truncated range.

/// Locates the first `@<name>` block that is followed by a prelude and a braced body.
///
/// Returns `(start, prelude, open, close)`: the offset of the `@`, the trimmed text between the
/// name and the opening brace, and the offsets of the braces themselves. A statement form without
/// a body, such as `@<name> x;`, does not match.
pub(in crate::compiler) fn find_at_rule_block(
    css: &str,
    name: &str,
) -> Option<(usize, String, usize, usize)> {
    let marker = format!("@{name}");
    let bytes = css.as_bytes();
    let mut cursor = 0;
    let mut quote = None;
    let mut escaped = false;
    let mut comment = false;
    while cursor < bytes.len() {
        let character = bytes[cursor] as char;
        if comment {
            if bytes[cursor..].starts_with(b"*/") {
                comment = false;
                cursor += 2;
            } else {
                cursor += 1;
            }
            continue;
        }
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active_quote {
                quote = None;
            }
            cursor += 1;
            continue;
        }
        match character {
            '/' if bytes[cursor..].starts_with(b"/*") => {
                comment = true;
                cursor += 2;
            }
            '\'' | '"' => {
                quote = Some(character);
                cursor += 1;
            }
            '@' if css[cursor..].starts_with(&marker) => {
                let after_name = cursor + marker.len();
                if bytes.get(after_name).is_some_and(u8::is_ascii_whitespace) {
                    let open = find_at_rule_block_open(css, after_name)?;
                    let close = matching_brace(css, open)?;
                    return Some((cursor, css[after_name..open].trim().to_owned(), open, close));
                }
                cursor = after_name;
            }
            _ => cursor += 1,
        }
    }
    None
}

/// Offset of the `{` that opens the block whose name ends at `from`.
///
/// A `;` before the brace means the at-rule is a statement, so the search reports `None` instead
/// of matching a later block.
///
/// # Panics
///
/// Panics when `from` is past the end of `css` or does not lie on a UTF-8 character boundary,
/// because the scan begins by slicing at `from`. Callers must pass the offset just past an
/// `@<name>` match in this same string, as [`find_at_rule_block`] does; an offset measured
/// against a rewritten or different buffer panics.
fn find_at_rule_block_open(css: &str, from: usize) -> Option<usize> {
    let mut stack = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    let mut comment = false;
    let characters = css[from..].char_indices().collect::<Vec<_>>();
    for (position, (offset, character)) in characters.iter().copied().enumerate() {
        if comment {
            if character == '*'
                && characters
                    .get(position + 1)
                    .is_some_and(|(_, next)| *next == '/')
            {
                comment = false;
            }
            continue;
        }
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active_quote {
                quote = None;
            }
            continue;
        }
        match character {
            '/' if characters
                .get(position + 1)
                .is_some_and(|(_, next)| *next == '*') =>
            {
                comment = true;
            }
            '\'' | '"' => quote = Some(character),
            '(' | '[' => stack.push(character),
            ')' | ']' => {
                stack.pop()?;
            }
            '{' if stack.is_empty() => return Some(from + offset),
            ';' if stack.is_empty() => return None,
            _ => {}
        }
    }
    None
}

/// Offset of the `}` matching the `{` at `open`, ignoring braces in strings and comments.
///
/// `open` must itself be a `{`; an unbalanced body returns `None`.
///
/// # Panics
///
/// Panics when `open` is past the end of `value` or does not lie on a UTF-8 character boundary,
/// because the scan begins by slicing at `open`. Callers must pass the offset of a `{` in this
/// same string, as [`find_at_rule_block_open`] returns; an offset that addresses another buffer
/// panics.
pub(in crate::compiler) fn matching_brace(value: &str, open: usize) -> Option<usize> {
    let mut depth = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    let mut comment = false;
    let characters = value[open..].char_indices().collect::<Vec<_>>();
    for (position, (offset, character)) in characters.iter().copied().enumerate() {
        if comment {
            if character == '*'
                && characters
                    .get(position + 1)
                    .is_some_and(|(_, next)| *next == '/')
            {
                comment = false;
            }
            continue;
        }
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active_quote {
                quote = None;
            }
            continue;
        }
        match character {
            '/' if characters
                .get(position + 1)
                .is_some_and(|(_, next)| *next == '*') =>
            {
                comment = true;
            }
            '\'' | '"' => quote = Some(character),
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_scanning_skips_strings_comments_and_prefix_collisions() {
        let css = "/* @apply x {} */ .x{content:'@apply x {}'} @apply-extra x {} @apply a {b{c}}";
        let (start, name, open, close) = find_at_rule_block(css, "apply").unwrap();
        assert_eq!(start, css.rfind("@apply a").unwrap());
        assert_eq!(name, "a");
        assert_eq!(&css[open..=close], "{b{c}}");
        assert_eq!(find_at_rule_block("@apply a {", "apply"), None);
        assert_eq!(matching_brace("{ '}' /* } */ }", 0), Some(14));
        assert_eq!(find_at_rule_block_open("'{' {", 0), Some(4));
    }
}
