//! Formats complete CSS for display without running stylesheet optimization.

use lean_string::{LeanStr, LeanString, ToLeanStr};
use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};

use crate::normalize_scopes;

/// Sentinel at-rule placed where utility declarations or rules belong in a variant
/// preview. [`format_css`] replaces each occurrence with `/* … */` after formatting.
pub const PREVIEW_SLOT: &str = "@stillwind-preview-slot;";

/// CSS could not be parsed or printed for display.
#[derive(Debug, thiserror::Error)]
pub enum CssFormatError {
    /// The stylesheet parser rejected the input.
    #[error("CSS parse failed: {0}")]
    Parse(LeanStr),
    /// The stylesheet printer could not serialize a parsed value.
    #[error("CSS print failed: {0}")]
    Print(LeanStr),
}

/// Returns CSS with line breaks and two-space indentation for editor previews.
///
/// `css` must contain a complete stylesheet. Nesting and fallback declarations are
/// retained. The printer may normalize property values, such as color spelling.
///
/// # Errors
///
/// Returns an error if the stylesheet cannot be parsed or printed.
pub fn format_css(css: &str) -> Result<LeanString, CssFormatError> {
    let normalized = normalize_scopes(css);
    let css = normalized.css.as_str();
    let stylesheet = StyleSheet::parse(css, ParserOptions::default())
        .map_err(|error| CssFormatError::Parse(error.to_lean_str()))?;
    let formatted = stylesheet
        .to_css(PrinterOptions::default())
        .map_err(|error| CssFormatError::Print(error.to_lean_str()))?
        .code;
    Ok(replace_preview_slots(&formatted))
}

/// Replaces [`PREVIEW_SLOT`] lines with `/* … */` and removes any blank line that
/// Lightning CSS inserts between a preceding declaration and the sentinel at-rule.
fn replace_preview_slots(css: &str) -> LeanString {
    if !css.contains(PREVIEW_SLOT) {
        return css.into();
    }
    let mut output = LeanString::with_capacity(css.len());
    let mut lines = css.split_inclusive('\n').peekable();
    while let Some(line) = lines.next() {
        if line.trim().is_empty()
            && output.ends_with(";\n")
            && lines.peek().is_some_and(|next| next.trim() == PREVIEW_SLOT)
        {
            continue;
        }
        if let Some(prefix) = line.strip_suffix("@stillwind-preview-slot;\n") {
            output.push_str(prefix);
            output.push_str("/* … */\n");
        } else if let Some(prefix) = line.strip_suffix(PREVIEW_SLOT) {
            output.push_str(prefix);
            output.push_str("/* … */");
        } else {
            output.push_str(line);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_declarations_are_printed_as_zero_specificity_rules() {
        let output = format_css("@scope (.card) {color:red;}").unwrap();
        assert!(output.contains(":where(:scope)"));
        assert!(output.contains("color: red"));
    }

    #[test]
    fn formats_nested_rules_without_optimizing_fallbacks() {
        let css = "@media (width >= 40rem){&{display:-webkit-box;display:flex;&:hover{color:red}display:grid}}";
        assert_eq!(
            format_css(css).unwrap(),
            "@media (width >= 40rem) {\n  & {\n    display: -webkit-box;\n    display: flex;\n\n    &:hover {\n      color: red;\n    }\n\n    display: grid;\n  }\n}\n"
        );
    }

    #[test]
    fn preserves_syntax_inside_strings_and_custom_values() {
        let css = r#"&{content:"{a;b}";--image:url("data:image/svg+xml;a{b}");--size:var(--spacing);padding:calc(var(--size)*2)}"#;
        let formatted = format_css(css).unwrap();
        assert!(formatted.contains(r#"content: "{a;b}";"#));
        assert!(formatted.contains(r#"--image: url("data:image/svg+xml;a{b}");"#));
        assert!(formatted.contains("--size: var(--spacing);"));
        assert_eq!(format_css(&formatted).unwrap(), formatted);
    }

    #[test]
    fn rejects_invalid_css_without_discarding_declarations() {
        assert!(format_css("&{color red}").is_err());
    }

    #[test]
    fn formats_preview_slot_as_ellipsis_comment() {
        assert_eq!(
            format_css(&format!("&:first-child{{{PREVIEW_SLOT}}}")).unwrap(),
            "&:first-child {\n  /* … */\n}\n"
        );
        assert_eq!(
            format_css(&format!(
                "&:hover{{@media (hover: hover){{{PREVIEW_SLOT}}}}}"
            ))
            .unwrap(),
            "&:hover {\n  @media (hover: hover) {\n    /* … */\n  }\n}\n"
        );
        assert_eq!(
            format_css(&format!(
                "&::before{{content:var(--tw-content);{PREVIEW_SLOT}}}"
            ))
            .unwrap(),
            "&:before {\n  content: var(--tw-content);\n  /* … */\n}\n"
        );
        assert_eq!(
            format_css(&format!(
                "@media (prefers-color-scheme: dark){{&{{{PREVIEW_SLOT}}}}}"
            ))
            .unwrap(),
            "@media (prefers-color-scheme: dark) {\n  & {\n    /* … */\n  }\n}\n"
        );
    }
}
