//! The folded numeric tree and its printer.
//!
//! Values are kept in their authored unit spelling, so `2rem + 3rem` folds to `5rem`
//! but `1cm + 1mm` does not fold at all - no conversion happens here. Anything the
//! parser cannot reduce to a number, dimension or percentage becomes an `Atom` and is
//! preserved verbatim.
//!
//! Serialization uses operator precedence. It may flatten equal-precedence sums and
//! products, so the printed syntax tree need not be identical to the stored tree.

/// A numeric CSS value whose unit category is retained during constant folding.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum MathValue {
    /// A unitless number.
    Number(
        /// Numeric part of a CSS number.
        f64,
    ),
    /// A number with its authored unit, for example `2px` or `3rem`.
    Dimension(
        /// Numeric coefficient preceding the unit.
        f64,
        /// Authored unit token, without whitespace or a numeric prefix; compared by exact spelling.
        String,
    ),
    /// A percentage, stored as the numeric part only.
    Percentage(
        /// Numeric part before `%`, so `50` represents `50%`, not `0.5`.
        f64,
    ),
}

/// One node of a parsed `calc()` expression.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum MathExpression {
    /// A fully known numeric value.
    Value(
        /// Numeric operand to serialize.
        MathValue,
    ),
    /// Text that is not numeric, such as `var(--x)` or an unknown function call, kept
    /// exactly as it was written.
    Atom(
        /// Opaque CSS operand text, preserving its internal whitespace and syntax.
        String,
    ),
    /// Addition of two operands.
    Sum(
        /// Left operand of `+`.
        Box<Self>,
        /// Right operand of `+`.
        Box<Self>,
    ),
    /// Multiplication of two operands.
    Product(
        /// Left operand of `*`.
        Box<Self>,
        /// Right operand of `*`.
        Box<Self>,
    ),
    /// Division of two operands, kept only when it cannot be folded.
    Quotient(
        /// Numerator.
        Box<Self>,
        /// Denominator; zero remains representable when division cannot be folded.
        Box<Self>,
    ),
    /// Unary minus, kept when the operand is not a value.
    Negate(
        /// Operand of unary minus.
        Box<Self>,
    ),
}

impl MathExpression {
    /// Negates the expression, folding the sign into a numeric value and cancelling an
    /// existing negation.
    pub(super) fn negate(self) -> Self {
        match self {
            Self::Value(MathValue::Number(value)) => Self::Value(MathValue::Number(-value)),
            Self::Value(MathValue::Dimension(value, unit)) => {
                Self::Value(MathValue::Dimension(-value, unit))
            }
            Self::Value(MathValue::Percentage(value)) => Self::Value(MathValue::Percentage(-value)),
            Self::Negate(value) => *value,
            value => Self::Negate(Box::new(value)),
        }
    }

    /// Multiplies two operands, folding what is exactly foldable.
    ///
    /// Multiplying by a dimensionless number keeps the other operand's unit, and
    /// multiplying a zero by a percentage stays a percentage so the result keeps
    /// percentage semantics in CSS. Anything else is kept as a product.
    pub(super) fn product(left: Self, right: Self) -> Self {
        if let (
            Self::Product(value, coefficient),
            Self::Value(MathValue::Number(right_coefficient)),
        ) = (&left, &right)
            && let Self::Value(MathValue::Number(left_coefficient)) = coefficient.as_ref()
        {
            return Self::product(
                (**value).clone(),
                Self::Value(MathValue::Number(left_coefficient * right_coefficient)),
            );
        }
        match (left, right) {
            (Self::Value(MathValue::Number(0.0)), Self::Value(MathValue::Percentage(_)))
            | (Self::Value(MathValue::Percentage(_)), Self::Value(MathValue::Number(0.0))) => {
                Self::Value(MathValue::Percentage(0.0))
            }
            (Self::Value(MathValue::Number(0.0)), value)
            | (value, Self::Value(MathValue::Number(0.0))) => value
                .numeric_unit()
                .map_or(Self::Value(MathValue::Number(0.0)), |unit| {
                    Self::Value(MathValue::Dimension(0.0, unit.to_owned()))
                }),
            (Self::Value(MathValue::Number(1.0)), value)
            | (value, Self::Value(MathValue::Number(1.0))) => value,
            (Self::Value(MathValue::Number(left)), Self::Value(MathValue::Number(right))) => {
                Self::Value(MathValue::Number(left * right))
            }
            (
                Self::Value(MathValue::Number(scale)),
                Self::Value(MathValue::Dimension(value, unit)),
            )
            | (
                Self::Value(MathValue::Dimension(value, unit)),
                Self::Value(MathValue::Number(scale)),
            ) => Self::Value(MathValue::Dimension(value * scale, unit)),
            (Self::Value(MathValue::Number(scale)), Self::Value(MathValue::Percentage(value)))
            | (Self::Value(MathValue::Percentage(value)), Self::Value(MathValue::Number(scale))) => {
                Self::Value(MathValue::Percentage(value * scale))
            }
            (left, right) => Self::Product(Box::new(left), Box::new(right)),
        }
    }

