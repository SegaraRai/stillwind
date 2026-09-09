//! Lossless parsing of one Tailwind-compatible candidate.

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantKind {
    Named(String),
    Arbitrary(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Variant {
    pub raw: String,
    pub kind: VariantKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    Named(String),
    Arbitrary(String),
    CssVariable(String),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_variants_modifiers_and_important_candidates() {
        let candidate = parse_candidate("hover:md:!bg-red-500/[.35]").unwrap();

        assert_eq!(candidate.raw, "hover:md:!bg-red-500/[.35]");
        assert!(candidate.important);
        assert!(!candidate.negative);
        assert_eq!(
            candidate.variants,
            vec![
                Variant {
                    raw: "hover".to_owned(),
                    kind: VariantKind::Named("hover".to_owned()),
                },
                Variant {
                    raw: "md".to_owned(),
                    kind: VariantKind::Named("md".to_owned()),
                },
            ]
        );
        assert_eq!(
            candidate.utility,
            Utility::Named {
                name: "bg-red-500".to_owned(),
                modifier: Some(Value::Arbitrary(".35".to_owned())),
            }
        );
    }

    #[test]
    fn parses_negative_arbitrary_and_css_variable_candidates() {
        assert_eq!(
            parse_candidate("-translate-x-[calc(100%_-_1rem)]")
                .unwrap()
                .utility,
            Utility::Named {
                name: "translate-x-[calc(100%_-_1rem)]".to_owned(),
                modifier: None,
            }
        );
        assert_eq!(
            parse_candidate("w-(--sidebar)").unwrap().utility,
            Utility::Named {
                name: "w-(--sidebar)".to_owned(),
                modifier: None,
            }
        );
        assert_eq!(
            parse_candidate("[color:rebeccapurple]").unwrap().utility,
            Utility::ArbitraryProperty {
                property: "color".to_owned(),
                value: "rebeccapurple".to_owned(),
            }
        );
    }

    #[test]
    fn parses_every_modifier_spelling_without_losing_the_utility_name() {
        let cases = [
            (
                "bg-red-500/50",
                Utility::Named {
                    name: "bg-red-500".to_owned(),
                    modifier: Some(Value::Named("50".to_owned())),
                },
            ),
            (
                "bg-red-500/[.35]",
                Utility::Named {
                    name: "bg-red-500".to_owned(),
                    modifier: Some(Value::Arbitrary(".35".to_owned())),
                },
            ),
            (
                "bg-red-500/(--brand-opacity)",
                Utility::Named {
                    name: "bg-red-500".to_owned(),
                    modifier: Some(Value::CssVariable("--brand-opacity".to_owned())),
                },
            ),
        ];

        for (raw, utility) in cases {
            assert_eq!(parse_candidate(raw).unwrap().utility, utility, "{raw}");
        }
    }

    #[test]
    fn retains_nested_colons_in_arbitrary_variants_and_values() {
        let candidate =
            parse_candidate("[@supports(display:grid)]:[&:nth-child(3)]:content-['hello\\_world']")
                .unwrap();

        assert_eq!(
            candidate.variants,
            vec![
                Variant {
                    raw: "[@supports(display:grid)]".to_owned(),
                    kind: VariantKind::Arbitrary("@supports(display:grid)".to_owned()),
                },
                Variant {
                    raw: "[&:nth-child(3)]".to_owned(),
                    kind: VariantKind::Arbitrary("&:nth-child(3)".to_owned()),
                },
            ]
        );
        assert_eq!(
            candidate.utility,
            Utility::Named {
                name: "content-['hello\\_world']".to_owned(),
                modifier: None,
            }
        );
    }

    #[test]
    fn parses_prefix_and_suffix_important_with_negative_utilities() {
        let cases = [("!flex", false), ("flex!", false), ("!-mt-4", true)];

        for (raw, negative) in cases {
            let candidate = parse_candidate(raw).unwrap();
            assert!(candidate.important, "{raw}");
            assert_eq!(candidate.negative, negative, "{raw}");
        }
    }

    #[test]
    fn decodes_arbitrary_property_values_but_preserves_css_escapes_elsewhere() {
        assert_eq!(
            parse_candidate("[--sidebar:calc(100%_-_1rem)]")
                .unwrap()
                .utility,
            Utility::ArbitraryProperty {
                property: "--sidebar".to_owned(),
                value: "calc(100% - 1rem)".to_owned(),
            }
        );
        assert_eq!(
            parse_candidate("content-['hello\\_world']")
                .unwrap()
                .utility,
            Utility::Named {
                name: "content-['hello\\_world']".to_owned(),
                modifier: None,
            }
        );
    }

    #[test]
    fn rejects_empty_dangling_and_unbalanced_candidates() {
        for raw in ["", ":", "hover:", "!", "bg-red-500/", "[", "hover:[&:focus"] {
            assert!(parse_candidate(raw).is_none(), "{raw}");
        }
    }

    #[test]
    fn preserves_escaped_top_level_delimiters_and_important_markers() {
        let cases = ["icon\\:large", "bg-red\\/500", "flex\\!", "content-[\\]]"];

        for raw in cases {
            let candidate = parse_candidate(raw).unwrap();
            assert!(candidate.variants.is_empty(), "{raw}");
            assert!(!candidate.important, "{raw}");
            assert_eq!(
                candidate.utility,
                Utility::Named {
                    name: raw.to_owned(),
                    modifier: None,
                },
                "{raw}"
            );
        }
    }
}
