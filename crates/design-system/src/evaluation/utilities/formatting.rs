//! Semantic declaration emission. Formatting and optimization belong to the host.

use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_css::{
    declaration_ranges, find_css_functions, format_numeric_zero, important_declarations,
};

use super::super::stillwind::stillwind_declaration_formats;
use crate::{ThemeIndex, declaration_format};

/// Emits resolved declarations with candidate priority and spaces in empty `var()`
/// fallbacks, without optimizing values.
pub(in crate::evaluation) fn finalize_declaration(
    declaration: LeanStr,
    is_important: bool,
    theme: &ThemeIndex,
) -> LeanStr {
    let declaration = theme
        .inline_asset_references(&declaration)
        .map_or(declaration, Into::into);
    let mut spaced = None;
    for call in find_css_functions(&declaration, &["var"], 0)
        .into_iter()
        .rev()
    {
        if call.end.is_some() && declaration[call.arguments.clone()].ends_with(',') {
            spaced
                .get_or_insert_with(|| declaration.to_lean_string())
                .insert(call.arguments.end, ' ');
        }
    }
    let declaration = spaced.map_or(declaration, Into::into);
    if is_important {
        important_declarations(&declaration).into()
    } else {
        declaration
    }
}

/// Supplies a length type while resolving a declaration, before joining its body. This
/// metadata describes value semantics, never a request for minification.
pub(in crate::evaluation) fn resolve_declaration_value(
    declaration: &str,
    theme: &ThemeIndex,
    stillwind: bool,
) -> LeanString {
    let formats = if theme.declaration_formats().is_empty() {
        stillwind_declaration_formats(theme)
    } else {
        theme.declaration_formats()
    };
    if !stillwind
        && !declaration_ranges(declaration).iter().any(|range| {
            declaration[range.clone()]
                .split_once(':')
                .is_some_and(|(property, _)| {
                    declaration_format(formats, property.trim()).is_some_and(|format| {
                        format.split_whitespace().any(|item| item == "zero-length")
                    })
                })
        })
    {
        return declaration.to_lean_string();
    }
    let mut output = LeanString::new();
    for range in declaration_ranges(declaration) {
        let segment = &declaration[range];
        if let Some((property, value)) = segment.split_once(':') {
            output.push_str(property.trim());
            output.push(':');
            if declaration_format(formats, property.trim())
                .is_some_and(|format| format.split_whitespace().any(|item| item == "zero-length"))
            {
                output.push_str(&format_numeric_zero(value.trim(), "px"));
            } else {
                output.push_str(value.trim());
            }
            output.push(';');
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeBuilder;

    #[test]
    fn preserves_authored_values_and_supplies_only_required_length_types() {
        let theme = ThemeBuilder {
            declaration_formats: [("--distance".into(), "zero-length".into())].into(),
            ..Default::default()
        }
        .finish();
        assert_eq!(
            resolve_declaration_value(
                "--distance:0;content:'var(--x) calc(1 + 2),var(--y)';width:calc(1px + 2px);transform:var(--x);",
                &theme,
                false
            ),
            "--distance:0px;content:'var(--x) calc(1 + 2),var(--y)';width:calc(1px + 2px);transform:var(--x);"
        );
        assert_eq!(
            finalize_declaration("width:calc(1px + 2px);".into(), true, &theme),
            "width:calc(1px + 2px)!important;"
        );
    }

    #[test]
    fn empty_variable_fallbacks_keep_a_space_without_changing_literals() {
        let theme = ThemeBuilder::default().finish();
        assert_eq!(
            finalize_declaration(
                "filter:var(--a,) var(--b,var(--c,));content:'var(--literal,)';width:var(--width, 1px);".into(),
                false,
                &theme,
            ),
            "filter:var(--a, ) var(--b,var(--c, ));content:'var(--literal,)';width:var(--width, 1px);"
        );
    }
}
