//! Brace and parenthesis matching used by the whole-crate evaluator.
//!
//! The scanners are deliberately character-based rather than CSS-aware: they run over
//! registry-authored fragments whose syntax is already known to be balanced, and they
//! must be cheap enough to call per evaluated declaration. A group that never closes
//! returns [`None`], which callers treat as "this branch does not match" instead of
//! guessing an end.

/// Returns the byte index of the brace closing the `{` at `open`.
///
/// `None` means the group is unclosed or contains a closer without an opener. `open` is
/// assumed to be at an opening brace.
pub(super) fn closing_brace(value: &str, open: usize) -> Option<usize> {
    let mut depth = 0_u32;
    for (offset, character) in value[open..].char_indices() {
        match character {
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

/// Returns the byte index of the parenthesis closing the `(` at `open`.
///
/// `None` means the group is unclosed or contains a closer without an opener. `open` is
/// assumed to be at an opening parenthesis.
pub(super) fn closing_parenthesis(value: &str, open: usize) -> Option<usize> {
    let mut depth = 0_u32;
    for (offset, character) in value[open..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
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
    fn locates_nested_closers_and_rejects_unclosed_groups() {
        assert_eq!(closing_brace("a{b{c}d}z", 1), Some(7));
        assert_eq!(closing_parenthesis("f(a(b)c)z", 1), Some(7));
        assert_eq!(closing_brace("{a", 0), None);
        assert_eq!(closing_parenthesis("(a", 0), None);
    }

    proptest::proptest! {
        #[test]
        fn balanced_parentheses_close_at_the_outer_end(depth in 1usize..40) {
            let input = format!("{}x{}tail", "(".repeat(depth), ")".repeat(depth));
            proptest::prop_assert_eq!(closing_parenthesis(&input, 0), Some(depth * 2));
        }
    }
}
