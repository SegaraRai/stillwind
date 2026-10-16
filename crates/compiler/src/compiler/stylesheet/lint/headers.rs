//! Comparable forms of the headers of nested style rules and conditional at-rules.
//!
//! `prefer-variant-directive` compares the headers that authored CSS writes with the
//! headers that `@variant` expands to. [`Header`] keeps only the differences that can
//! change the meaning. It removes whitespace that CSS ignores and comments. It
//! lowercases media conditions and pseudo-class names, and it writes `min-` and `max-`
//! media and container features in range syntax. [`header_key`] orders two nested
//! headers when their order does not change the result.

use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_css::{absolutize_nested_selector, selector_list_branches};

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
                Self::Selector(normalize(
                    &absolutize_nested_selector(selector),
                    SELECTOR_SPACING,
                    Case::PseudoClasses,
                ))
            }),
            Level::AtRule(name, prelude) => {
                let prelude = match name {
                    "media" => {
                        let case = if prelude.contains("--") {
                            Case::Keep
                        } else {
                            Case::All
                        };
                        let condition = ranges(&normalize(prelude, CONDITION_SPACING, case));
                        if px_to_rem {
                            px_lengths(&condition)
                        } else {
                            condition
                        }
                    }
                    "container" => ranges(&normalize(prelude, CONDITION_SPACING, Case::Keep)),
                    // `selector()` arguments are selectors, in which a space before `:`
                    // is a descendant combinator.
                    "supports" if prelude.contains("selector(") => {
                        normalize(prelude, SELECTOR_SPACING, Case::Keep)
                    }
                    "supports" => normalize(prelude, CONDITION_SPACING, Case::Keep),
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

/// ASCII case folding that [`normalize`] applies outside strings.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Case {
    /// Keeps the case, because names such as container names are case-sensitive.
    Keep,
    /// Lowercases the identifier after each `:`, which names a pseudo-class or
    /// pseudo-element. Class names and attribute values keep their case.
    PseudoClasses,
    /// Lowercases everything.
    All,
}

/// Returns `text` with comments removed and whitespace collapsed to one space.
///
/// A space is removed at the start and end, after `(` and `[`, before `)` and `]`, and
/// on both sides of each character of `spacing`. Strings and escaped characters stay
/// as written.
fn normalize(text: &str, spacing: &[char], case: Case) -> LeanString {
    let mut output = LeanString::new();
    let mut space = false;
    let mut pseudo = false;
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '/' && characters.peek() == Some(&'*') {
            characters.next();
            let mut previous = '\0';
            for next in characters.by_ref() {
                if previous == '*' && next == '/' {
                    break;
                }
                previous = next;
            }
            continue;
        }
        if character.is_whitespace() {
            space = !output.is_empty();
            continue;
        }
        if space {
            let after_tight = output
                .chars()
                .last()
                .is_some_and(|last| last == '(' || last == '[' || spacing.contains(&last));
            if !after_tight && character != ')' && character != ']' && !spacing.contains(&character)
            {
                output.push(' ');
            }
            space = false;
        }
        match character {
            '\\' => {
                output.push(character);
                output.extend(characters.next());
                pseudo = false;
            }
            '"' | '\'' => {
                output.push(character);
                let mut escaped = false;
                for next in characters.by_ref() {
                    output.push(next);
                    if escaped {
                        escaped = false;
                    } else if next == '\\' {
                        escaped = true;
                    } else if next == character {
                        break;
                    }
                }
                pseudo = false;
            }
            _ => {
                let identifier =
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_');
                pseudo = character == ':' || (pseudo && identifier);
                if case == Case::All || (case == Case::PseudoClasses && pseudo) {
                    output.push(character.to_ascii_lowercase());
                } else {
                    output.push(character);
                }
            }
        }
    }
    output
}

/// Rewrites the features of each innermost parenthesized group of a normalized
/// condition into range syntax with the feature name first.
///
/// `(min-width:40rem)` becomes `(width>=40rem)` and `(max-width:40rem)` becomes
/// `(width<=40rem)`, as Media Queries Level 4 defines them. `(40rem<=width)` becomes
/// `(width>=40rem)`. Other groups stay as written.
fn ranges(condition: &str) -> LeanString {
    let mut output = LeanString::new();
    let mut rest = condition;
    while let Some(open) = rest.find('(') {
        output.push_str(&rest[..=open]);
        rest = &rest[open + 1..];
        let end = rest.find(['(', ')']).unwrap_or(rest.len());
        if rest[end..].starts_with(')') {
            output.push_str(&range_feature(&rest[..end]));
            rest = &rest[end..];
        }
    }
    output.push_str(rest);
    output
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
    let mut output = LeanString::new();
    let mut rest = condition;
    while let Some(start) =
        rest.find(|character: char| character.is_ascii_digit() || character == '.')
    {
        let identifier = rest[..start]
            .chars()
            .last()
            .is_some_and(|last| last.is_ascii_alphanumeric() || last == '_');
        output.push_str(&rest[..start]);
        rest = &rest[start..];
        let number_end = rest
            .find(|character: char| !character.is_ascii_digit() && character != '.')
            .unwrap_or(rest.len());
        let unit_end = rest[number_end..]
            .find(|character: char| !character.is_ascii_alphabetic())
            .map_or(rest.len(), |end| number_end + end);
        let scale = match &rest[number_end..unit_end] {
            "px" => Some(1.0),
            "rem" => Some(16.0),
            _ => None,
        };
        match (identifier, scale, rest[..number_end].parse::<f64>()) {
            (false, Some(scale), Ok(number)) => {
                output.push_str(&format_args!("{}px", number * scale).to_lean_string());
            }
            _ => output.push_str(&rest[..unit_end]),
        }
        rest = &rest[unit_end..];
    }
    output.push_str(rest);
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
