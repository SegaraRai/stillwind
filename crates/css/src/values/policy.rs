//! CSS value inspection for property and unit policies.
//!
//! `property_value_domains` expands parsed shorthands without inventing values for
//! unresolved variables. Lexical inspection skips strings and URLs and preserves
//! nonzero dimensions inside mathematical functions.

use std::collections::{BTreeMap, BTreeSet};

use cssparser::{ParseError, Parser, Token};
use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};
use lightningcss::properties::{Property, PropertyId};
use lightningcss::stylesheet::{ParserOptions, PrinterOptions};
use lightningcss::traits::Parse;
use lightningcss::values::color::CssColor;

use super::webref::{is_logical, physical_group_longhands, webref_property};

/// Matches an entire string using `*` for any sequence and `?` for one character.
/// Matching is case-sensitive; callers normalize CSS property and unit names.
pub fn policy_glob_matches(pattern: &str, value: &str) -> bool {
    let pattern: Vec<_> = pattern.chars().collect();
    let value: Vec<_> = value.chars().collect();
    let (mut p, mut v, mut star, mut retry) = (0, 0, None, 0);
    while v < value.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == value[v]) {
            p += 1;
            v += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            p += 1;
            retry = v;
        } else if let Some(index) = star {
            retry += 1;
            v = retry;
            p = index + 1;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}

/// Returns each longhand that the declaration `property: value` sets, with its value.
/// For example, `margin: 1px 2px` gives four longhands. A nested shorthand is expanded
/// too. Property names are lowercase.
///
/// An unknown property, or a value that contains a variable and cannot be split,
/// returns one entry with `property` and `value`. A trailing `!important` is removed
/// from `value`.
pub fn property_value_domains(property: &str, value: &str) -> Vec<(LeanString, LeanString)> {
    let value = value
        .rsplit_once('!')
        .filter(|(_, suffix)| suffix.trim().eq_ignore_ascii_case("important"))
        .map_or(value, |(value, _)| value)
        .trim();
    let mut result = vec![(
        property.to_lean_string().to_ascii_lowercase(),
        value.to_lean_string(),
    )];
    let id = PropertyId::from(property);
    if id.longhands().is_none() {
        return result;
    }
    let Ok(parsed) = Property::parse_string(id, value, ParserOptions::default()) else {
        return result;
    };
    let mut pending = vec![parsed];
    while let Some(parsed) = pending.pop() {
        if let Some(longhands) = parsed.property_id().longhands() {
            for id in longhands {
                if let Some(longhand) = parsed.longhand(&id) {
                    if let Ok(css) = longhand.to_css_string(false, PrinterOptions::default())
                        && let Some((name, value)) = css.split_once(':')
                    {
                        result.push((name.trim().to_lean_string(), value.trim().to_lean_string()));
                    }
                    pending.push(longhand);
                }
            }
        }
    }
    result
}

/// Returns nonzero length unit names and direct `var()` / `--theme()` references.
/// References in fallback arguments are included; their declaration values are not followed.
pub fn inspect_css_value(value: &str) -> (BTreeSet<LeanStr>, BTreeSet<LeanStr>) {
    let mut units = BTreeSet::new();
    let mut references = BTreeSet::new();
    inspect(&mut Parser::new(value), &mut units, &mut references);
    (units, references)
}

/// Returns whether explicit CSS text contains a literal color or nonzero spacing measure.
/// Strings and URL arguments are skipped. Variable fallbacks and mathematical expressions
/// are inspected; referenced custom-property definitions are left to the caller.
pub fn arbitrary_value_flags(property: &str, value: &str, color_domain: bool) -> (bool, bool) {
    let property = property.to_lean_string().to_ascii_lowercase();
    let color = color_domain
        || property.contains("color")
        || matches!(
            property.as_str(),
            "backdrop-filter"
                | "-webkit-backdrop-filter"
                | "background"
                | "background-image"
                | "border"
                | "border-top"
                | "border-right"
                | "border-bottom"
                | "border-left"
                | "border-block"
                | "border-inline"
                | "border-image"
                | "border-image-source"
                | "outline"
                | "box-shadow"
                | "text-shadow"
                | "filter"
                | "mask"
                | "mask-image"
                | "fill"
                | "stroke"
        );
    let mut flags = (false, false);
    inspect_arbitrary(&mut Parser::new(value), color, &mut flags);
    flags
}

