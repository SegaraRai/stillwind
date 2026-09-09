//! Stillwind's small, source-spanned utility-string lexer.

use stillwind_source::{Span, Spanned};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilityToken {
    pub raw: String,
}

/// One parsed Tailwind-compatible candidate. The original spelling remains
/// available through [`Candidate::raw`] for stable hashing and diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub raw: String,
    pub variants: Vec<Variant>,
    pub important: bool,
    pub negative: bool,
    pub utility: Utility,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Variant {
    pub raw: String,
    pub kind: VariantKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantKind {
    Named(String),
    Arbitrary(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Utility {
    Named {
        /// The complete utility name before a `/` modifier. Registry matching
        /// determines the longest valid root, because grammar alone cannot
        /// distinguish `bg-red-500` from `translate-x-4`.
        name: String,
        modifier: Option<Value>,
    },
    ArbitraryProperty {
        property: String,
        value: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    Named(String),
    Arbitrary(String),
    CssVariable(String),
}

/// Splits one decoded utility string on Unicode whitespace while retaining a
/// source span for every token.
pub fn lex_utilities(input: &Spanned<String>) -> Vec<Spanned<UtilityToken>> {
    let mut tokens = Vec::new();
    let mut token_start = None;

    for (offset, character) in input.value.char_indices() {
        if character.is_whitespace() {
            if let Some(start) = token_start.take() {
                push_token(input, start, offset, &mut tokens);
            }
        } else if token_start.is_none() {
            token_start = Some(offset);
        }
    }

    if let Some(start) = token_start {
        push_token(input, start, input.value.len(), &mut tokens);
    }

    tokens
}

/// Parses a candidate after it has been lexed from a utility string.
///
/// Parsing is deliberately lossless: arbitrary selectors and values retain
/// their decoded spelling, while `raw` preserves the exact source candidate.
/// Validation belongs to a utility or variant registry, not this grammar
/// layer.
#[must_use]
pub fn parse_candidate(raw: &str) -> Option<Candidate> {
    if raw.is_empty() || !balanced(raw) {
        return None;
    }
    let (variant_parts, utility) = split_top_level(raw, ':');
    let utility = utility?;
    let variants = variant_parts
        .into_iter()
        .map(|raw| Variant {
            kind: if let Some(value) = arbitrary(&raw) {
                VariantKind::Arbitrary(value.to_owned())
            } else {
                VariantKind::Named(raw.clone())
            },
            raw,
        })
        .collect();
    let (important, utility) = utility
        .strip_prefix('!')
        .map_or((false, utility), |value| (true, value));
    let (important_suffix, utility) =
        strip_unescaped_suffix(utility, '!').map_or((false, utility), |value| (true, value));
    let (negative, utility) = utility
        .strip_prefix('-')
        .map_or((false, utility), |value| (true, value));
    let utility = parse_utility(utility)?;
    Some(Candidate {
        raw: raw.to_owned(),
        variants,
        important: important || important_suffix,
        negative,
        utility,
    })
}

fn balanced(value: &str) -> bool {
    let mut delimiters = Vec::new();
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        match character {
            '[' => delimiters.push(']'),
            '(' => delimiters.push(')'),
            ']' | ')' if delimiters.pop() != Some(character) => return false,
            _ => {}
        }
    }
    delimiters.is_empty() && !escaped
}

fn parse_utility(raw: &str) -> Option<Utility> {
    if let Some(value) = arbitrary(raw)
        && let Some((property, value)) = value.split_once(':')
    {
        return Some(Utility::ArbitraryProperty {
            property: property.to_owned(),
            value: decode_arbitrary(value),
        });
    }
    let (before_modifier, modifier) = split_once_top_level(raw, '/');
    let modifier = match modifier {
        Some(value) => Some(parse_value(value)?),
        None => None,
    };
    (!before_modifier.is_empty()).then(|| Utility::Named {
        name: before_modifier.to_owned(),
        modifier,
    })
}

fn parse_value(value: &str) -> Option<Value> {
    if let Some(value) = arbitrary(value) {
        return Some(Value::Arbitrary(decode_arbitrary(value)));
    }
    if let Some(value) = value
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
    {
        return value
            .strip_prefix("--")
            .map(|value| Value::CssVariable(format!("--{value}")));
    }
    (!value.is_empty()).then(|| Value::Named(value.to_owned()))
}

fn arbitrary(value: &str) -> Option<&str> {
    value.strip_prefix('[')?.strip_suffix(']')
}

fn split_top_level(value: &str, delimiter: char) -> (Vec<String>, Option<&str>) {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0_u32;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        match character {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            character if character == delimiter && depth == 0 => {
                parts.push(value[start..index].to_owned());
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    (
        parts,
        (!value[start..].is_empty()).then_some(&value[start..]),
    )
}

fn split_once_top_level(value: &str, delimiter: char) -> (&str, Option<&str>) {
    let mut depth = 0_u32;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        match character {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            character if character == delimiter && depth == 0 => {
                return (
                    &value[..index],
                    Some(&value[index + character.len_utf8()..]),
                );
            }
            _ => {}
        }
    }
    (value, None)
}

fn strip_unescaped_suffix(value: &str, suffix: char) -> Option<&str> {
    let value = value.strip_suffix(suffix)?;
    let slash_count = value
        .chars()
        .rev()
        .take_while(|character| *character == '\\')
        .count();
    (slash_count % 2 == 0).then_some(value)
}

fn decode_arbitrary(value: &str) -> String {
    let mut output = String::new();
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            output.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '_' {
            output.push(' ');
        } else {
            output.push(character);
        }
    }
    output
}

fn push_token(
    input: &Spanned<String>,
    start: usize,
    end: usize,
    tokens: &mut Vec<Spanned<UtilityToken>>,
) {
    let start = u32::try_from(start).expect("utility strings must be smaller than 4 GiB");
    let end = u32::try_from(end).expect("utility strings must be smaller than 4 GiB");
    tokens.push(Spanned::new(
        UtilityToken {
            raw: input.value[start as usize..end as usize].to_owned(),
        },
        Span::new(
            input.span.file,
            input.span.start.0 + start,
            input.span.start.0 + end,
        ),
    ));
}

#[cfg(test)]
mod tests;
