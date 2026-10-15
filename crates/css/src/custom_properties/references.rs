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
/// Names continue only with ASCII letters, digits, `-` and `_`, so bare `--` and
/// `--` followed by anything else are skipped. The scan never fails and never
/// decodes escapes.
pub fn custom_property_references(css: &str) -> BTreeSet<LeanStr> {
    let mut references = BTreeSet::new();
    let mut cursor = 0;
    while let Some(relative) = css[cursor..].find("--") {
        let start = cursor + relative;
        let end = css[start..]
            .char_indices()
            .take_while(|(_, character)| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
            .last()
            .map_or(start + 2, |(offset, character)| {
                start + offset + character.len_utf8()
            });
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
    }
}
