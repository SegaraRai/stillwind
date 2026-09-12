//! Final serialization of resolved declarations.
//!
//! The registry describes how its declarations must be spelled, and this module applies
//! that description: `!important` on every declaration in the body, fallback
//! normalization for selected properties, slash spacing, zero-length units, and folding
//! of compile-time values inside `calc()`.
//!
//! The order of those steps is semantic, not incidental. Importance is added before
//! formatting so a formatted property can still be recognized, and `calc()` folding
//! runs last so it sees the final text.
//!
//! Declaration formats come from either the project CSS or the selected built-ins,
//! never from both: a project that declares formats replaces the built-in set rather
//! than extending it.

use stillwind_css::{fold_calc_expressions, format_numeric_zero};

use super::super::builtins::metadata::builtin_declaration_formats;
use super::super::syntax::closing_parenthesis;
use crate::{ThemeIndex, declaration_format};

/// Appends `!important` to every declaration in `declaration`.
///
/// The input is a declaration list, so the marker is applied per declaration rather
/// than once at the end of the string. An empty body stays empty instead of gaining a
/// stray marker. This helper splits on every semicolon, including semicolons inside
/// strings or function arguments; it does not tokenize the declaration list.
pub(crate) fn important(declaration: &str) -> String {
    declaration
        .split_terminator(';')
        .filter(|declaration| !declaration.is_empty())
        .fold(String::new(), |mut result, declaration| {
            result.push_str(declaration);
            result.push_str("!important;");
            result
        })
}

/// Applies every serialization rule to one resolved declaration.
///
/// Fallback normalization runs before the format rules because a format string may
/// describe a fallback that normalization would otherwise leave untouched; `calc()`
/// folding runs last so it operates on the final spelling.
pub(crate) fn finalize_declaration(
    declaration: String,
    is_important: bool,
    theme: &ThemeIndex,
) -> String {
    let declaration = if is_important {
        important(&declaration)
    } else {
        declaration
    };
    let declaration = normalize_format_var_fallbacks(declaration, theme);
    let declaration = apply_declaration_formats(declaration, theme);
    let declaration = apply_zero_length_format(declaration, theme);
    fold_calc_expressions(&declaration)
}

/// Applies formats that rewrite a property's value text, currently the slash spacing
/// required by `span` values such as `grid-row`.
///
/// Only a declaration that starts with the property and whose value contains `span` is
/// rewritten, so an unrelated property spelling the same prefix is left alone.
pub(crate) fn apply_declaration_formats(mut declaration: String, theme: &ThemeIndex) -> String {
    let formats = if theme.declaration_formats.is_empty() {
        builtin_declaration_formats(theme)
    } else {
        &theme.declaration_formats
    };
    for (property, format) in formats {
        if !format
            .split_whitespace()
            .any(|item| item == "slash-space-span")
        {
            continue;
        }
        let prefix = format!("{property}:");
        if let Some(value) = declaration.strip_prefix(&prefix) {
            if !value.contains("span") {
                continue;
            }
            let value = value.replace('/', " / ");
            declaration = format!("{prefix}{value}");
        }
    }
    declaration
}

/// Adds the unit to zero lengths for properties whose format requires it.
///
/// Both the property lookup and the replacement run on whole declarations, so a zero in
/// another property's value or in a function argument is not touched.
pub(crate) fn apply_zero_length_format(mut declaration: String, theme: &ThemeIndex) -> String {
    let formats = if theme.declaration_formats.is_empty() {
        builtin_declaration_formats(theme)
    } else {
        &theme.declaration_formats
    };
    let ranges = declaration
        .split_terminator(';')
        .scan(0, |cursor, segment| {
            let start = *cursor;
            *cursor += segment.len() + 1;
            Some((start, start + segment.len(), segment))
        })
        .filter_map(|(start, end, segment)| {
            let property = segment.split_once(':')?.0;
            declaration_format(formats, property)
                .is_some_and(|format| format.split_whitespace().any(|item| item == "zero-length"))
                .then_some((start, end))
        })
        .collect::<Vec<_>>();
    for (start, end) in ranges.into_iter().rev() {
        let segment = declaration[start..end].split_once(':').map_or_else(
            || declaration[start..end].to_owned(),
            |(property, value)| format!("{property}:{}", format_numeric_zero(value, "px")),
        );
        declaration.replace_range(start..end, &segment);
    }
    declaration
}