fn inspect_arbitrary(parser: &mut Parser<'_>, color: bool, flags: &mut (bool, bool)) {
    while let Ok(token) = parser.next().cloned() {
        match token {
            Token::Hash(_) | Token::IDHash(_) if color => {
                flags.0 = true;
            }
            Token::Ident(name) if color => {
                if !name.eq_ignore_ascii_case("transparent")
                    && !name.eq_ignore_ascii_case("currentcolor")
                    && CssColor::parse_string(&name).is_ok()
                {
                    flags.0 = true;
                }
            }
            Token::Dimension { value, unit, .. } if value != 0.0 && is_length_unit(&unit) => {
                flags.1 = true;
            }
            Token::Percentage { unit_value, .. } if unit_value != 0.0 => {
                flags.1 = true;
            }
            Token::Function(name) => {
                let start = parser.position();
                let skip = ["url", "style", "media", "supports"]
                    .iter()
                    .any(|skip| name.eq_ignore_ascii_case(skip));
                let color_function = color
                    && matches!(
                        name.to_lean_string().to_ascii_lowercase().as_str(),
                        "rgb"
                            | "rgba"
                            | "hsl"
                            | "hsla"
                            | "hwb"
                            | "lab"
                            | "lch"
                            | "oklab"
                            | "oklch"
                            | "color"
                            | "device-cmyk"
                    );
                let _ = parser.parse_nested_block(|inner| {
                    if !skip {
                        inspect_arbitrary(inner, color, flags);
                    }
                    Ok::<_, ParseError<()>>(())
                });
                if color_function && inspect_css_value(parser.slice_from(start)).1.is_empty() {
                    flags.0 = true;
                }
            }
            Token::ParenthesisBlock => {
                let _ = parser.parse_nested_block(|inner| {
                    inspect_arbitrary(inner, color, flags);
                    Ok::<_, ParseError<()>>(())
                });
            }
            _ => {}
        }
    }
}

/// Whether a media condition compares a viewport width or height with a value.
/// Parentheses are inspected recursively; strings, functions, and media type names
/// do not count as features. Boolean features without a comparison are excluded.
pub fn media_has_size_comparison(condition: &str) -> bool {
    !media_size_comparisons(condition).is_empty()
}

/// Returns parenthesized viewport size comparisons from a media prelude.
/// Logical groups are traversed; unrelated feature values and function arguments
/// do not become size comparisons.
pub fn media_size_comparisons(condition: &str) -> Vec<LeanStr> {
    let mut comparisons = Vec::new();
    size_comparisons(&mut Parser::new(condition), false, &mut comparisons);
    comparisons
}

fn size_comparisons(parser: &mut Parser<'_>, feature_block: bool, comparisons: &mut Vec<LeanStr>) {
    let start = parser.position();
    let (mut size, mut comparison) = (false, false);
    while let Ok(token) = parser.next() {
        match token {
            Token::Ident(name) if feature_block => {
                size |= [
                    "width",
                    "height",
                    "min-width",
                    "max-width",
                    "min-height",
                    "max-height",
                ]
                .iter()
                .any(|feature| name.eq_ignore_ascii_case(feature));
            }
            Token::Colon | Token::Delim('<' | '>' | '=') if feature_block => {
                comparison = true;
            }
            Token::ParenthesisBlock => {
                let _ = parser.parse_nested_block(|inner| {
                    size_comparisons(inner, true, comparisons);
                    Ok::<_, ParseError<()>>(())
                });
            }
            _ => {}
        }
    }
    if size && comparison {
        comparisons.push(format_args!("({})", parser.slice_from(start)).to_lean_str());
    }
}

fn inspect(
    parser: &mut Parser<'_>,
    units: &mut BTreeSet<LeanStr>,
    references: &mut BTreeSet<LeanStr>,
) {
    while let Ok(token) = parser.next() {
        match token {
            Token::Dimension { value, unit, .. } if *value != 0.0 && is_length_unit(unit) => {
                units.insert((unit.to_lean_string().to_ascii_lowercase()).into());
            }
            Token::Function(name) => {
                let reference =
                    name.eq_ignore_ascii_case("var") || name.eq_ignore_ascii_case("--theme");
                let skip = name.eq_ignore_ascii_case("url");
                let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                    if reference
                        && let Ok(name) =
                            inner.try_parse(|p| p.expect_ident().map(ToLeanString::to_lean_string))
                        && name.starts_with("--")
                    {
                        references.insert((name).into());
                    }
                    if !skip {
                        inspect(inner, units, references);
                    }
                    Ok(())
                });
            }
            Token::ParenthesisBlock => {
                let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                    inspect(inner, units, references);
                    Ok(())
                });
            }
            _ => {}
        }
    }
}

/// Returns whether a unit name denotes a CSS length, without ASCII case.
pub fn is_length_unit(unit: &str) -> bool {
    matches!(
        unit.to_lean_string().to_ascii_lowercase().as_str(),
        "px" | "cm"
            | "mm"
            | "q"
            | "in"
            | "pt"
            | "pc"
            | "em"
            | "rem"
            | "ex"
            | "rex"
            | "cap"
            | "rcap"
            | "ch"
            | "rch"
            | "ic"
            | "ric"
            | "lh"
            | "rlh"
            | "vw"
            | "vh"
            | "vi"
            | "vb"
            | "vmin"
            | "vmax"
            | "svw"
            | "svh"
            | "svi"
            | "svb"
            | "svmin"
            | "svmax"
            | "lvw"
            | "lvh"
            | "lvi"
            | "lvb"
            | "lvmin"
            | "lvmax"
            | "dvw"
            | "dvh"
            | "dvi"
            | "dvb"
            | "dvmin"
            | "dvmax"
            | "cqw"
            | "cqh"
            | "cqi"
            | "cqb"
            | "cqmin"
            | "cqmax"
    )
}

