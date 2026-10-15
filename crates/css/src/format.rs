//! Formats complete CSS for display without running stylesheet optimization.

use lean_string::{LeanStr, LeanString, ToLeanStr};
use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};

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
    let stylesheet = StyleSheet::parse(css, ParserOptions::default())
        .map_err(|error| CssFormatError::Parse(error.to_lean_str()))?;
    stylesheet
        .to_css(PrinterOptions::default())
        .map(|result| result.code.into())
        .map_err(|error| CssFormatError::Print(error.to_lean_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
