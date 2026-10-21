//! Shared structural parsing of complete and unfinished candidate spellings.
//!
//! Separators use Tailwind's spelling rules, before arbitrary values are decoded as
//! CSS. Prefixes retain unmatched openers, but mismatched closers fail.

use lean_string::{LeanStr, LeanString};

/// Structure of a candidate or an unfinished candidate prefix.
#[derive(Debug)]
pub struct CandidatePrefix<'a> {
    raw: &'a str,
    separators: Vec<(usize, char)>,
    closing: Vec<char>,
    parenthesis_depth: usize,
    parenthesis_openings: Vec<usize>,
    quote: Option<char>,
    escaped: bool,
}

impl<'a> CandidatePrefix<'a> {
    /// Reads raw spelling, retaining EOF inside a quote, escape, or group. Returns
    /// `None` for a closing bracket of the wrong kind or without an opener.
    pub fn parse(raw: &'a str) -> Option<Self> {
        let mut result = Self {
            raw,
            separators: Vec::new(),
            closing: Vec::new(),
            parenthesis_depth: 0,
            parenthesis_openings: Vec::new(),
            quote: None,
            escaped: false,
        };
        for (offset, character) in raw.char_indices() {
            result.advance(offset, character)?;
        }
        Some(result)
    }

    /// Records one character and its byte offset. Returns `None` if a closing bracket
    /// has no opener of the same kind; quotes and escapes keep their delimiters
    /// literal.
    fn advance(&mut self, offset: usize, character: char) -> Option<()> {
        if self.escaped {
            self.escaped = false;
            return Some(());
        }
        if character == '\\' {
            self.escaped = true;
            return Some(());
        }
        if let Some(quote) = self.quote {
            if character == quote {
                self.quote = None;
            }
            return Some(());
        }
        match character {
            '\'' | '"' => {
                self.quote = Some(character);
            }
            '[' => {
                self.closing.push(']');
            }
            '(' => {
                self.parenthesis_depth += 1;
                if self.parenthesis_depth > self.parenthesis_openings.len() {
                    self.parenthesis_openings.push(offset);
                }
                self.closing.push(')');
            }
            ']' | ')' => {
                if self.closing.pop() != Some(character) {
                    return None;
                }
                if character == ')' {
                    self.parenthesis_depth -= 1;
                }
            }
            ':' | '/' if self.closing.is_empty() => {
                self.separators.push((offset, character));
            }
            _ => {}
        }
        Some(())
    }

    /// Reports whether all groups, quotes, and escapes are complete.
    pub fn is_complete(&self) -> bool {
        self.closing.is_empty() && self.quote.is_none() && !self.escaped
    }

    /// Returns the first parenthesis exceeding limit, using the undecoded candidate
    /// grammar. Quoted and escaped parentheses never increase the depth.
    pub fn nesting_overflow(&self, limit: usize) -> Option<usize> {
        self.parenthesis_openings.get(limit).copied()
    }

    /// Returns the last top-level `:` or `/` byte offset in the original spelling.
    pub fn last_separator(&self, delimiter: char) -> Option<usize> {
        self.separators
            .iter()
            .rev()
            .find_map(|&(offset, character)| (character == delimiter).then_some(offset))
    }

    /// Returns missing bracket closers, innermost first. An open quote or dangling
    /// escape has no safe bracket-only completion.
    pub fn completion_closers(&self) -> Option<LeanString> {
        (self.quote.is_none() && !self.escaped).then(|| self.closing.iter().rev().collect())
    }

    /// Splits at top-level `:` or `/`, preserving each component's spelling.
    pub(crate) fn split(&self, delimiter: char) -> (Vec<LeanStr>, Option<&'a str>) {
        let mut parts = Vec::new();
        let mut start = 0;
        for &(offset, character) in &self.separators {
            if character == delimiter {
                parts.push(self.raw[start..offset].into());
                start = offset + 1;
            }
        }
        (
            parts,
            (!self.raw[start..].is_empty()).then_some(&self.raw[start..]),
        )
    }

    /// Splits at the first top-level `:` or `/` without trimming.
    pub fn split_once(&self, delimiter: char) -> (&'a str, Option<&'a str>) {
        self.separators
            .iter()
            .find(|(_, character)| *character == delimiter)
            .map_or((self.raw, None), |&(offset, _)| {
                (&self.raw[..offset], Some(&self.raw[offset + 1..]))
            })
    }
}