/// Returns whether a declaration uses physical directions that have logical
/// counterparts. A shorthand is exempt when it gives every physical longhand of a
/// logical group the same value, as in `padding: 1rem` and `overflow: hidden`, because
/// the result does not depend on writing mode.
pub fn declaration_is_physical(property: &str, value: &str) -> bool {
    let value = value
        .rsplit_once('!')
        .filter(|(_, suffix)| suffix.trim().eq_ignore_ascii_case("important"))
        .map_or(value, |(value, _)| value)
        .trim();
    if matches!(property, "text-align" | "float" | "clear")
        && (value.eq_ignore_ascii_case("left") || value.eq_ignore_ascii_case("right"))
    {
        return true;
    }
    let Some(definition) = webref_property(property) else {
        return false;
    };
    if is_logical(property) {
        return false;
    }
    if definition.longhands.is_empty() {
        return definition.logical_group.is_some();
    }
    let domains = property_value_domains(property, value);
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    for (name, value) in &domains {
        if let Some(p) = webref_property(name)
            && p.longhands.is_empty()
            && !is_logical(name)
            && let Some(group) = p.logical_group
        {
            groups.entry(group).or_default().push(value);
        }
    }
    groups.iter().any(|(group, values)| {
        values.len() != physical_group_longhands(group).len()
            || values.iter().any(|value| *value != values[0])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_size_comparisons_skip_other_features_and_function_contents() {
        for condition in [
            "(width >= 900px)",
            "(40rem < width < 80rem)",
            "not ((min-height: 10px) or (width: calc(1px + 2rem)))",
            "(WIDTH: 20rem)",
        ] {
            assert!(media_has_size_comparison(condition), "{condition}");
        }
        for condition in [
            "width",
            "(width)",
            "(orientation:portrait)",
            "(aspect-ratio:16/9)",
            "(foo: 'width>10px')",
            "style(width>10px)",
        ] {
            assert!(!media_has_size_comparison(condition), "{condition}");
        }
        assert_eq!(
            media_size_comparisons("print and (width >= 900px) and (color:8)"),
            ["(width >= 900px)"]
        );
    }

    #[test]
    fn dimensions_skip_literals_and_keep_math_and_variable_fallbacks() {
        let (units, refs) =
            inspect_css_value("calc(1rem + var(--size, 2px)) url('3em') '4cm' 0px 2s");
        assert_eq!(units, BTreeSet::from(["rem".into(), "px".into()]));
        assert_eq!(refs, BTreeSet::from(["--size".into()]));
    }

    #[test]
    fn arbitrary_values_skip_query_constants_but_inspect_branch_values() {
        assert_eq!(
            arbitrary_value_flags(
                "padding",
                "if(style(--mode: 10px): var(--space); else: 0px)",
                false
            ),
            (false, false)
        );
        assert_eq!(
            arbitrary_value_flags("padding", "if(media(width > 900px): 2rem; else: 0)", false),
            (false, true)
        );
        assert_eq!(
            arbitrary_value_flags(
                "color",
                "if(style(--mode: red): var(--fg); else: currentColor)",
                true
            ),
            (false, false)
        );
        assert_eq!(
            arbitrary_value_flags(
                "color",
                "if(supports(color: red): #fff; else: var(--fg))",
                true
            ),
            (true, false)
        );
    }

    #[test]
    fn arbitrary_colors_follow_mask_and_filter_values() {
        for (property, value, expected) in [
            (
                "mask",
                "linear-gradient(red, transparent) no-repeat",
                (true, false),
            ),
            ("mask-image", "var(--paint, red)", (true, false)),
            ("filter", "drop-shadow(0 0 2px #abc)", (true, true)),
            ("backdrop-filter", "drop-shadow(0 0 2px red)", (true, true)),
            (
                "-webkit-backdrop-filter",
                "drop-shadow(0 0 2px red)",
                (true, true),
            ),
            ("filter", "blur(2px)", (false, true)),
            ("mask-image", "url('red.svg#fff')", (false, false)),
            ("mask-image", "var(--paint, currentColor)", (false, false)),
        ] {
            assert_eq!(
                arbitrary_value_flags(property, value, false),
                expected,
                "{property}: {value}"
            );
        }
    }

    #[test]
    fn shorthand_values_keep_their_property_domains() {
        let domains = property_value_domains("border", "1px solid red");
        assert!(domains.contains(&("border-top-width".into(), "1px".into())));
        assert!(!declaration_is_physical("padding", "2rem"));
        assert!(declaration_is_physical("padding", "1rem 2rem"));
        assert!(!declaration_is_physical("padding-inline", "2rem"));
        assert!(declaration_is_physical("text-align", "left"));
        assert!(declaration_is_physical("text-align", "LEFT!important"));
        assert!(declaration_is_physical("border-top", "1px solid red"));
        assert!(!declaration_is_physical("border", "1px solid red"));
        assert!(!declaration_is_physical("overflow", "hidden"));
        assert!(declaration_is_physical("overflow", "hidden auto"));
        assert!(declaration_is_physical("overflow-x", "hidden"));
    }
}
