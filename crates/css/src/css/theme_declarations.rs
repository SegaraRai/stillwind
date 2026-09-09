use stillwind_source::{FileId, Span, Spanned};

use super::theme_domains::{ThemeNamespace, ThemeTokenKind};
use super::theme_index::{ThemeToken, ThemeTokenMode};
use super::theme_parser::offset;

pub(super) fn extract_theme_declarations(
    file: FileId,
    body: &str,
    body_start: usize,
    mode: ThemeTokenMode,
) -> Vec<ThemeToken> {
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
                Span::new(file, offset(name_start), offset(name_start + name.len())),
            ),
            value: Spanned::new(
                value.to_owned(),
                Span::new(file, offset(value_start), offset(value_start + value.len())),
            ),
            kind: classify_theme_token(name),
            mode,
        });
    }
    tokens
}

pub(super) fn matching_brace(value: &str, open: usize) -> Option<usize> {
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
            '{' => depth += 1,
            '}' => {
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

pub(super) fn classify_theme_token(name: &str) -> ThemeTokenKind {
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
