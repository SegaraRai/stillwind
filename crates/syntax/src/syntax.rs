//! Lossless parsing of one Tailwind-compatible candidate.
//!
//! A candidate is a utility string as written: variant prefixes, optional `!`
//! importance, optional `-` negativity, a utility name and an optional `/` modifier.
//! This module only splits that shape. Names are never validated against a registry
//! here, because the grammar cannot separate `bg-red-500` from `translate-x-4`; the
//! design system decides which roots exist.
//!
//! The parse is lossless in both directions: `raw` keeps the exact source spelling,
//! escapes included, for hashing and diagnostics, while arbitrary values keep their
//! decoded text for evaluation. Unbalanced delimiters and empty variant segments
//! are rejected. Arbitrary-property names and values receive no semantic validation
//! here, and may be empty.

use lean_string::{LeanStr, LeanString, ToLeanString};

use crate::arbitrary::decode_arbitrary_value;
use crate::candidate::CandidatePrefix;

/// How a variant prefix was written.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantKind {
    /// A plain name such as `hover` or `md`.
    Named(LeanStr),
    /// The inner text of `[...]`, kept verbatim with escapes intact.
    Arbitrary(LeanStr),
}

/// One variant prefix of a candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Variant {
    /// Source spelling of the whole prefix, brackets included.
    pub raw: LeanStr,
    /// Form used for matching against the variant registry.
    pub kind: VariantKind,
}

/// A `/` modifier or arbitrary-property value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    /// Literal modifier text such as `50`.
    Named(LeanStr),
    /// Decoded contents of a `[...]` modifier.
    Arbitrary(LeanStr),
    /// A `(--name)` modifier, stored with its leading `--` and without the parentheses.
    CssVariable(LeanStr),
}

/// The utility part of a candidate: either a named utility or an inline
/// arbitrary property.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Utility {
    /// A utility name with an optional `/` modifier. Arbitrary values stay inside
    /// `name`, so the registry supplies the longest valid root.
    Named {
        /// The complete utility name before a `/` modifier. Registry matching
        /// determines the longest valid root, because grammar alone cannot distinguish
        /// `bg-red-500` from `translate-x-4`.
        name: LeanStr,
        /// `/` modifier, or `None` when the candidate has no slash.
        modifier: Option<Value>,
    },
    /// A `[property:value]` candidate written directly.
    ArbitraryProperty {
        /// Property text before the first top-level `:`, for example `--sidebar`.
        property: LeanStr,
        /// Decoded value text after that colon.
        value: LeanStr,
        /// The opacity modifier after `/`, decoded with the same rules as other
        /// modifiers. It is `None` when the candidate has no `/`.
        modifier: Option<Value>,
    },
}

/// Serializes a named utility and its modifier as one candidate spelling for exact
/// plugin registration.
///
/// An arbitrary modifier is encoded again so that the key contains no whitespace and
/// decodes to the same modifier: a space becomes `_` and an underscore `\_`. Candidates
/// whose modifiers decode equally, such as `x/[calc(1+2)]` and `x/[calc(1_+_2)]`,
/// therefore share one key.
pub fn utility_registration_key(
    name: &str,
    modifier: Option<&Value>,
    negative: bool,
) -> LeanString {
    let mut key = LeanString::new();
    if negative {
        key.push('-');
    }
    key.push_str(name);
    match modifier {
        Some(Value::Named(value)) => {
            key.push('/');
            key.push_str(value);
        }
        Some(Value::Arbitrary(value)) => {
            key.push_str("/[");
            for character in value.chars() {
                match character {
                    ' ' => {
                        key.push('_');
                    }
                    '_' => {
                        key.push_str("\\_");
                    }
                    _ => {
                        key.push(character);
                    }
                }
            }
            key.push(']');
        }
        Some(Value::CssVariable(value)) => {
            key.push_str("/(");
            key.push_str(value);
            key.push(')');
        }
        None => {}
    }
    key
}

/// One parsed Tailwind-compatible candidate. The original spelling remains available
/// through [`Candidate::raw`] for stable hashing and diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    /// Exact source spelling, escapes included.
    pub raw: LeanStr,
    /// Variant prefixes in source order, outermost first.
    pub variants: Vec<Variant>,
    /// Whether `!` was written before the utility or as its trailing marker.
    pub important: bool,
    /// Whether the utility was written with a leading `-`.
    pub negative: bool,
    /// The parsed utility; still unvalidated against any registry.
    pub utility: Utility,
}

