use stillwind_css::blocks::matching_brace;
use stillwind_source::{FileId, Span, Spanned};

use super::namespace::ThemeNamespace;
use super::parser::{ThemeError, offset};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeTokenMode {
    Default,
    Inline,
    Static,
}

impl ThemeTokenMode {
    pub fn parse(modifier: &str) -> Self {
        match modifier {
            "inline" => ThemeTokenMode::Inline,
            "static" => ThemeTokenMode::Static,
            _ => ThemeTokenMode::Default,
        }
    }
}

/// The semantic identity of one custom property in a `@theme` block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ThemeTokenKind {
    /// `--spacing` is a scalar used by numeric spacing utilities.
    Scale(ThemeNamespace),
    /// A named token such as `--color-accent` or `--breakpoint-md`.
    Token {
        namespace: ThemeNamespace,
        name: String,
    },
    /// A property that Stillwind emits unchanged but does not use for built-ins.
    Custom,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeToken {
    pub name: Spanned<String>,
    pub value: Spanned<String>,
    pub kind: ThemeTokenKind,
    pub mode: ThemeTokenMode,
}

pub(crate) fn extract_theme_declarations(
    file: FileId,
    body: &str,
    body_start: usize,
    mode: ThemeTokenMode,
) -> Result<Vec<ThemeToken>, ThemeError> {
    let end = body_start
        .checked_add(body.len())
        .ok_or(ThemeError::InvalidOffset(body_start))?;
    offset(end)?;
    let mut tokens = Vec::new();
    let mut cursor = 0;
    while cursor < body.len() {
        cursor += body[cursor..]
            .find(|character: char| !character.is_whitespace())
            .unwrap_or(body[cursor..].len());
        if cursor >= body.len() {
            break;
        }
        if body[cursor..].starts_with("/*") {
            let Some(end) = body[cursor + 2..].find("*/") else {
                break;
            };
            cursor += end + 4;
            continue;
        }
        if body[cursor..].starts_with('@') {
            let Some((offset, delimiter)) = body[cursor..]
                .char_indices()
                .find(|(_, character)| matches!(character, '{' | ';'))
            else {
                break;
            };
            if delimiter == ';' {
                cursor += offset + 1;
            } else if let Some(close) = matching_brace(body, cursor + offset) {
                cursor = close + 1;
            } else {
                break;
            }
            continue;
        }

        let declaration_start = cursor;
        let Some(end) = body[cursor..].find(';') else {
            break;
        };
        let declaration = &body[cursor..cursor + end];
        cursor += end + 1;
        let Some((raw_name, raw_value)) = declaration.split_once(':') else {
            continue;
        };
        let name = raw_name.trim();
        let value = raw_value.trim();
        if !name.starts_with("--") || value.is_empty() {
            continue;
        }
        let name_start =
            body_start + declaration_start + declaration.find(name).unwrap_or_default();
        let value_start =
            body_start + declaration_start + declaration.find(value).unwrap_or_default();
        tokens.push(ThemeToken {
            name: Spanned::new(
                name.to_owned(),
                Span::new(file, offset(name_start)?, offset(name_start + name.len())?),
            ),
            value: Spanned::new(
                value.to_owned(),
                Span::new(
                    file,
                    offset(value_start)?,
                    offset(value_start + value.len())?,
                ),
            ),
            kind: classify_theme_token(name),
            mode,
        });
    }
    Ok(tokens)
}

pub(crate) fn classify_theme_token(name: &str) -> ThemeTokenKind {
    let Some(name) = name.strip_prefix("--") else {
        return ThemeTokenKind::Custom;
    };
    for namespace in [
        ThemeNamespace::AspectRatio,
        ThemeNamespace::FontWeight,
        ThemeNamespace::FontStretch,
        ThemeNamespace::InsetShadow,
        ThemeNamespace::DropShadow,
        ThemeNamespace::TextShadow,
        ThemeNamespace::Breakpoint,
        ThemeNamespace::Perspective,
        ThemeNamespace::Container,
        ThemeNamespace::Tracking,
        ThemeNamespace::Duration,
        ThemeNamespace::Animate,
        ThemeNamespace::Color,
        ThemeNamespace::Spacing,
        ThemeNamespace::Radius,
        ThemeNamespace::Shadow,
        ThemeNamespace::Leading,
        ThemeNamespace::Font,
        ThemeNamespace::Text,
        ThemeNamespace::Blur,
        ThemeNamespace::Ease,
        ThemeNamespace::ZIndex,
    ] {
        let namespace_name = namespace.css_name();
        if name == namespace_name {
            return ThemeTokenKind::Scale(namespace);
        }
        if let Some(token_name) = name.strip_prefix(&format!("{namespace_name}-")) {
            return ThemeTokenKind::Token {
                namespace,
                name: token_name.to_owned(),
            };
        }
    }
    ThemeTokenKind::Custom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_overflowing_base_offsets_before_token_arithmetic() {
        assert!(matches!(
            extract_theme_declarations(FileId(1), "--x:red;", usize::MAX, ThemeTokenMode::Default),
            Err(ThemeError::InvalidOffset(_))
        ));
        assert!(matches!(
            extract_theme_declarations(
                FileId(1),
                "--x:red;",
                u32::MAX as usize,
                ThemeTokenMode::Default
            ),
            Err(ThemeError::InvalidOffset(_))
        ));
    }

    #[test]
    fn declaration_extraction_keeps_value_spans_and_skips_non_declarations() {
        let tokens = extract_theme_declarations(
            FileId(1),
            "--color-a: red; --spacing: .25rem;",
            10,
            ThemeTokenMode::Inline,
        )
        .unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].name.value, "--color-a");
        assert_eq!(tokens[0].name.span.start.0, 10);
        assert_eq!(tokens[0].value.value, "red");
        assert_eq!(tokens[0].mode, ThemeTokenMode::Inline);
        assert!(matches!(
            classify_theme_token("--color-a"),
            ThemeTokenKind::Token {
                namespace: ThemeNamespace::Color,
                ..
            }
        ));
        assert_eq!(classify_theme_token("--unknown-a"), ThemeTokenKind::Custom);
        assert_eq!(ThemeTokenMode::parse("static"), ThemeTokenMode::Static);
        assert_eq!(ThemeTokenMode::parse("unknown"), ThemeTokenMode::Default);
    }
}
