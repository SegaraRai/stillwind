use lightningcss::properties::{Property, PropertyId};
use lightningcss::stylesheet::ParserOptions;
use lightningcss::traits::Parse;
use lightningcss::values::length::Length;
use lightningcss::values::percentage::NumberOrPercentage;
use lightningcss::values::position::Position;

use super::domain::is_css_wide_keyword;
use super::webref::webref_property;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CssPropertyGrammar {
    LightningCss,
    Columns,
    Contain,
    FontFeatureSettings,
    ObjectPosition,
    WillChange,
    Zoom,
    AnchorName,
    PositionAnchor,
    AnchorScope,
    PositionArea,
    PositionTryFallbacks,
}

pub(crate) const PROPERTY_GRAMMAR_EXCEPTIONS: &[(&str, CssPropertyGrammar)] = &[
    // These established properties are not represented by the current
    // Lightning CSS property table. Keep their grammar exceptions explicit
    // and covered by acceptance/rejection tests.
    ("contain", CssPropertyGrammar::Contain),
    ("columns", CssPropertyGrammar::Columns),
    (
        "font-feature-settings",
        CssPropertyGrammar::FontFeatureSettings,
    ),
    ("object-position", CssPropertyGrammar::ObjectPosition),
    ("will-change", CssPropertyGrammar::WillChange),
    ("zoom", CssPropertyGrammar::Zoom),
    // CSS Anchor Positioning is intentionally kept here until Lightning CSS
    // exposes property parsers for it. Each exception has a narrow validator
    // below and a boundary test; do not add an unchecked catch-all.
    ("anchor-name", CssPropertyGrammar::AnchorName),
    ("position-anchor", CssPropertyGrammar::PositionAnchor),
    ("anchor-scope", CssPropertyGrammar::AnchorScope),
    ("position-area", CssPropertyGrammar::PositionArea),
    (
        "position-try-fallbacks",
        CssPropertyGrammar::PositionTryFallbacks,
    ),
];

/// A CSSWG-defined property whose value grammar is delegated to Lightning CSS
/// where available, with narrow temporary exceptions where it is not.
///
/// The supported set is generated from W3C Webref's curated CSSWG data,
/// rather than maintained as a partial list. It is an internal validator for
/// named CSS value domains; property names are never part of the public
/// `--value([type])` grammar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssProperty {
    name: String,
    grammar: CssPropertyGrammar,
}

impl CssProperty {
    /// Finds a CSSWG-defined property for an internal value-domain validator.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let webref = webref_property(name)?;
        let property_id = PropertyId::from(name);
        if !matches!(property_id, PropertyId::Custom(_)) {
            return Some(Self {
                name: webref.name.to_owned(),
                grammar: CssPropertyGrammar::LightningCss,
            });
        }
        PROPERTY_GRAMMAR_EXCEPTIONS
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(name, grammar)| Self {
                name: (*name).to_owned(),
                grammar: *grammar,
            })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Validates the value through Lightning CSS's parser for this exact CSS
    /// property. This intentionally covers property-specific keyword and
    /// shorthand rules that a generic value type cannot express.
    #[must_use]
    pub fn accepts(&self, value: &str) -> bool {
        if is_css_wide_keyword(value) {
            return true;
        }
        match self.grammar {
            CssPropertyGrammar::LightningCss => matches!(
                Property::parse_string(
                    PropertyId::from(self.name()),
                    value.trim(),
                    ParserOptions::default(),
                ),
                Ok(property) if !matches!(property, Property::Unparsed(_) | Property::Custom(_))
            ),
            CssPropertyGrammar::Columns => columns_value(value),
            CssPropertyGrammar::Contain => contain_value(value),
            CssPropertyGrammar::FontFeatureSettings => font_feature_settings(value),
            CssPropertyGrammar::ObjectPosition => Position::parse_string(value).is_ok(),
            CssPropertyGrammar::WillChange => will_change(value),
            CssPropertyGrammar::Zoom => {
                matches!(value.trim(), "normal" | "reset")
                    || NumberOrPercentage::parse_string(value).is_ok()
            }
            CssPropertyGrammar::AnchorName => dashed_ident_list(value, "none"),
            CssPropertyGrammar::PositionAnchor => {
                value.trim() == "auto" || dashed_ident(value.trim())
            }
            CssPropertyGrammar::AnchorScope => {
                matches!(value.trim(), "none" | "all") || dashed_ident_list(value, "")
            }
            CssPropertyGrammar::PositionArea => position_area(value),
            CssPropertyGrammar::PositionTryFallbacks => position_try_fallbacks(value),
        }
    }
}

