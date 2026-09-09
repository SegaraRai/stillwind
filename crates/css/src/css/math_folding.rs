use super::{MathExpression, MathParser, MathValue};

pub(super) fn matching_parenthesis(value: &str, open: usize) -> Option<usize> {
    let mut depth = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    for (offset, character) in value[open..].char_indices() {
        if let Some(current_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == current_quote {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parses and folds every valid `calc()` expression in a CSS fragment.
///
/// Invalid or currently unsupported expressions are retained byte-for-byte.
#[must_use]
pub fn fold_calc_expressions(value: &str) -> String {
    fold_calc_expressions_with_zero_unit(value, None)
}

/// Parses and folds `calc()` expressions, assigning a dimension to a folded
/// zero when the surrounding declaration metadata requires one.
#[must_use]
pub fn fold_calc_expressions_with_zero_unit(value: &str, zero_unit: Option<&str>) -> String {
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0;
    while let Some(relative) = value[cursor..].find("calc(") {
        let start = cursor + relative;
        output.push_str(&value[cursor..start]);
        let open = start + "calc".len();
        let Some(close) = matching_parenthesis(value, open) else {
            output.push_str(&value[start..]);
            return output;
        };
        let expression = &value[open + 1..close];
        if let Some(mut expression) = MathParser::new(expression).parse() {
            if matches!(expression, MathExpression::Value(MathValue::Number(0.0)))
                && let Some(unit) = zero_unit
            {
                expression = MathExpression::Value(MathValue::Dimension(0.0, unit.to_owned()));
            }
            let serialized = expression.serialize(0);
            if matches!(
                expression,
                MathExpression::Value(_) | MathExpression::Atom(_)
            ) {
                output.push_str(&serialized);
            } else {
                output.push_str("calc(");
                output.push_str(&serialized);
                output.push(')');
            }
        } else {
            output.push_str(&value[start..=close]);
        }
        cursor = close + 1;
    }
    output.push_str(&value[cursor..]);
    output
}

/// Applies a declaration's length-zero format after parsing the value as CSS
/// numeric syntax. Non-zero and non-numeric values are returned unchanged.
#[must_use]
pub fn format_numeric_zero(value: &str, unit: &str) -> String {
    let folded = fold_calc_expressions_with_zero_unit(value, Some(unit));
    if matches!(
        MathParser::new(folded.trim()).parse(),
        Some(MathExpression::Value(MathValue::Number(0.0)))
    ) {
        format!("0{unit}")
    } else {
        folded
    }
}
