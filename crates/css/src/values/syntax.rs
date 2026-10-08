//! Focused value-shape checks for grammars Lightning CSS cannot type.
//!
//! Each helper answers one narrow question - is this a gradient argument list, a
//! `content` value, a transform function body, a `var()` reference - and they exist so
//! that a value type without a typed parser is still checked by something stricter than
//! "looks plausible". Some helpers still use lexical shape checks, so acceptance is not
//! proof that a browser will accept the whole value.

use lightningcss::properties::{Property, PropertyId};
use lightningcss::stylesheet::ParserOptions;
use lightningcss::traits::Parse;
use lightningcss::values::image::Image;

use super::properties::CssProperty;
use crate::colors::is_css_string;

/// True when the text references a custom property, i.e. contains `var(--…)` with only
/// whitespace between the parenthesis and the `--`.
///
/// This is a lexical check, not a parse: `var(x)` and bare `--name` are not references,
/// and the surrounding grammar is the caller's responsibility.
pub(super) fn contains_custom_property_reference(value: &str) -> bool {
    let mut cursor = value;
    while let Some(start) = cursor.find("var(") {
        let after_function = &cursor[start + "var(".len()..];
        let name = after_function.trim_start();
        if name.starts_with("--") {
            return true;
        }
        cursor = after_function;
    }
    false
}

/// True when `value` parses as the argument list of the gradient `function`.
///
/// The text is re-wrapped as `function(value)` and handed to Lightning CSS, so this
/// accepts exactly what a gradient parser accepts there.
pub(super) fn gradient_arguments(function: &str, value: &str) -> bool {
    Image::parse_string(&format!("{function}({value})")).is_ok()
}

/// True for the primitive `content` productions Stillwind supports: the keywords, a CSS
/// string, `attr()`, `counter()`, `counters()`, `leader()`, `element()`, or an image.
///
/// Lightning CSS has no typed `content` parser, so bare identifiers are rejected here
/// rather than accepted unchecked.
pub(super) fn content_value(value: &str) -> bool {
    let value = value.trim();
    matches!(
        value,
        "normal" | "none" | "open-quote" | "close-quote" | "no-open-quote" | "no-close-quote"
    ) || is_css_string(value)
        || ["attr(", "counter(", "counters(", "leader(", "element("]
            .iter()
            .any(|prefix| {
                value
                    .strip_prefix(prefix)
                    .is_some_and(|value| value.ends_with(')'))
            })
        || Image::parse_string(value).is_ok()
}

/// True when `value` is a valid argument list for the transform function `function`.
///
/// The check goes through the `transform` property parser, so a value that parser
/// leaves unparsed counts as a rejection.
pub(super) fn transform_function_arguments(function: &str, value: &str) -> bool {
    let value = format!("{function}({value})");
    matches!(
        Property::parse_string(
            PropertyId::from("transform"),
            &value,
            ParserOptions::default(),
        ),
        Ok(property) if !matches!(property, Property::Unparsed(_) | Property::Custom(_))
    )
}

/// True when `value` is valid for the named CSS property.
///
/// Unknown properties are rejected: the name must be in the Webref snapshot or in the
/// explicit exception table.
pub(super) fn property_value(property: &str, value: &str) -> bool {
    CssProperty::from_name(property).is_some_and(|property| property.accepts(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_shapes_require_valid_css_and_expected_property_grammar() {
        assert!(contains_custom_property_reference("calc(var(--x) * 2)"));
        assert!(!contains_custom_property_reference("var(x)"));
        assert!(gradient_arguments("linear-gradient", "red, blue"));
        assert!(!gradient_arguments("linear-gradient", "red / blue"));
        assert!(content_value("'hello'"));
        assert!(!content_value("hello"));
        assert!(transform_function_arguments("skew", "15deg"));
        assert!(!transform_function_arguments("skew", "red"));
        assert!(property_value("width", "2px"));
        assert!(!property_value("width", "red"));
    }
}