/// Normalizes `var()` fallbacks for properties whose format asks for it.
///
/// `var-fallback` adds an empty fallback to a reference that has none, which is how a
/// property is reset without losing the custom property; a fallback that is already
/// present, even an empty one, is preserved.
pub(crate) fn normalize_format_var_fallbacks(
    mut declaration: String,
    theme: &ThemeIndex,
) -> String {
    let formats = if theme.declaration_formats.is_empty() {
        builtin_declaration_formats(theme)
    } else {
        &theme.declaration_formats
    };
    for (property, format) in formats {
        let add_missing = format.split_whitespace().any(|item| item == "var-fallback");
        let normalize_empty = format
            .split_whitespace()
            .any(|item| item == "empty-var-fallback");
        if !add_missing && !normalize_empty {
            continue;
        }
        let prefix = format!("{property}:");
        let mut cursor = 0;
        while let Some(relative) = declaration[cursor..].find(&prefix) {
            let start = cursor + relative;
            let end = declaration[start..]
                .find(';')
                .map_or(declaration.len(), |offset| start + offset);
            let value = normalize_var_fallbacks(&declaration[start..end], add_missing);
            declaration.replace_range(start..end, &value);
            cursor = start + value.len();
            if end == declaration.len() {
                break;
            }
        }
    }
    declaration
}

/// Rewrites every `var(--x)` (or `var(--x, fallback)`) reference in a value.
///
/// The scan is paren-aware and leaves a malformed reference as written instead of
/// dropping it, because a value that already reaches this point has been accepted for
/// emission.
pub(crate) fn normalize_var_fallbacks(value: &str, add_missing: bool) -> String {
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0;
    while let Some(relative) = value[cursor..].find("var(--") {
        let start = cursor + relative;
        output.push_str(&value[cursor..start]);
        let Some(open) = value[start..].find('(').map(|offset| start + offset) else {
            output.push_str(&value[start..]);
            return output;
        };
        let Some(close) = closing_parenthesis(value, open) else {
            output.push_str(&value[start..]);
            return output;
        };
        let expression = &value[open + 1..close];
        output.push_str(&value[start..=open]);
        if let Some((name, fallback)) = expression.split_once(',') {
            output.push_str(name.trim());
            output.push(',');
            if !fallback.trim().is_empty() {
                output.push_str(fallback);
            }
        } else if add_missing {
            output.push_str(expression);
            output.push(',');
        } else {
            output.push_str(expression);
        }
        output.push(')');
        cursor = close + 1;
    }
    output.push_str(&value[cursor..]);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn important_handles_multiple_declarations_and_empty_input() {
        assert_eq!(
            important("color:red;width:1px;"),
            "color:red!important;width:1px!important;"
        );
        assert_eq!(important(""), "");
    }

    #[test]
    fn fallback_normalization_preserves_nonempty_and_malformed_values() {
        assert_eq!(normalize_var_fallbacks("var(--x)", true), "var(--x,)");
        assert_eq!(normalize_var_fallbacks("var(--x,  )", false), "var(--x,)");
        assert_eq!(
            normalize_var_fallbacks("var(--x, red)", true),
            "var(--x, red)"
        );
        assert_eq!(normalize_var_fallbacks("var(--x", true), "var(--x");
        assert_eq!(normalize_var_fallbacks("var(--x)", false), "var(--x)");
    }

    proptest::proptest! {
        #[test]
        fn fallback_insertion_is_idempotent(name in "[a-z]{1,20}") {
            let once = normalize_var_fallbacks(&format!("var(--{name})"), true);
            proptest::prop_assert_eq!(normalize_var_fallbacks(&once, true), once);
        }
    }

    #[test]
    fn authored_formats_only_change_matching_properties() {
        let theme = ThemeIndex {
            declaration_formats: [
                ("width".into(), "zero-length".into()),
                ("grid-row".into(), "slash-space-span".into()),
                ("color".into(), "var-fallback".into()),
            ]
            .into(),
            ..ThemeIndex::default()
        };
        assert_eq!(
            apply_zero_length_format("width:0;height:0;".into(), &theme),
            "width:0px;height:0;"
        );
        assert_eq!(
            apply_declaration_formats("grid-row:span 2/span 2;".into(), &theme),
            "grid-row:span 2 / span 2;"
        );
        assert_eq!(
            apply_declaration_formats("grid-row:1/2;".into(), &theme),
            "grid-row:1/2;"
        );
        assert_eq!(
            normalize_format_var_fallbacks("color:var(--x);width:var(--w);".into(), &theme),
            "color:var(--x,);width:var(--w);"
        );
        assert_eq!(
            finalize_declaration("width:calc(1px + 2px);".into(), true, &theme),
            "width:3px!important;"
        );
    }
}
