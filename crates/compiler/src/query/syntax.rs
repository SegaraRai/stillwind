//! Delimiter scanning shared by candidate completion and canonical spelling queries.

/// Finds a delimiter outside arbitrary values, selectors, functions, and escaped text.
pub(super) fn last_top_level(value: &str, delimiter: char) -> Option<usize> {
    let mut depth = 0_usize;
    let mut escaped = false;
    let mut result = None;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => {
                escaped = true;
            }
            '[' | '(' => {
                depth += 1;
            }
            ']' | ')' => {
                depth = depth.saturating_sub(1);
            }
            character if character == delimiter && depth == 0 => {
                result = Some(index);
            }
            _ => {}
        }
    }
    result
}