/// Parses a candidate after it has been lexed from a utility string.
///
/// Parsing is deliberately lossless: arbitrary selectors and values retain their
/// decoded spelling, while `raw` preserves the exact source candidate. Validation
/// belongs to a utility or variant registry, not this grammar layer.
///
/// `None` means the input is not a candidate of this shape at all, never
/// unknown-but-well-formed; callers must not turn it into a guess.
#[must_use]
pub fn parse_candidate(raw: &str) -> Option<Candidate> {
    let prefix = CandidatePrefix::parse(raw)?;
    if raw.is_empty() || !prefix.is_complete() {
        return None;
    }
    let (variant_parts, utility) = prefix.split(':');
    let utility = utility?;
    let variants = variant_parts.into_iter().map(variant).collect();
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
        raw: raw.into(),
        variants,
        important: important || important_suffix,
        negative,
        utility,
    })
}

/// Parses a variant prefix without a utility, such as `hover:data-[open]:` in a style
/// object key.
///
/// Returns the variants in source order, or `None` when `raw` does not end with a
/// top-level `:`, when a variant is empty, or when delimiters are unbalanced.
pub fn parse_variant_prefix(raw: &str) -> Option<Vec<Variant>> {
    let stem = raw.strip_suffix(':')?;
    let prefix = CandidatePrefix::parse(stem)?;
    if stem.is_empty() || !prefix.is_complete() {
        return None;
    }
    let (mut parts, last) = prefix.split(':');
    parts.push(last?.into());
    parts
        .iter()
        .all(|part| !part.is_empty())
        .then(|| parts.into_iter().map(variant).collect())
}

/// Classifies one variant spelling as an arbitrary or a named variant.
fn variant(raw: LeanStr) -> Variant {
    Variant {
        kind: if let Some(value) = arbitrary(&raw) {
            VariantKind::Arbitrary(value.into())
        } else {
            VariantKind::Named(raw.clone())
        },
        raw,
    }
}

/// Parses the utility after variant prefixes have been removed.
///
/// A `[...]`-wrapped utility is an arbitrary property when its inner text contains a
/// `:`; the first colon is used without checking its nesting depth; otherwise it is
/// treated as a named utility whose name happens to be bracketed, leaving the registry
/// to resolve it.
fn parse_utility(raw: &str) -> Option<Utility> {
    let prefix = CandidatePrefix::parse(raw)?;
    let (before_modifier, modifier) = prefix.split_once('/');
    let modifier = match modifier {
        Some(value) => Some(parse_value(value)?),
        None => None,
    };
    if let Some(value) = arbitrary(before_modifier)
        && let Some((property, value)) = value.split_once(':')
    {
        return Some(Utility::ArbitraryProperty {
            property: property.into(),
            value: decode_arbitrary_value(value).into(),
            modifier,
        });
    }
    (!before_modifier.is_empty()).then(|| Utility::Named {
        name: before_modifier.into(),
        modifier,
    })
}

/// Parses one `/` modifier.
///
/// A parenthesized modifier is accepted only in the CSS-variable form `(--name)`; any
/// other parenthesized text is rejected rather than kept as a name.
fn parse_value(value: &str) -> Option<Value> {
    if let Some(value) = arbitrary(value) {
        return Some(Value::Arbitrary(decode_arbitrary_value(value).into()));
    }
    if let Some(value) = value
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
    {
        return value
            .strip_prefix("--")
            .map(|value| Value::CssVariable(format_args!("--{value}").to_lean_string().into()));
    }
    (!value.is_empty()).then(|| Value::Named(value.into()))
}

/// Returns the contents of a `[..]` wrapper.
///
/// Nesting and escaping were already checked by
/// [`crate::CandidatePrefix::is_complete`], so this only strips the outermost pair.
fn arbitrary(value: &str) -> Option<&str> {
    value.strip_prefix('[')?.strip_suffix(']')
}

