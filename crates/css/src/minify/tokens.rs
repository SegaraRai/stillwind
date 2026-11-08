//! Optimizations of unparsed token lists, such as custom property values and values that
//! contain `var()`.
//!
//! Lightning CSS keeps these values as tokens and prints the tokens without inserting
//! separators. Each rewrite here keeps the token boundaries that the printed text
//! needs, and it keeps the value that each token produces after `var()` substitution.

use std::fmt::Write;

use cssparser::Parser;
use lightningcss::properties::custom::{Function, Token, TokenOrValue};
use lightningcss::stylesheet::PrinterOptions;
use lightningcss::traits::ToCss;

use lightningcss::values::angle::Angle;
use lightningcss::values::resolution::Resolution;
use lightningcss::values::time::Time;

use crate::math::fold_calc_expressions_dividing_exactly;
use crate::values::MATH_FUNCTIONS;

/// Folds numeric `calc()` calls in `tokens` and then drops whitespace next to commas.
///
/// A `calc()` call becomes a literal only under the conditions of [`literal_for_calc`],
/// which receives `fraction_allowed`. Nested token lists are left to the caller.
pub(super) fn optimize_token_list(tokens: &mut Vec<TokenOrValue<'_>>, fraction_allowed: bool) {
    for token in tokens.iter_mut() {
        if let TokenOrValue::Function(function) = token
            && let Some(literal) = literal_for_calc(function, fraction_allowed)
        {
            *token = literal;
        }
    }
    remove_separator_whitespace(tokens, false);
}

/// Drops whitespace next to `*` and `/` in the arguments of a math function such as
/// `calc()`.
///
/// Outside math functions, whitespace around `*` can be a descendant combinator, as in
/// `.a * .b` inside the block of an unknown at-rule, so other functions keep it.
pub(super) fn optimize_function_arguments(function: &mut Function<'_>) {
    if MATH_FUNCTIONS
        .iter()
        .any(|name| function.name.0.eq_ignore_ascii_case(name))
    {
        remove_separator_whitespace(&mut function.arguments.0, true);
    }
}

/// Returns the literal that a numeric `calc()` call always produces, or `None`.
///
/// The arguments may contain only numbers, percentages, dimensions, parentheses,
/// arithmetic operators, and nested `calc()` calls.
/// [`fold_calc_expressions_dividing_exactly`] must reduce them to one value. It folds a
/// division only by a unitless number and only when the quotient terminates, so
/// `calc(1px / 3)` and `calc(10px / 2px)` stay. Dividing by a dimension needs typed
/// arithmetic, which older browsers reject, and the folded literal would be valid there.
///
/// A unitless value must be an integer unless `fraction_allowed` is true, because an
/// `<integer>` property rounds `calc(1.5)` but rejects `1.5`. The caller allows
/// fractions in the value of a custom property that `@property` registers as a number,
/// whose computed value is already the folded number. Browsers without `@property`
/// substitute the `calc()` call instead, and only an `<integer>` property that reads
/// the value can then differ. A negative value folds although a property such as
/// `width` clamps a negative `calc()` but rejects the same literal: generated values
/// are negative only for properties that accept negative values, such as `translate`.
/// A folded zero keeps the unit of its operands, so `calc(0 * 1px)` becomes `0px` and
/// can still be added to a length after `var()` substitution.
fn literal_for_calc<'i>(
    function: &Function<'_>,
    fraction_allowed: bool,
) -> Option<TokenOrValue<'i>> {
    if !function.name.0.eq_ignore_ascii_case("calc") {
        return None;
    }
    let mut text = String::from("calc(");
    write_math_text(&function.arguments.0, &mut text)?;
    text.push(')');
    let folded = fold_calc_expressions_dividing_exactly(&text);
    let mut parser = Parser::new(&folded);
    let token = match parser.next().ok()? {
        &cssparser::Token::Number {
            value, int_value, ..
        } if fraction_allowed || value.fract() == 0.0 => Token::Number {
            has_sign: false,
            value,
            int_value,
        },
        &cssparser::Token::Percentage {
            unit_value,
            int_value,
            ..
        } => Token::Percentage {
            has_sign: false,
            unit_value,
            int_value,
        },
        cssparser::Token::Dimension {
            value,
            int_value,
            unit,
            ..
        } => Token::Dimension {
            has_sign: false,
            value: *value,
            int_value: *int_value,
            unit: unit.as_ref().to_owned().into(),
        },
        _ => return None,
    };
    parser.is_exhausted().then_some(TokenOrValue::Token(token))
}

