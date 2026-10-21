//! `calc()` folding over a CSS fragment.
//!
//! Folding locates complete `calc(` names outside strings, comments and escapes,
//! resolves the matching parenthesis, and reprints successfully parsed expressions.
//! A `calc()` that fails to parse, or whose matching parenthesis is missing,
//! is copied through unchanged, so malformed input still reaches the CSS parser and
//! produces a real error there.

use lean_string::{LeanString, ToLeanStr, ToLeanString};

use super::expression::{MathExpression, MathValue};
use super::parser::MathParser;
use crate::functions::find_css_functions;

/// Parses and folds every valid `calc()` expression in a CSS fragment.
///
/// Invalid or unsupported expressions are retained byte-for-byte. `zero_unit` assigns a
/// dimension to a folded zero when the surrounding declaration requires one.
///
/// The unit applies only to a value that really folded to the bare number zero,
/// including `calc(0)`, but never to a bare `0` outside `calc()` or to an atom.
#[must_use]
pub fn fold_calc_expressions(value: &str, zero_unit: Option<&str>) -> LeanString {
    let mut output = LeanString::with_capacity(value.len());
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
                expression = MathExpression::Value(MathValue::Dimension(0.0, unit.to_lean_str()));
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
pub fn format_numeric_zero(value: &str, unit: &str) -> LeanString {
    if matches!(
        MathParser::new(value.trim()).parse(),
        Some(MathExpression::Value(MathValue::Number(0.0)))
    ) {
        format_args!("0{unit}").to_lean_string()
    } else {
        value.to_lean_string()
    }
}

#[cfg(test)]
mod tests {
    use stillwind_source::ReplaceText;

    use super::*;
    use crate::important_declarations;
    use crate::math::parser::MAX_VALUE_NESTING;

    #[test]
    fn folding_preserves_strings_comments_urls_and_identifier_suffixes() {
        let css = r#"content:"calc(1 + 1)";/* calc(2 + 2) */background:url('data:text/plain;calc(1 + 1)');x:mycalc(2 + 2);width:calc(1px + 2px);"#;
        assert_eq!(
            fold_calc_expressions(css, None),
            css.replace_text("width:calc(1px + 2px)", "width:3px")
        );
        let important = important_declarations(css);
        assert!(
            fold_calc_expressions(&important, None).contains("data:text/plain;calc(1 + 1)"),
            "{important}"
        );
    }

    #[test]
    fn folds_trailing_numeric_factors_without_string_rewrites() {
        assert_eq!(
            fold_calc_expressions(
                "left:calc(var(--spacing) * 4 * -1);right:calc(var(--spacing) * -1 * 4);",
                None,
            ),
            "left:calc(var(--spacing) * -4);right:calc(var(--spacing) * -4);"
        );
    }

    #[test]
    fn folds_calc_expressions_through_a_numeric_syntax_tree() {
        assert_eq!(
            fold_calc_expressions(
                "width:calc(2 * 1px);scale:calc(125 * 1%);left:calc(var(--spacing) * 4);right:calc((2px + 3px) * -1);",
                None,
            ),
            "width:2px;scale:125%;left:calc(var(--spacing) * 4);right:-5px;"
        );
    }

    #[test]
    fn nesting_past_the_limit_is_reported_and_left_unfolded() {
        let levels = MAX_VALUE_NESTING + 1;
        let value = format_args!("{}1{}", "(".repeat(levels), ")".repeat(levels)).to_lean_string();
        let css = format_args!("width:calc({value})").to_lean_string();
        assert_eq!(fold_calc_expressions(&css, None), css);
    }

    #[test]
    fn sibling_groups_and_quoted_parentheses_do_not_consume_nesting() {
        let siblings =
            format_args!("calc({})", "(1) + ".repeat(200).trim_end_matches(" + ")).to_lean_string();
        assert_eq!(fold_calc_expressions(&siblings, None), "200");
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn preserves_invalid_math_and_applies_zero_units_only_to_numeric_zero() {
        assert_eq!(fold_calc_expressions("calc(1 +)", None), "calc(1 +)");
        assert_eq!(fold_calc_expressions("calc(1 + 2", None), "calc(1 + 2");
        assert_eq!(fold_calc_expressions("calc(2 - 2)", Some("px")), "0px");
        assert_eq!(fold_calc_expressions("0", Some("px")), "0");
        assert_eq!(format_numeric_zero("calc(2 - 2)", "px"), "0px");
        assert_eq!(format_numeric_zero("red", "px"), "red");
        assert_eq!(format_numeric_zero("2", "px"), "2");
    }

    proptest::proptest! {
        #[test]
        fn integer_arithmetic_obeys_precedence(a in -1000i32..1000, b in -1000i32..1000,c in -20i32..20) {
            let source = format_args!("calc({a} + {b} * {c})").to_lean_string();
            proptest::prop_assert_eq!(fold_calc_expressions(&source, None), (a + b * c).to_lean_string());
        }

        #[test]
        fn folding_a_numeric_result_is_idempotent(a in -1000i32..1000, b in -1000i32..1000) {
            let once = fold_calc_expressions(&format_args!("calc({a}px + {b}px)").to_lean_string(), None);
            proptest::prop_assert_eq!(fold_calc_expressions(&once, None), once);
        }
    }
}
