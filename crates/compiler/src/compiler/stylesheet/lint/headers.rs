//! Comparable forms of the headers of nested style rules and conditional at-rules.
//!
//! `prefer-variant-directive` compares the headers that authored CSS writes with the
//! headers that `@variant` expands to. [`Header`] keeps only the differences that can
//! change the meaning. It removes whitespace that CSS ignores and comments. It
//! lowercases media conditions and pseudo-class names, and it writes `min-` and `max-`
//! media and container features in range syntax. [`header_key`] orders two nested
//! headers when their order does not change the result.

use cssparser::{Parser, Token};
use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_css::{
    CssCase, absolutize_nested_selector, normalize_css_fragment, parse_css_block,
    selector_list_branches,
};

/// One authored or expanded header, as its text appears in the stylesheet.
#[derive(Clone, Copy, Debug)]
pub(super) enum Level<'a> {
    /// Selector of a style rule.
    Rule(&'a str),
    /// At-rule name in ASCII lowercase and its prelude.
    AtRule(&'a str, &'a str),
}

/// Header of one nested style rule or conditional at-rule in comparable form.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) enum Header {
    /// Selector of a style rule, with `&` written where nesting implies it.
    Selector(LeanString),
    /// `@media`, `@supports`, `@container`, or `@starting-style` with its prelude.
    AtRule {
        /// At-rule name in ASCII lowercase.
        name: LeanStr,
        /// Prelude in comparable form.
        prelude: LeanString,
    },
}

impl Header {
    /// Returns the comparable form of `level`.
    ///
    /// With `px_to_rem`, `px` and `rem` lengths of a `@media` condition become `px` at
    /// 16px = 1rem. Returns `None` for a selector list, which `@variant` cannot
    /// replace, and for an at-rule that no variant applies as a condition.
    pub(super) fn of(level: Level<'_>, px_to_rem: bool) -> Option<Self> {
        match level {
            Level::Rule(selector) => (selector_list_branches(selector).len() == 1).then(|| {
                Self::Selector(normalize_css_fragment(
                    &absolutize_nested_selector(selector),
                    SELECTOR_SPACING,
                    CssCase::PseudoClasses,
                ))
            }),
            Level::AtRule(name, prelude) => {
                let prelude = match name {
                    "media" => {
                        let case = if prelude.contains("--") {
                            CssCase::Keep
                        } else {
                            CssCase::All
                        };
                        let condition =
                            ranges(&normalize_css_fragment(prelude, CONDITION_SPACING, case));
                        if px_to_rem {
                            px_lengths(&condition)
                        } else {
                            condition
                        }
                    }
                    "container" => ranges(&normalize_css_fragment(
                        prelude,
                        CONDITION_SPACING,
                        CssCase::Keep,
                    )),
                    // `selector()` arguments are selectors, in which a space before `:`
                    // is a descendant combinator.
                    "supports" if prelude.contains("selector(") => {
                        normalize_css_fragment(prelude, SELECTOR_SPACING, CssCase::Keep)
                    }
                    "supports" => normalize_css_fragment(prelude, CONDITION_SPACING, CssCase::Keep),
                    "starting-style" if prelude.trim().is_empty() => LeanString::new(),
                    _ => return None,
                };
                Some(Self::AtRule {
                    name: name.into(),
                    prelude,
                })
            }
        }
    }
}

