//! Formats complete CSS for display without running stylesheet optimization.

use lean_string::{LeanStr, LeanString, ToLeanStr};
use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};

use crate::normalize_scopes;

/// At-rule name (with leading `@`) used for preview slot placeholders.
pub const PREVIEW_SLOT_RULE: &str = "@stillwind-preview-slot";

/// At-rule name (with leading `@`) used for rule-leading preview comments.
pub const PREVIEW_COMMENT_RULE: &str = "@stillwind-preview-comment";

/// Sentinel at-rule placed where utility declarations or rules belong in a variant
/// preview. [`format_css`] replaces each occurrence with `/* … */` after formatting.
pub const PREVIEW_SLOT: &str = "@stillwind-preview-slot;";

/// Maximum visible character count of a preview comment before truncation.
const MAX_PREVIEW_COMMENT_CHARS: usize = 80;

/// Encodes `label` as a [`PREVIEW_COMMENT_RULE`] statement that [`format_css`] prints as
/// a rule-leading `/* ... */` comment.
///
/// Whitespace runs (including line breaks) collapse to a single space, `*/` is broken
/// with a zero-width space so the comment cannot terminate early, and labels longer than
/// 80 characters are truncated with an ellipsis. Returns an empty string when `label`
/// contains only whitespace.
#[must_use]
pub fn preview_comment_at_rule(label: &str) -> LeanString {
    let mut normalized = String::with_capacity(label.len());
    let mut pending_space = false;
    for ch in label.chars() {
        if ch.is_whitespace() {
            pending_space = !normalized.is_empty();
        } else {
            if pending_space {
                normalized.push(' ');
                pending_space = false;
            }
            normalized.push(ch);
        }
    }
    if normalized.is_empty() {
        return LeanString::new();
    }
    let sanitized = normalized.replace("*/", "*\u{200B}/");
    let text = if sanitized.chars().count() > MAX_PREVIEW_COMMENT_CHARS {
        let mut cut: String = sanitized
            .chars()
            .take(MAX_PREVIEW_COMMENT_CHARS - 1)
            .collect();
        cut.push('…');
        cut
    } else {
        sanitized
    };
    let mut output = LeanString::with_capacity(PREVIEW_COMMENT_RULE.len() + text.len() + 5);
    output.push_str(PREVIEW_COMMENT_RULE);
    output.push_str(" \"");
    for ch in text.chars() {
        match ch {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            other => output.push(other),
        }
    }
    output.push_str("\";");
    output
}

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
    Ok(replace_preview_sentinels(&formatted))
}

/// Replaces [`PREVIEW_SLOT`] lines with `/* … */` (or `/* <prelude>: … */`) and
/// [`PREVIEW_COMMENT_RULE`] lines with `/* <text> */`, removing blank lines inserted by
/// Lightning CSS around them.
fn replace_preview_sentinels(css: &str) -> LeanString {
    if !css.contains(PREVIEW_SLOT_RULE) && !css.contains(PREVIEW_COMMENT_RULE) {
        return css.into();
    }
    let mut output = LeanString::with_capacity(css.len());
    let mut lines = css.split_inclusive('\n').peekable();
    let mut skip_blank = false;
    while let Some(line) = lines.next() {
        if skip_blank {
            skip_blank = false;
            if line.trim().is_empty() {
                continue;
            }
        }
        if line.trim().is_empty()
            && output.ends_with(";\n")
            && lines
                .peek()
                .is_some_and(|next| preview_slot_comment(next.trim()).is_some())
        {
            continue;
        }
        let (body, newline) = match line.strip_suffix('\n') {
            Some(body) => (body, "\n"),
            None => (line, ""),
        };
        let trimmed = body.trim_start();
        if let Some(comment) = preview_slot_comment(trimmed) {
            let indent = &body[..body.len() - trimmed.len()];
            output.push_str(indent);
            output.push_str(&comment);
            output.push_str(newline);
        } else if let Some(comment) = preview_comment_line(trimmed) {
            let indent = &body[..body.len() - trimmed.len()];
            output.push_str(indent);
            output.push_str(&comment);
            output.push_str(newline);
            skip_blank = true;
        } else {
            output.push_str(line);
        }
    }
    output
}

/// Returns the `/* ... */` comment for a trimmed [`PREVIEW_COMMENT_RULE`] line, or
/// `None` for any other line.
fn preview_comment_line(trimmed: &str) -> Option<LeanString> {
    let rest = trimmed
        .strip_prefix(PREVIEW_COMMENT_RULE)?
        .strip_prefix(' ')?
        .strip_suffix(';')?
        .trim();
    let inner = rest
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            rest.strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })?;
    let mut text = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\'
            && let Some(escaped) = chars.next()
        {
            text.push(escaped);
        } else {
            text.push(ch);
        }
    }
    Some(format!("/* {text} */").into())
}

/// Returns the placeholder comment for a trimmed [`PREVIEW_SLOT`] line, or `None` for
/// any other line.
fn preview_slot_comment(trimmed: &str) -> Option<LeanString> {
    if trimmed == PREVIEW_SLOT {
        return Some("/* … */".into());
    }
    let prelude = trimmed
        .strip_prefix(PREVIEW_SLOT_RULE)?
        .strip_prefix(' ')?
        .strip_suffix(';')?
        .trim();
    if prelude.is_empty() {
        Some("/* … */".into())
    } else {
        Some(format!("/* {prelude}: … */").into())
    }
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
        assert_eq!(
            format_css("&{@stillwind-preview-slot if(style(--foo));}").unwrap(),
            "& {\n  /* if(style(--foo)): … */\n}\n"
        );
        assert_eq!(
            format_css(
                "&::before{content:var(--tw-content);@stillwind-preview-slot if(style(--foo));}"
            )
            .unwrap(),
            "&:before {\n  content: var(--tw-content);\n  /* if(style(--foo)): … */\n}\n"
        );
    }

    #[test]
    fn formats_preview_comments_and_sanitizes_text() {
        let css = format!(
            "{}&{{padding:1rem;}}{}&{{color:red;}}",
            preview_comment_at_rule("active"),
            preview_comment_at_rule("intent === \"primary\" && /* x */\n  size === \"lg\"")
        );
        assert_eq!(
            format_css(&css).unwrap(),
            "/* active */\n& {\n  padding: 1rem;\n}\n\n/* intent === \"primary\" && /* x *\u{200B}/ size === \"lg\" */\n& {\n  color: red;\n}\n"
        );
        assert_eq!(preview_comment_at_rule("   \n\t  "), "");
    }
}