pub(crate) fn columns_value(value: &str) -> bool {
    let values = value.split_whitespace().collect::<Vec<_>>();
    if values.is_empty() || values.len() > 2 {
        return false;
    }
    let mut width = false;
    let mut count = false;
    for value in values {
        if !width && (value == "auto" || Length::parse_string(value).is_ok()) {
            width = true;
        } else if !count && (value == "auto" || value.parse::<u32>().is_ok_and(|value| value > 0)) {
            count = true;
        } else {
            return false;
        }
    }
    width || count
}

pub(crate) fn contain_value(value: &str) -> bool {
    const SINGLE: &[&str] = &["none", "strict", "content"];
    const COMPONENTS: &[&str] = &["size", "inline-size", "layout", "style", "paint"];
    let values = value.split_whitespace().collect::<Vec<_>>();
    SINGLE.contains(&value.trim())
        || (!values.is_empty()
            && values.len() <= COMPONENTS.len()
            && values.iter().all(|value| COMPONENTS.contains(value))
            && values.windows(2).all(|pair| pair[0] != pair[1]))
}

pub(crate) fn font_feature_settings(value: &str) -> bool {
    let value = value.trim();
    value == "normal"
        || value.split(',').all(|feature| {
            let mut parts = feature.split_whitespace();
            let Some(tag) = parts.next() else {
                return false;
            };
            let valid_tag = tag
                .strip_prefix('"')
                .and_then(|tag| tag.strip_suffix('"'))
                .is_some_and(|tag| tag.len() == 4 && tag.is_ascii());
            let valid_value = match parts.next() {
                None | Some("on" | "off") => true,
                Some(number) => number.parse::<i32>().is_ok(),
            };
            valid_tag && valid_value && parts.next().is_none()
        })
}

pub(crate) fn will_change(value: &str) -> bool {
    let value = value.trim();
    value == "auto"
        || value.split(',').all(|item| {
            let item = item.trim();
            matches!(item, "scroll-position" | "contents") || custom_ident(item)
        })
}

pub(crate) fn custom_ident(value: &str) -> bool {
    value
        .chars()
        .next()
        .is_some_and(|character| !character.is_ascii_digit())
        && !matches!(
            value,
            "initial" | "inherit" | "unset" | "revert" | "revert-layer"
        )
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_')
                || !character.is_ascii()
        })
}

pub(crate) fn dashed_ident_list(value: &str, keyword: &str) -> bool {
    let value = value.trim();
    value == keyword || (!value.is_empty() && value.split_whitespace().all(dashed_ident))
}

pub(crate) fn dashed_ident(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("--") else {
        return false;
    };
    !rest.is_empty()
        && rest.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_')
                || !character.is_ascii()
        })
}

pub(crate) fn position_area(value: &str) -> bool {
    const HORIZONTAL: &[&str] = &[
        "left",
        "center",
        "right",
        "span-left",
        "span-right",
        "x-start",
        "x-end",
        "span-x-start",
        "span-x-end",
        "self-x-start",
        "self-x-end",
        "span-self-x-start",
        "span-self-x-end",
        "span-all",
        "inline-start",
        "inline-end",
        "span-inline-start",
        "span-inline-end",
        "self-inline-start",
        "self-inline-end",
        "span-self-inline-start",
        "span-self-inline-end",
    ];
    const VERTICAL: &[&str] = &[
        "top",
        "center",
        "bottom",
        "span-top",
        "span-bottom",
        "y-start",
        "y-end",
        "span-y-start",
        "span-y-end",
        "self-y-start",
        "self-y-end",
        "span-self-y-start",
        "span-self-y-end",
        "span-all",
        "block-start",
        "block-end",
        "span-block-start",
        "span-block-end",
        "self-block-start",
        "self-block-end",
        "span-self-block-start",
        "span-self-block-end",
    ];

    let values = value.split_whitespace().collect::<Vec<_>>();
    matches!(values.as_slice(), [single] if HORIZONTAL.contains(single) || VERTICAL.contains(single))
        || matches!(values.as_slice(), [first, second] if {
            (HORIZONTAL.contains(first) && VERTICAL.contains(second))
                || (VERTICAL.contains(first) && HORIZONTAL.contains(second))
        })
}

