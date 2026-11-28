//! ASCII case-insensitive string matching without allocating a lowercased copy.

/// ASCII case-insensitive matching helpers for string slices.
///
/// Only ASCII letters `A`-`Z` and `a`-`z` are folded; non-ASCII characters are compared
/// byte-for-byte, matching CSS ASCII case-insensitive comparison rules.
pub trait AsciiCase {
    /// Returns whether `needle` appears anywhere in `self`, ignoring ASCII case.
    ///
    /// An empty `needle` always matches, matching [`str::contains`].
    #[must_use]
    fn contains_ignore_ascii_case(&self, needle: &str) -> bool;

    /// Returns the suffix after `prefix` when `self` starts with `prefix` ignoring
    /// ASCII case, or `None` otherwise.
    ///
    /// An empty `prefix` returns `Some(self)`, matching [`str::strip_prefix`].
    #[must_use]
    fn strip_prefix_ignore_ascii_case(&self, prefix: &str) -> Option<&str>;
}

impl AsciiCase for str {
    fn contains_ignore_ascii_case(&self, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }
        let needle = needle.as_bytes();
        self.as_bytes()
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle))
    }

    fn strip_prefix_ignore_ascii_case(&self, prefix: &str) -> Option<&str> {
        self.get(..prefix.len())
            .filter(|head| head.eq_ignore_ascii_case(prefix))
            .map(|_| &self[prefix.len()..])
    }
}

#[cfg(test)]
mod tests {
    use super::AsciiCase;

    #[test]
    fn handles_empty_unicode_and_boundary_cases() {
        assert!("".contains_ignore_ascii_case(""));
        assert!("abc".contains_ignore_ascii_case(""));
        assert!(!"ab".contains_ignore_ascii_case("abc"));
        assert!("a:IS(b)".contains_ignore_ascii_case(":is("));
        assert!(!"İstanbul".contains_ignore_ascii_case("istanbul"));
        assert_eq!(
            "VAR(--x)".strip_prefix_ignore_ascii_case("var("),
            Some("--x)")
        );
        assert_eq!("αβ".strip_prefix_ignore_ascii_case("a"), None);
        assert_eq!("abc".strip_prefix_ignore_ascii_case(""), Some("abc"));
        assert_eq!("ab".strip_prefix_ignore_ascii_case("abc"), None);
    }

    proptest::proptest! {
        #[test]
        fn matches_ascii_lowercased_standard_string_semantics(
            text in ".{0,48}",
            pattern in ".{0,8}",
        ) {
            let lower_text = text.to_ascii_lowercase();
            let lower_pattern = pattern.to_ascii_lowercase();
            proptest::prop_assert_eq!(
                text.contains_ignore_ascii_case(&pattern),
                lower_text.contains(&lower_pattern)
            );
            proptest::prop_assert_eq!(
                text.strip_prefix_ignore_ascii_case(&pattern).map(str::len),
                lower_text.strip_prefix(&lower_pattern).map(str::len)
            );
        }
    }
}
