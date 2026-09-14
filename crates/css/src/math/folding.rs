//! `calc()` folding over a CSS fragment.
//!
//! Folding locates complete `calc(` names outside strings, comments and escapes,
//! resolves the matching parenthesis, and reprints successfully parsed expressions.
//! A `calc()` that fails to parse, or whose matching parenthesis is missing,
//! is copied through unchanged, so malformed input still reaches the CSS parser and
//! produces a real error there.

use super::expression::{MathExpression, MathValue};
use super::parser::MathParser;
use crate::find_css_functions;

/// Returns the byte offset of the `)` matching the `(` at `open`.
///
/// Quoted strings and escaped characters are skipped, so parentheses inside a string do
/// not affect the depth. `None` means the parenthesis is never closed or a `)` appears
/// first.
pub(crate) fn matching_parenthesis(value: &str, open: usize) -> Option<usize> {
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

/// Parses and folds `calc()` expressions, assigning a dimension to a folded zero when
/// the surrounding declaration metadata requires one.
///
/// The unit applies only to a value that really folded to the bare number zero,
/// including `calc(0)`, but never to a bare `0` outside `calc()` or to an atom.
#[must_use]
pub fn fold_calc_expressions_with_zero_unit(value: &str, zero_unit: Option<&str>) -> String {
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0;
    for found in find_css_functions(value, &["calc"], 0) {
        let start = found.start;
        if start < cursor {
            continue;
        }
        output.push_str(&value[cursor..start]);
        let Some(end) = found.end else {
            output.push_str(&value[start..]);
            return output;
        };
        let expression = &value[found.arguments];
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
            output.push_str(&value[start..end]);
        }
        cursor = end;
    }
    output.push_str(&value[cursor..]);
    output
}

/// Applies a declaration's length-zero format after parsing the value as CSS numeric
/// syntax. Non-zero and non-numeric values are returned unchanged.
#[must_use]
pub fn format_numeric_zero(value: &str, unit: &str) -> String {
    if matches!(
        MathParser::new(value.trim()).parse(),
        Some(MathExpression::Value(MathValue::Number(0.0)))
    ) {
        format!("0{unit}")
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::important_declarations;

    #[test]
    fn folding_preserves_strings_comments_urls_and_identifier_suffixes() {
        let css = r#"content:"calc(1 + 1)";/* calc(2 + 2) */background:url('data:text/plain;calc(1 + 1)');x:mycalc(2 + 2);width:calc(1px + 2px);"#;
        assert_eq!(
            fold_calc_expressions(css),
            css.replace("width:calc(1px + 2px)", "width:3px")
        );
        let important = important_declarations(css);
        assert!(
            fold_calc_expressions(&important).contains("data:text/plain;calc(1 + 1)"),
            "{important}"
        );
    }

    #[test]
    fn folds_trailing_numeric_factors_without_string_rewrites() {
        assert_eq!(
            fold_calc_expressions(
                "left:calc(var(--spacing) * 4 * -1);right:calc(var(--spacing) * -1 * 4);"
            ),
            "left:calc(var(--spacing) * -4);right:calc(var(--spacing) * -4);"
        );
    }

    #[test]
    fn folds_calc_expressions_through_a_numeric_syntax_tree() {
        assert_eq!(
            fold_calc_expressions(
                "width:calc(2 * 1px);scale:calc(125 * 1%);left:calc(var(--spacing) * 4);right:calc((2px + 3px) * -1);"
            ),
            "width:2px;scale:125%;left:calc(var(--spacing) * 4);right:-5px;"
        );
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn preserves_invalid_math_and_applies_zero_units_only_to_numeric_zero() {
        assert_eq!(fold_calc_expressions("calc(1 +)"), "calc(1 +)");
        assert_eq!(fold_calc_expressions("calc(1 + 2"), "calc(1 + 2");
        assert_eq!(format_numeric_zero("calc(2 - 2)", "px"), "0px");
        assert_eq!(format_numeric_zero("red", "px"), "red");
        assert_eq!(format_numeric_zero("2", "px"), "2");
    }

    proptest::proptest! {
        #[test]
        fn integer_arithmetic_obeys_precedence(a in -1000i32..1000, b in -1000i32..1000,c in -20i32..20) {
            let source = format!("calc({a} + {b} * {c})");
            proptest::prop_assert_eq!(fold_calc_expressions(&source), (a + b * c).to_string());
        }

        #[test]
        fn folding_a_numeric_result_is_idempotent(a in -1000i32..1000, b in -1000i32..1000) {
            let once = fold_calc_expressions(&format!("calc({a}px + {b}px)"));
            proptest::prop_assert_eq!(fold_calc_expressions(&once), once);
        }
    }
}