/// Strips a trailing `suffix` unless it is escaped.
///
/// A preceding odd number of backslashes escapes the marker, so `flex\!` keeps the `!`
/// as part of the utility name instead of marking importance.
fn strip_unescaped_suffix(value: &str, suffix: char) -> Option<&str> {
    let value = value.strip_suffix(suffix)?;
    let slash_count = value
        .chars()
        .rev()
        .take_while(|character| *character == '\\')
        .count();
    (slash_count % 2 == 0).then_some(value)
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
                    raw: "hover".into(),
                    kind: VariantKind::Named("hover".into()),
                },
                Variant {
                    raw: "md".into(),
                    kind: VariantKind::Named("md".into()),
                },
            ]
        );
        assert_eq!(
            candidate.utility,
            Utility::Named {
                name: "bg-red-500".into(),
                modifier: Some(Value::Arbitrary(".35".into())),
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
                name: "translate-x-[calc(100%_-_1rem)]".into(),
                modifier: None,
            }
        );
        assert_eq!(
            parse_candidate("w-(--sidebar)").unwrap().utility,
            Utility::Named {
                name: "w-(--sidebar)".into(),
                modifier: None,
            }
        );
        assert_eq!(
            parse_candidate("[color:rebeccapurple]").unwrap().utility,
            Utility::ArbitraryProperty {
                property: "color".into(),
                value: "rebeccapurple".into(),
                modifier: None,
            }
        );
    }

    #[test]
    fn parses_every_modifier_spelling_without_losing_the_utility_name() {
        let cases = [
            (
                "bg-red-500/50",
                Utility::Named {
                    name: "bg-red-500".into(),
                    modifier: Some(Value::Named("50".into())),
                },
            ),
            (
                "bg-red-500/[.35]",
                Utility::Named {
                    name: "bg-red-500".into(),
                    modifier: Some(Value::Arbitrary(".35".into())),
                },
            ),
            (
                "bg-red-500/(--brand-opacity)",
                Utility::Named {
                    name: "bg-red-500".into(),
                    modifier: Some(Value::CssVariable("--brand-opacity".into())),
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
                    raw: "[@supports(display:grid)]".into(),
                    kind: VariantKind::Arbitrary("@supports(display:grid)".into()),
                },
                Variant {
                    raw: "[&:nth-child(3)]".into(),
                    kind: VariantKind::Arbitrary("&:nth-child(3)".into()),
                },
            ]
        );
        assert_eq!(
            candidate.utility,
            Utility::Named {
                name: "content-['hello\\_world']".into(),
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
                property: "--sidebar".into(),
                value: "calc(100% - 1rem)".into(),
                modifier: None,
            }
        );
        assert_eq!(
            parse_candidate("content-['hello\\_world']")
                .unwrap()
                .utility,
            Utility::Named {
                name: "content-['hello\\_world']".into(),
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
                    name: raw.into(),
                    modifier: None,
                },
                "{raw}"
            );
        }
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn registration_keys_preserve_nested_values_and_escapes() {
        let key = |raw| {
            let candidate = parse_candidate(raw).unwrap();
            let Utility::Named { name, modifier } = candidate.utility else {
                panic!("expected a named utility");
            };
            utility_registration_key(&name, modifier.as_ref(), candidate.negative)
        };
        assert_eq!(key("-probe-one/[foo_bar]"), "-probe-one/[foo_bar]");
        assert_eq!(key(r"probe-one/[foo\_bar]"), r"probe-one/[foo\_bar]");
        assert_eq!(key("probe/(--x)"), "probe/(--x)");
    }

    #[test]
    fn quoted_delimiters_do_not_end_values_or_create_variants_and_modifiers() {
        let candidate = parse_candidate(r#"hover:content-["']:)/"]/50"#).unwrap();
        assert_eq!(candidate.variants.len(), 1);
        assert_eq!(
            candidate.utility,
            Utility::Named {
                name: r#"content-["']:)/"]"#.into(),
                modifier: Some(Value::Named("50".into())),
            }
        );
        assert!(parse_candidate("content-['unterminated]").is_none());
        assert!(parse_candidate(r"content-['it\'s]']").is_some());
    }

    proptest::proptest! {
        #[test]
        fn valid_named_candidates_keep_their_original_spelling(name in "[a-z]{1,12}-[0-9]{1,4}") {
            let candidate = parse_candidate(&name).unwrap();
            proptest::prop_assert!(!candidate.important);
            proptest::prop_assert!(!candidate.negative);
            proptest::prop_assert_eq!(candidate.utility, Utility::Named{name: name.into(), modifier:None});
        }
    }
}
