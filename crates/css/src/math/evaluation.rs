//! Numeric evaluation of `calc()` values for display.
//!
//! Folding keeps a division that would lose precision, because emitted CSS must keep
//! the authored value. Editor previews only describe a value, so this evaluation also
//! divides and accepts the floating-point result.

use lean_string::{LeanStr, ToLeanStr};

use super::expression::{MathExpression, MathValue};
use super::parser::MathParser;

/// Evaluates a numeric value or a `calc()` expression to one number and its unit.
///
/// Returns the unit as written for a dimension, `%` for a percentage, and an empty
/// string for a number. Division by a number and division of two values with the same
/// unit are evaluated, so the result may be inexact. Returns `None` when the value
/// contains anything other than numeric terms, combines different units, or divides
/// by zero.
#[must_use]
pub fn evaluate_numeric(value: &str) -> Option<(f64, LeanStr)> {
    let value = evaluate(&MathParser::new(value.trim()).parse()?)?;
    let result = match value {
        MathValue::Number(number) => (number, LeanStr::new()),
        MathValue::Dimension(number, unit) => (number, unit),
        MathValue::Percentage(number) => (number, "%".to_lean_str()),
    };
    result.0.is_finite().then_some(result)
}

fn evaluate(expression: &MathExpression) -> Option<MathValue> {
    use MathValue::{Dimension, Number, Percentage};

    match expression {
        MathExpression::Value(value) => Some(value.clone()),
        MathExpression::Atom(_) => None,
        MathExpression::Negate(value) => match evaluate(value)? {
            Number(value) => Some(Number(-value)),
            Dimension(value, unit) => Some(Dimension(-value, unit)),
            Percentage(value) => Some(Percentage(-value)),
        },
        MathExpression::Sum(left, right) => match (evaluate(left)?, evaluate(right)?) {
            (Number(left), Number(right)) => Some(Number(left + right)),
            (Percentage(left), Percentage(right)) => Some(Percentage(left + right)),
            (Dimension(left, unit), Dimension(right, right_unit))
                if unit.eq_ignore_ascii_case(&right_unit) =>
            {
                Some(Dimension(left + right, unit))
            }
            _ => None,
        },
        MathExpression::Product(left, right) => match (evaluate(left)?, evaluate(right)?) {
            (Number(scale), value) | (value, Number(scale)) => Some(scale_value(value, scale)),
            _ => None,
        },
        MathExpression::Quotient(left, right) => match (evaluate(left)?, evaluate(right)?) {
            (_, Number(0.0)) => None,
            (value, Number(divisor)) => Some(scale_value(value, divisor.recip())),
            (Percentage(left), Percentage(right)) if right != 0.0 => Some(Number(left / right)),
            (Dimension(left, unit), Dimension(right, right_unit))
                if right != 0.0 && unit.eq_ignore_ascii_case(&right_unit) =>
            {
                Some(Number(left / right))
            }
            _ => None,
        },
    }
}

fn scale_value(value: MathValue, scale: f64) -> MathValue {
    match value {
        MathValue::Number(value) => MathValue::Number(value * scale),
        MathValue::Dimension(value, unit) => MathValue::Dimension(value * scale, unit),
        MathValue::Percentage(value) => MathValue::Percentage(value * scale),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_division_and_keeps_units() {
        let result = evaluate_numeric("calc(1.25 / 0.875)").unwrap();
        assert!((result.0 - 1.25 / 0.875).abs() < 1e-15);
        assert_eq!(result.1, "");
        assert_eq!(
            evaluate_numeric("calc(.25rem * 4)"),
            Some((1.0, "rem".into()))
        );
        assert_eq!(
            evaluate_numeric("calc((1rem + 2rem) / 2)"),
            Some((1.5, "rem".into()))
        );
        assert_eq!(evaluate_numeric("calc(3px / 2px)"), Some((1.5, "".into())));
        assert_eq!(evaluate_numeric("calc(50% / 2)"), Some((25.0, "%".into())));
        assert_eq!(evaluate_numeric("1.50rem"), Some((1.5, "rem".into())));
    }

    #[test]
    fn rejects_atoms_mixed_units_and_division_by_zero() {
        assert_eq!(evaluate_numeric("calc(var(--x) * 2)"), None);
        assert_eq!(evaluate_numeric("calc(1rem + 1px)"), None);
        assert_eq!(evaluate_numeric("calc(1rem / 0)"), None);
        assert_eq!(evaluate_numeric("calc(1rem * 2px)"), None);
        assert_eq!(evaluate_numeric("red"), None);
    }
}
