//! Construction of compact owned text without an intermediate `String`.

use lean_string::{LeanString, ToLeanString};

/// Concatenates borrowed text into a compact owned string.
pub trait JoinText {
    /// Returns the items in order, separated by `separator`.
    /// An empty slice produces an empty string.
    ///
    /// # Panics
    ///
    /// Panics if the combined byte length exceeds the string capacity limit or
    /// memory allocation fails.
    fn join_text(&self, separator: &str) -> LeanString;
}

impl<T: AsRef<str>> JoinText for [T] {
    fn join_text(&self, separator: &str) -> LeanString {
        let capacity = self.iter().map(|value| value.as_ref().len()).sum::<usize>()
            + self.len().saturating_sub(1) * separator.len();
        let mut output = LeanString::with_capacity(capacity);
        for (index, value) in self.iter().enumerate() {
            if index > 0 {
                output.push_str(separator);
            }
            output.push_str(value.as_ref());
        }
        output
    }
}

/// Replaces text while constructing a compact owned result.
pub trait ReplaceText {
    /// Replaces every nonoverlapping occurrence of `from` with `replacement`.
    /// An empty `from` matches each UTF-8 character boundary, including both ends.
    ///
    /// # Panics
    ///
    /// Panics if the result exceeds the string capacity limit or memory allocation fails.
    fn replace_text(&self, from: &str, replacement: &str) -> LeanString;

    /// Replaces every occurrence of `from` with `replacement`.
    ///
    /// # Panics
    ///
    /// Panics if the result exceeds the string capacity limit or memory allocation fails.
    fn replace_char(&self, from: char, replacement: &str) -> LeanString;
}

impl ReplaceText for str {
    fn replace_text(&self, from: &str, replacement: &str) -> LeanString {
        if !self.contains(from) {
            return self.to_lean_string();
        }
        replace_matches(self, self.match_indices(from), replacement)
    }

    fn replace_char(&self, from: char, replacement: &str) -> LeanString {
        if !self.contains(from) {
            return self.to_lean_string();
        }
        replace_matches(self, self.match_indices(from), replacement)
    }
}

fn replace_matches<'a>(
    text: &'a str,
    matches: impl Iterator<Item = (usize, &'a str)>,
    replacement: &str,
) -> LeanString {
    let mut output = LeanString::with_capacity(text.len());
    let mut last = 0;
    for (index, matched) in matches {
        output.push_str(&text[last..index]);
        output.push_str(replacement);
        last = index + matched.len();
    }
    output.push_str(&text[last..]);
    output
}

#[cfg(test)]
mod tests {
    use super::{JoinText, ReplaceText};

    #[test]
    fn joins_empty_and_unicode_items_in_order() {
        assert_eq!(["α", "", "β"].join_text("/"), "α//β");
        assert_eq!(["α", "β"].join_text(""), "αβ");
        assert!([""; 0].join_text("/").is_empty());
    }

    proptest::proptest! {
        #[test]
        fn replacements_preserve_standard_string_semantics(
            text in ".{0,48}",
            pattern in ".{0,4}",
            character in proptest::char::any(),
            replacement in ".{0,8}",
        ) {
            let replaced = text.replace_text(&pattern, &replacement);
            proptest::prop_assert_eq!(replaced.as_str(), text.replace(&pattern, &replacement));
            let replaced = text.replace_char(character, &replacement);
            proptest::prop_assert_eq!(replaced.as_str(), text.replace(character, &replacement));
        }
    }
}