/// Finds possible candidates in arbitrary source text, including comments and strings.
/// Host punctuation separates candidates; quotes and punctuation inside bracketed
/// arbitrary values remain in the candidate. Registry lookup belongs to the caller.
pub fn source_candidate_tokens(text: &str) -> Vec<&str> {
    let mut state = CandidatePrefix {
        raw: text,
        separators: Vec::new(),
        closing: Vec::new(),
        parenthesis_depth: 0,
        parenthesis_openings: Vec::new(),
        quote: None,
        escaped: false,
    };
    let mut result = Vec::new();
    let mut start = 0;
    for (index, character) in text.char_indices() {
        let top_level = state.closing.is_empty() && state.quote.is_none() && !state.escaped;
        let boundary = top_level
            && (character.is_whitespace()
                || matches!(
                    character,
                    '"' | '\u{27}' | '`' | ',' | ';' | '{' | '}' | '<' | '>' | '='
                )
                || (character == '(' && !text[start..index].ends_with(['-', '/']))
                || character == ')');
        if boundary {
            if start < index {
                result.push(&text[start..index]);
            }
            start = index + character.len_utf8();
            state.closing.clear();
            state.parenthesis_depth = 0;
            state.parenthesis_openings.clear();
            state.quote = None;
            state.escaped = false;
            state.separators.clear();
        } else if state.advance(index, character).is_none() {
            state.closing.clear();
            state.parenthesis_depth = 0;
            state.parenthesis_openings.clear();
            state.quote = None;
            state.escaped = false;
        }
    }
    if start < text.len() {
        result.push(&text[start..]);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separators_preserve_nested_groups_and_escaped_spelling() {
        assert_eq!(
            CandidatePrefix::parse("hover:[&:focus]:x")
                .unwrap()
                .split(':'),
            (vec!["hover".into(), "[&:focus]".into()], Some("x"))
        );
        for (input, expected) in [
            ("a/[b/c]", ("a", Some("[b/c]"))),
            ("[x/y]/group", ("[x/y]", Some("group"))),
            ("[x/y]", ("[x/y]", None)),
        ] {
            assert_eq!(
                CandidatePrefix::parse(input).unwrap().split_once('/'),
                expected
            );
        }
        assert!(CandidatePrefix::parse("[f(']')]").unwrap().is_complete());
    }

    #[test]
    fn nesting_limits_use_the_undecoded_candidate_grammar() {
        let raw = format!("probe-[{}1{}]", "(".repeat(65), ")".repeat(65));
        let prefix = CandidatePrefix::parse(&raw).unwrap();
        assert_eq!(prefix.nesting_overflow(64), Some("probe-[".len() + 64));
        assert_eq!(prefix.nesting_overflow(65), None);
        assert_eq!(
            CandidatePrefix::parse("content-['(((((']")
                .unwrap()
                .nesting_overflow(0),
            None
        );
    }

    #[test]
    fn source_scanning_preserves_quoted_brackets_and_variable_modifiers() {
        let source = r#"style("icon-[f(']')] icon-(--name)/[x] data-[x=')']:icon-a", other)"#;
        assert_eq!(
            source_candidate_tokens(source),
            [
                "style",
                "icon-[f(']')]",
                "icon-(--name)/[x]",
                "data-[x=')']:icon-a",
                "other"
            ]
        );
    }

    #[test]
    fn prefixes_share_quote_escape_and_delimiter_rules_with_complete_candidates() {
        let raw = r"hover:content-['a:b/c']:bg-[var(--x";
        let prefix = CandidatePrefix::parse(raw).unwrap();
        assert!(!prefix.is_complete());
        assert_eq!(prefix.last_separator(':'), raw.rfind(':'));
        assert_eq!(prefix.last_separator('/'), None);
        assert_eq!(prefix.completion_closers().as_deref(), Some(")]"));
        assert!(CandidatePrefix::parse("hover:[x)").is_none());
        assert!(CandidatePrefix::parse("hover:x]").is_none());
        assert_eq!(
            CandidatePrefix::parse("content-['x")
                .unwrap()
                .completion_closers(),
            None
        );
        assert!(
            CandidatePrefix::parse(r"content-['a:b/c']")
                .unwrap()
                .is_complete()
        );
    }
}