/// Returns the comparable form of the nested `levels`, outermost first, or `None` when
/// a level has no comparable form.
///
/// A conditional at-rule applies to the selector of its parent rule, so it commutes
/// with a nested style rule and with another at-rule. Two levels are sorted unless both
/// are style rules, because nested selectors do not commute.
pub(super) fn header_key(levels: &[Level<'_>], px_to_rem: bool) -> Option<Vec<Header>> {
    let mut key = levels
        .iter()
        .map(|&level| Header::of(level, px_to_rem))
        .collect::<Option<Vec<_>>>()?;
    if !key
        .iter()
        .all(|header| matches!(header, Header::Selector(_)))
    {
        key.sort();
    }
    Some(key)
}

/// Characters around which whitespace is removed from a selector.
const SELECTOR_SPACING: &[char] = &['>', '+', '~', ','];

/// Characters around which whitespace is removed from a condition.
const CONDITION_SPACING: &[char] = &[':', ',', '<', '>', '='];

/// Rewrites the features of each innermost parenthesized group of a normalized
/// condition into range syntax with the feature name first.
///
/// `(min-width:40rem)` becomes `(width>=40rem)` and `(max-width:40rem)` becomes
/// `(width<=40rem)`, as Media Queries Level 4 defines them. `(40rem<=width)` becomes
/// `(width>=40rem)`. Other groups stay as written.
fn ranges(condition: &str) -> LeanString {
    fn walk(parser: &mut Parser<'_>, source: &str) -> (LeanString, bool) {
        let mut output = LeanString::new();
        let mut groups = false;
        loop {
            let start = parser.position().byte_index();
            let Ok(token) = parser.next_including_whitespace_and_comments().cloned() else {
                break;
            };
            let end = parser.position().byte_index();
            output.push_str(&source[start..end]);
            let parenthesis = matches!(token, Token::ParenthesisBlock);
            if matches!(
                token,
                Token::ParenthesisBlock
                    | Token::Function(_)
                    | Token::SquareBracketBlock
                    | Token::CurlyBracketBlock
            ) && let Ok(((inner, children), block)) =
                parse_css_block(parser, |inner| walk(inner, source))
            {
                output.push_str(&if parenthesis && !children {
                    range_feature(&inner)
                } else {
                    inner
                });
                if let Some(close) = block.close {
                    output.push_str(&source[close..=close]);
                }
                groups |= parenthesis || children;
            }
        }
        (output, groups)
    }
    walk(&mut Parser::new(condition), condition).0
}

/// Returns one feature test of [`ranges`] in range syntax, or `feature` unchanged.
fn range_feature(feature: &str) -> LeanString {
    let is_name = |text: &str| {
        text.starts_with(|character: char| character.is_ascii_alphabetic())
            && text
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
    };
    if let Some((name, value)) = feature.split_once(':') {
        for (prefix, operator) in [("min-", ">="), ("max-", "<=")] {
            if let Some(name) = name.strip_prefix(prefix)
                && is_name(name)
            {
                return format_args!("{name}{operator}{value}").to_lean_string();
            }
        }
        return feature.into();
    }
    let Some(start) = feature.find(['<', '>', '=']) else {
        return feature.into();
    };
    let length = if feature[start + 1..].starts_with('=') {
        2
    } else {
        1
    };
    let (value, operator, name) = (
        &feature[..start],
        &feature[start..start + length],
        &feature[start + length..],
    );
    if !is_name(name) || is_name(value) || name.contains(['<', '>', '=']) {
        return feature.into();
    }
    let flipped = match operator {
        "<" => ">",
        ">" => "<",
        "<=" => ">=",
        ">=" => "<=",
        _ => operator,
    };
    format_args!("{name}{flipped}{value}").to_lean_string()
}

/// Returns `condition` with each `px` and `rem` length written in `px` at 16px = 1rem.
fn px_lengths(condition: &str) -> LeanString {
    fn walk(parser: &mut Parser<'_>, css: &str, output: &mut LeanString) {
        loop {
            let start = parser.position().byte_index();
            let Ok(token) = parser.next_including_whitespace_and_comments().cloned() else {
                break;
            };
            let end = parser.position().byte_index();
            let raw = &css[start..end];
            let converted = if let Token::Dimension { unit, .. } = &token {
                let scale = match unit.as_ref() {
                    "px" => Some(1.0),
                    "rem" => Some(16.0),
                    _ => None,
                };
                scale.and_then(|scale| {
                    raw.strip_suffix(unit.as_ref())?
                        .parse::<f64>()
                        .ok()
                        .map(|number| number * scale)
                })
            } else {
                None
            };
            if let Some(number) = converted {
                output.push_str(&format_args!("{number}px").to_lean_string());
            } else {
                output.push_str(raw);
            }
            if matches!(
                token,
                Token::Function(_)
                    | Token::ParenthesisBlock
                    | Token::SquareBracketBlock
                    | Token::CurlyBracketBlock
            ) && let Ok(((), block)) = parse_css_block(parser, |inner| walk(inner, css, output))
                && let Some(close) = block.close
            {
                output.push_str(&css[close..=close]);
            }
        }
    }
    let mut output = LeanString::new();
    walk(&mut Parser::new(condition), condition, &mut output);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media(prelude: &str, px_to_rem: bool) -> Option<Header> {
        Header::of(Level::AtRule("media", prelude), px_to_rem)
    }

    #[test]
    fn media_conditions_compare_without_spacing_case_or_prefix_form() {
        let expected = media("(width >= 40rem)", false);
        for written in [
            "(width>=40rem)",
            "( WIDTH >= 40rem )",
            "(min-width: 40rem)",
            "(min-width:40rem)",
            "(40rem <= width)",
            "/* sm */ (width >= 40rem)",
        ] {
            assert_eq!(media(written, false), expected, "{written}");
        }
        for different in [
            "(max-width: 40rem)",
            "(width > 40rem)",
            "(width >= 640px)",
            "screen and (width >= 40rem)",
        ] {
            assert_ne!(media(different, false), expected, "{different}");
        }
        assert_eq!(
            media("(max-width: 40rem)", false),
            media("(width <= 40rem)", false)
        );
        assert_ne!(
            media("(max-width: 40rem)", false),
            media("(width < 40rem)", false)
        );
    }

    #[test]
    fn px_lengths_match_rem_lengths_only_when_converted() {
        assert_eq!(
            media("(min-width: 640px)", true),
            media("(width >= 40rem)", true)
        );
        assert_eq!(
            media("(width >= 40.5rem)", true),
            media("(width >= 648px)", true)
        );
        assert_ne!(
            media("(min-width: 640px)", false),
            media("(width >= 40rem)", false)
        );
        assert_ne!(
            media("(width >= 40em)", true),
            media("(width >= 640px)", true)
        );
    }

    #[test]
    fn selectors_keep_combinators_strings_and_class_case() {
        let selector = |text| Header::of(Level::Rule(text), false);
        assert_eq!(selector("&:Focus-Visible"), selector("&:focus-visible"));
        assert_eq!(selector("& >  .item"), selector("&>.item"));
        assert_eq!(selector(".item"), selector("& .item"));
        assert_ne!(selector("& :hover"), selector("&:hover"));
        assert_ne!(selector("&.Open"), selector("&.open"));
        assert_ne!(selector("&[title='a  b']"), selector("&[title='a b']"));
        assert_eq!(selector("&:hover, &:focus"), None);
    }

    #[test]
    fn supports_conditions_compare_without_spacing() {
        let supports = |text| Header::of(Level::AtRule("supports", text), false);
        assert_eq!(supports("( display : grid )"), supports("(display:grid)"));
        assert_ne!(
            supports("not (display:grid)"),
            supports("not(display:grid)")
        );
        assert_ne!(
            supports("selector(& :hover)"),
            supports("selector(&:hover)")
        );
    }

    #[test]
    fn nested_conditions_commute_with_their_rule() {
        let rule = Level::Rule("&:hover");
        let condition = Level::AtRule("media", "(hover: hover)");
        assert_eq!(
            header_key(&[rule, condition], false),
            header_key(&[condition, rule], false)
        );
        let inner = Level::Rule("& .item");
        assert_ne!(
            header_key(&[rule, inner], false),
            header_key(&[inner, rule], false)
        );
        assert_eq!(header_key(&[Level::AtRule("layer", "x")], false), None);
    }
}
