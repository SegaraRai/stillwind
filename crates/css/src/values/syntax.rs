use lightningcss::properties::{Property, PropertyId};
use lightningcss::stylesheet::ParserOptions;
use lightningcss::traits::Parse;
use lightningcss::values::image::Image;

use crate::colors::is_css_string;

use super::properties::CssProperty;

pub(crate) fn contains_custom_property_reference(value: &str) -> bool {
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

pub(crate) fn gradient_arguments(function: &str, value: &str) -> bool {
    Image::parse_string(&format!("{function}({value})")).is_ok()
}

pub(crate) fn content_value(value: &str) -> bool {
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

pub(crate) fn transform_function_arguments(function: &str, value: &str) -> bool {
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

pub(crate) fn property_value(property: &str, value: &str) -> bool {
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
