use stillwind_css::{ThemeIndex, declaration_format, fold_calc_expressions, format_numeric_zero};

use super::arbitrary::important;
use super::metadata::builtin_declaration_formats;
use super::normalization::normalize_var_fallbacks;

pub(super) fn finalize_declaration(
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

pub(super) fn apply_declaration_formats(mut declaration: String, theme: &ThemeIndex) -> String {
    let formats = if theme.declaration_formats.is_empty() {
        builtin_declaration_formats()
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

pub(super) fn apply_zero_length_format(mut declaration: String, theme: &ThemeIndex) -> String {
    let formats = if theme.declaration_formats.is_empty() {
        builtin_declaration_formats()
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

pub(super) fn normalize_format_var_fallbacks(
    mut declaration: String,
    theme: &ThemeIndex,
) -> String {
    let formats = if theme.declaration_formats.is_empty() {
        builtin_declaration_formats()
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