    /// Adds two operands, folding only values of the same kind.
    ///
    /// Dimensions fold only when their unit spellings match case-insensitively; no unit
    /// conversion is attempted, so mixed units stay symbolic.
    pub(super) fn sum(left: Self, right: Self) -> Self {
        match (left, right) {
            (Self::Value(MathValue::Number(left)), Self::Value(MathValue::Number(right))) => {
                Self::Value(MathValue::Number(left + right))
            }
            (
                Self::Value(MathValue::Dimension(left, left_unit)),
                Self::Value(MathValue::Dimension(right, right_unit)),
            ) if left_unit.eq_ignore_ascii_case(&right_unit) => {
                Self::Value(MathValue::Dimension(left + right, left_unit))
            }
            (
                Self::Value(MathValue::Percentage(left)),
                Self::Value(MathValue::Percentage(right)),
            ) => Self::Value(MathValue::Percentage(left + right)),
            (left, right) => Self::Sum(Box::new(left), Box::new(right)),
        }
    }

    /// The unit of the first numeric dimension anywhere in the tree.
    ///
    /// Used to give a folded bare zero the dimension the surrounding declaration
    /// expects; it never invents a unit where none exists.
    pub(super) fn numeric_unit(&self) -> Option<&str> {
        match self {
            Self::Value(MathValue::Dimension(_, unit)) => Some(unit),
            Self::Product(left, right) | Self::Quotient(left, right) | Self::Sum(left, right) => {
                left.numeric_unit().or_else(|| right.numeric_unit())
            }
            Self::Negate(value) => value.numeric_unit(),
            Self::Value(MathValue::Number(_) | MathValue::Percentage(_)) | Self::Atom(_) => None,
        }
    }

    /// Binding strength of the node, used only to decide parentheses.
    pub(super) fn precedence(&self) -> u8 {
        match self {
            Self::Sum(..) => 1,
            Self::Product(..) | Self::Quotient(..) => 2,
            Self::Negate(..) => 3,
            Self::Value(..) | Self::Atom(..) => 4,
        }
    }

    /// Prints the expression, parenthesizing operands weaker than `parent_precedence`.
    ///
    /// A negation on the right of a sum prints as subtraction, and division prints its
    /// right operand one level tighter. Equal-precedence sums and products may be
    /// flattened during printing.
    pub(super) fn serialize(&self, parent_precedence: u8) -> String {
        let precedence = self.precedence();
        let value = match self {
            Self::Value(MathValue::Number(value)) => serialize_number(*value),
            Self::Value(MathValue::Dimension(value, unit)) => {
                format!("{}{unit}", serialize_number(*value))
            }
            Self::Value(MathValue::Percentage(value)) => format!("{}%", serialize_number(*value)),
            Self::Atom(value) => value.clone(),
            Self::Sum(left, right) => {
                if let Self::Negate(right) = right.as_ref() {
                    format!(
                        "{} - {}",
                        left.serialize(precedence),
                        right.serialize(precedence + 1)
                    )
                } else {
                    format!(
                        "{} + {}",
                        left.serialize(precedence),
                        right.serialize(precedence)
                    )
                }
            }
            Self::Product(left, right) => format!(
                "{} * {}",
                left.serialize(precedence),
                right.serialize(precedence)
            ),
            Self::Quotient(left, right) => format!(
                "{} / {}",
                left.serialize(precedence),
                right.serialize(precedence + 1)
            ),
            Self::Negate(value) => format!("-{}", value.serialize(precedence)),
        };
        if precedence < parent_precedence {
            format!("({value})")
        } else {
            value
        }
    }
}

/// Prints a number the way Stillwind emits CSS numbers.
///
/// Negative zero prints as `0`, integral values lose their fractional part, and
/// fractions below one drop the leading zero (`.5`, `-.5`).
fn serialize_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    if value.fract() == 0.0 {
        return format!("{value:.0}");
    }
    let mut value = value.to_string();
    if let Some(fraction) = value.strip_prefix("0.") {
        value = format!(".{fraction}");
    } else if let Some(fraction) = value.strip_prefix("-0.") {
        value = format!("-.{fraction}");
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_preserves_units_and_parenthesizes_non_associative_operands() {
        let px = MathExpression::Value(MathValue::Dimension(3.0, "px".into()));
        let zero = MathExpression::Value(MathValue::Number(0.0));
        assert_eq!(
            MathExpression::product(px.clone(), zero).serialize(0),
            "0px"
        );
        assert_eq!(px.clone().negate().negate(), px);
        let sum = MathExpression::Sum(
            Box::new(MathExpression::Atom("a".into())),
            Box::new(MathExpression::Atom("b".into())),
        );
        let quotient = MathExpression::Quotient(Box::new(px), Box::new(sum));
        assert_eq!(quotient.serialize(0), "3px / (a + b)");
        assert_eq!(quotient.numeric_unit(), Some("px"));
        assert_eq!(serialize_number(-0.0), "0");
        assert_eq!(serialize_number(0.125), ".125");
    }
}