/// Appends the text of numeric math arguments to `output`.
///
/// Returns `None` for any token that [`literal_for_calc`] does not accept, such as
/// `var()`. Lengths, angles, times, and resolutions keep their unit even when they are
/// zero.
fn write_math_text(tokens: &[TokenOrValue<'_>], output: &mut String) -> Option<()> {
    for token in tokens {
        match token {
            TokenOrValue::Token(
                token @ (Token::Number { .. } | Token::Percentage { .. } | Token::Dimension { .. }),
            ) => {
                output.push_str(&token.to_css_string(PrinterOptions::default()).ok()?);
            }
            TokenOrValue::Token(Token::WhiteSpace(_)) => {
                output.push(' ');
            }
            TokenOrValue::Token(Token::Delim(operator @ ('+' | '-' | '*' | '/'))) => {
                output.push(*operator);
            }
            TokenOrValue::Token(Token::ParenthesisBlock) => {
                output.push('(');
            }
            TokenOrValue::Token(Token::CloseParenthesis) => {
                output.push(')');
            }
            TokenOrValue::Length(length) => {
                let (value, unit) = length.to_unit_value();
                write!(output, "{value}{unit}").ok()?;
            }
            TokenOrValue::Angle(angle) => {
                let (value, unit) = match angle {
                    Angle::Deg(value) => (value, "deg"),
                    Angle::Rad(value) => (value, "rad"),
                    Angle::Grad(value) => (value, "grad"),
                    Angle::Turn(value) => (value, "turn"),
                };
                write!(output, "{value}{unit}").ok()?;
            }
            TokenOrValue::Time(time) => {
                let (value, unit) = match time {
                    Time::Seconds(value) => (value, "s"),
                    Time::Milliseconds(value) => (value, "ms"),
                };
                write!(output, "{value}{unit}").ok()?;
            }
            TokenOrValue::Resolution(resolution) => {
                let (value, unit) = match resolution {
                    Resolution::Dpi(value) => (value, "dpi"),
                    Resolution::Dpcm(value) => (value, "dpcm"),
                    Resolution::Dppx(value) => (value, "dppx"),
                };
                write!(output, "{value}{unit}").ok()?;
            }
            TokenOrValue::Function(function) if function.name.0.eq_ignore_ascii_case("calc") => {
                output.push_str("calc(");
                write_math_text(&function.arguments.0, output)?;
                output.push(')');
            }
            _ => {
                return None;
            }
        }
    }
    Some(())
}

/// Drops each whitespace token next to a comma, or next to `*` or `/` when `math` is
/// true.
///
/// The token printer writes adjacent tokens without a separator. Whitespace next to `*`
/// or `/` stays when the other neighbor is a delimiter or a comment, because `/ *` would
/// print as the comment opener `/*` and `* =` as the `*=` token.
fn remove_separator_whitespace(tokens: &mut Vec<TokenOrValue<'_>>, math: bool) {
    let mut input = std::mem::take(tokens).into_iter().peekable();
    while let Some(token) = input.next() {
        let removable = token.is_whitespace()
            && (separates(tokens.last(), input.peek(), math)
                || separates(input.peek(), tokens.last(), math));
        if !removable {
            tokens.push(token);
        }
    }
}

/// Whether `separator` is a token that whitespace between it and `other` may be
/// dropped next to.
fn separates(
    separator: Option<&TokenOrValue<'_>>,
    other: Option<&TokenOrValue<'_>>,
    math: bool,
) -> bool {
    match separator {
        Some(TokenOrValue::Token(Token::Comma)) => true,
        Some(TokenOrValue::Token(Token::Delim('*' | '/'))) if math => !matches!(
            other,
            Some(TokenOrValue::Token(Token::Delim(_) | Token::Comment(_)))
        ),
        _ => false,
    }
}
