#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MathValue {
    Number(f64),
    Dimension(f64, String),
    Percentage(f64),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MathExpression {
    Value(MathValue),
    Atom(String),
    Sum(Box<Self>, Box<Self>),
    Product(Box<Self>, Box<Self>),
    Quotient(Box<Self>, Box<Self>),
    Negate(Box<Self>),
}

impl MathExpression {
    pub(crate) fn negate(self) -> Self {
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

    pub(crate) fn product(left: Self, right: Self) -> Self {
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

    pub(crate) fn sum(left: Self, right: Self) -> Self {
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

    pub(crate) fn numeric_unit(&self) -> Option<&str> {
        match self {
            Self::Value(MathValue::Dimension(_, unit)) => Some(unit),
            Self::Product(left, right) | Self::Quotient(left, right) | Self::Sum(left, right) => {
                left.numeric_unit().or_else(|| right.numeric_unit())
            }
            Self::Negate(value) => value.numeric_unit(),
            Self::Value(MathValue::Number(_) | MathValue::Percentage(_)) | Self::Atom(_) => None,
        }
    }

    pub(crate) fn precedence(&self) -> u8 {
        match self {
            Self::Sum(..) => 1,
            Self::Product(..) | Self::Quotient(..) => 2,
            Self::Negate(..) => 3,
            Self::Value(..) | Self::Atom(..) => 4,
        }
    }

    pub(crate) fn serialize(&self, parent_precedence: u8) -> String {
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

pub(crate) fn serialize_number(value: f64) -> String {
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
