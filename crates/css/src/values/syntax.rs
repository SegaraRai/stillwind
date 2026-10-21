//! Focused value-shape checks for grammars Lightning CSS cannot type.
//!
//! Each helper answers one narrow question - is this a gradient argument list, a
//! `content` value, a transform function body, a `var()` reference - and they exist so
//! that a value type without a typed parser is still checked by something stricter than
//! "looks plausible". Some helpers still use lexical shape checks, so acceptance is not
//! proof that a browser will accept the whole value.

use cssparser::{ParseError, Parser, Token};
use lean_string::ToLeanString;
use lightningcss::properties::{Property, PropertyId};
use lightningcss::stylesheet::ParserOptions;
use lightningcss::traits::Parse;
use lightningcss::values::angle::AnglePercentage;
use lightningcss::values::image::Image;
use lightningcss::values::length::LengthPercentage;
use lightningcss::values::number::CSSNumber;
use lightningcss::values::time::Time;

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

/// Math functions whose arguments [`value_is_well_formed`] parses.
const MATH_FUNCTIONS: &[&str] = &[
    "calc", "min", "max", "clamp", "round", "mod", "rem", "abs", "sign", "sin", "cos", "tan",
    "asin", "acos", "atan", "atan2", "pow", "sqrt", "hypot", "log", "exp",
];

/// Whether `value` is a nonempty sequence of CSS component values whose math functions
/// parse, regardless of the value's type.
///
/// `1/2` and `12px foo` are well formed. A value is rejected when it is blank, contains a
/// bad string or URL token, or closes a block that it did not open. It is also rejected
/// when a math function such as `calc()` does not parse as a number, length, percentage,
/// angle, or time calculation, as in `calc(1rem + )`. A math function that contains
/// `var()` is accepted, because its type is known only after substitution.
pub fn value_is_well_formed(value: &str) -> bool {
    !value.trim().is_empty() && component_values_are_well_formed(&mut Parser::new(value), value)
}

/// Checks the component values that `parser` reads from `source`, including nested
/// blocks.
fn component_values_are_well_formed(parser: &mut Parser<'_>, source: &str) -> bool {
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            return true;
        };
        match token {
            Token::BadString(_)
            | Token::BadUrl(_)
            | Token::CloseParenthesis
            | Token::CloseSquareBracket
            | Token::CloseCurlyBracket => {
                return false;
            }
            Token::Function(name) => {
                let math = MATH_FUNCTIONS
                    .iter()
                    .any(|function| name.eq_ignore_ascii_case(function));
                let mut nested = true;
                let result: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                    nested = component_values_are_well_formed(inner, source);
                    while inner.next_including_whitespace_and_comments().is_ok() {}
                    Ok(())
                });
                let end = parser.position().byte_index();
                if !nested || result.is_err() {
                    return false;
                }
                if math && !math_function_parses(&source[start..end]) {
                    return false;
                }
            }
            Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                let mut nested = true;
                let result: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                    nested = component_values_are_well_formed(inner, source);
                    while inner.next_including_whitespace_and_comments().is_ok() {}
                    Ok(())
                });
                if !nested || result.is_err() {
                    return false;
                }
            }
            _ => {}
        }
    }
}

/// Whether one complete math function, such as `calc(1rem + 2px)`, parses as a
/// calculation of a number, length, percentage, angle, or time.
fn math_function_parses(function: &str) -> bool {
    function.contains("var(")
        || CSSNumber::parse_string(function).is_ok()
        || LengthPercentage::parse_string(function).is_ok()
        || AnglePercentage::parse_string(function).is_ok()
        || Time::parse_string(function).is_ok()
}

/// True when `value` parses as the argument list of the gradient `function`.
///
/// The text is re-wrapped as `function(value)` and handed to Lightning CSS, so this
/// accepts exactly what a gradient parser accepts there.
pub(super) fn gradient_arguments(function: &str, value: &str) -> bool {
    Image::parse_string(&format_args!("{function}({value})").to_lean_string()).is_ok()
}

/// True when `value` followed by two more color stops parses as the argument list of
/// the gradient `function`.
///
/// This accepts a gradient line, shape, or position without color stops, such as
/// `to right` or `at center`, and it also accepts a complete argument list.
pub(super) fn gradient_position(function: &str, value: &str) -> bool {
    !value.trim().is_empty()
        && gradient_arguments(
            function,
            &format_args!("{value}, red, blue").to_lean_string(),
        )
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
    let value = format_args!("{function}({value})").to_lean_string();
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

    #[test]
    fn well_formed_values_have_any_type_but_parsable_math() {
        for value in [
            "1/2",
            "red",
            "12px foo",
            "calc(1rem + 2px)",
            "f(calc(var(--x) + ))",
        ] {
            assert!(value_is_well_formed(value), "{value}");
        }
        for value in [
            "",
            " ",
            "calc(1rem + )",
            "f(min(1px, ))",
            "a)",
            "\"a
b\"",
        ] {
            assert!(!value_is_well_formed(value), "{value}");
        }
    }
}