pub(crate) fn position_try_fallbacks(value: &str) -> bool {
    const TACTICS: &[&str] = &["flip-block", "flip-inline", "flip-start", "flip-end"];
    value.split(',').all(|item| {
        let item = item.trim();
        !item.is_empty() && (dashed_ident(item) || TACTICS.contains(&item))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_anchor_positioning_with_explicit_property_exceptions() {
        let domain = |name| CssProperty::from_name(name).unwrap();

        assert!(domain("anchor-name").accepts("--card --tooltip"));
        assert!(!domain("anchor-name").accepts("card"));
        assert!(domain("position-anchor").accepts("--card"));
        assert!(domain("position-anchor").accepts("auto"));
        assert!(!domain("position-anchor").accepts("card"));
        assert!(domain("anchor-scope").accepts("all"));
        assert!(domain("anchor-scope").accepts("--dialog"));
        assert!(!domain("anchor-scope").accepts("dialog"));
        assert!(domain("position-area").accepts("top span-inline-end"));
        assert!(!domain("position-area").accepts("middle middle"));
        assert!(domain("position-try-fallbacks").accepts("--fallback, flip-block"));
        assert!(!domain("position-try-fallbacks").accepts("fallback"));
    }

    #[test]
    fn validates_csswg_property_grammars_with_lightningcss_or_exceptions() {
        for (name, valid, invalid) in [
            ("padding", "calc(1rem + 50%)", "red"),
            ("margin", "auto", "red"),
            ("border-width", "thin", "50%"),
            ("transition-duration", "150ms", "red"),
            ("aspect-ratio", "16 / 9", "red"),
            ("opacity", ".5", "red"),
            // This property was deliberately absent from the old hand-written
            // list. Its acceptance proves the domain comes from Lightning
            // CSS's generated property registry, not a fixture-derived list.
            ("text-decoration-line", "underline overline", "12px"),
            ("view-transition-name", "hero", "12px"),
            ("contain", "layout paint", "middle"),
            ("font-feature-settings", "\"liga\" 1", "liga"),
            ("object-position", "left 25%", "middle middle"),
            ("will-change", "transform, opacity", "1px"),
            ("zoom", "125%", "red"),
        ] {
            let property = CssProperty::from_name(name).unwrap();
            assert!(property.accepts(valid), "{name}: {valid}");
            assert!(!property.accepts(invalid), "{name}: {invalid}");
        }

        assert!(webref_property("padding").is_some());
        assert!(webref_property("grid-template-columns").is_some());
        assert!(webref_property("view-transition-name").is_some());
        assert!(webref_property("not-a-css-property").is_none());
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn property_grammar_helpers_reject_extra_tokens_and_reserved_identifiers() {
        for (valid, invalid, check) in [
            ("2 auto", "2 3 4", columns_value as fn(&str) -> bool),
            ("layout paint", "layout layout", contain_value),
            ("\"liga\" on", "\"liga\" on extra", font_feature_settings),
            ("transform, opacity", "1thing", will_change),
            ("hello-world", "inherit", custom_ident),
            ("--x", "--", dashed_ident),
            ("left top", "left right", position_area),
        ] {
            assert!(check(valid), "{valid}");
            assert!(!check(invalid), "{invalid}");
        }
        assert!(dashed_ident_list("--one --two", "none"));
        assert!(!dashed_ident_list("--one two", "none"));
        assert!(position_try_fallbacks("--fallback"));
        assert!(!position_try_fallbacks(""));
        assert!(CssProperty::from_name("unknown-property-xyz").is_none());
    }
}
