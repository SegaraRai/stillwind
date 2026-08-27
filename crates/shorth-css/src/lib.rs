//! CSS-first theme parsing. Lightning CSS validates the stylesheet before shorth
//! extracts its small, source-spanned `@theme` surface.

use lightningcss::stylesheet::{ParserOptions, StyleSheet};
use shorth_source::{FileId, SourceFile, Span, Spanned};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeToken {
    pub name: Spanned<String>,
    pub value: Spanned<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ThemeIndex {
    pub tokens: Vec<ThemeToken>,
}

#[derive(Debug, Error)]
pub enum ThemeError {
    #[error("CSS parse error: {0}")]
    Parse(String),
    #[error("unterminated @theme block")]
    UnterminatedTheme,
}

/// Parses and indexes source-spanned `@theme` token declarations.
///
/// # Errors
///
/// Returns an error when Lightning CSS rejects the stylesheet or a discovered
/// `@theme` block has no closing brace.
pub fn parse_theme(source: &SourceFile) -> Result<ThemeIndex, ThemeError> {
    StyleSheet::parse(source.text(), ParserOptions::default())
        .map_err(|error| ThemeError::Parse(error.to_string()))?;

    extract_theme_tokens(source.id(), source.text())
}

fn offset(value: usize) -> u32 {
    u32::try_from(value).expect("shorth source files must be smaller than 4 GiB")
}

fn extract_theme_tokens(file: FileId, css: &str) -> Result<ThemeIndex, ThemeError> {
    let mut tokens = Vec::new();
    let mut cursor = 0;

    while let Some(theme_relative) = css[cursor..].find("@theme") {
        let theme_start = cursor + theme_relative;
        let Some(open_relative) = css[theme_start..].find('{') else {
            return Err(ThemeError::UnterminatedTheme);
        };
        let open = theme_start + open_relative;
        let Some(close_relative) = css[open + 1..].find('}') else {
            return Err(ThemeError::UnterminatedTheme);
        };
        let close = open + 1 + close_relative;

        let body = &css[open + 1..close];
        let mut body_cursor = 0;
        for declaration in body.split_inclusive(';') {
            let declaration_start = open + 1 + body_cursor;
            body_cursor += declaration.len();
            let Some((raw_name, raw_value)) = declaration.trim_end_matches(';').split_once(':')
            else {
                continue;
            };
            let name = raw_name.trim();
            let value = raw_value.trim();
            if !name.starts_with("--") || value.is_empty() {
                continue;
            }

            let name_start = declaration_start + declaration.find(name).unwrap_or_default();
            let value_start = declaration_start + declaration.find(value).unwrap_or_default();
            tokens.push(ThemeToken {
                name: Spanned::new(
                    name.to_owned(),
                    Span::new(file, offset(name_start), offset(name_start + name.len())),
                ),
                value: Spanned::new(
                    value.to_owned(),
                    Span::new(file, offset(value_start), offset(value_start + value.len())),
                ),
            });
        }

        cursor = close + 1;
    }

    Ok(ThemeIndex { tokens })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_spanned_theme_tokens_after_lightningcss_validates_css() {
        let source = SourceFile::new(
            FileId(7),
            "shorth.css",
            "@theme { --color-accent: oklch(.6 .2 250); --spacing-4: 1rem; }",
        );

        let theme = parse_theme(&source).unwrap();

        assert_eq!(theme.tokens.len(), 2);
        assert_eq!(theme.tokens[0].name.value, "--color-accent");
        assert_eq!(source.slice(theme.tokens[1].value.span).unwrap(), "1rem");
    }
}
