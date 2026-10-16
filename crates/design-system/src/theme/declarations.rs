//! Theme token extraction: which custom properties a `@theme` block declares, what each
//! one means, and where it was written.
//!
//! A declaration is read textually so its span can point at the authored name and value
//! inside the buffer under parse. The span math is explicit: offsets are validated
//! before arithmetic, so an overflowing base or a buffer beyond the 32-bit source limit
//! is an error rather than a wrapped coordinate.
//!
//! Extraction is syntax-level. Namespace resets and duplicate resolution are decided
//! later, because they depend on the whole token list rather than on one declaration.

use lean_string::{LeanStr, ToLeanStr, ToLeanString};

use stillwind_css::matching_brace;
use stillwind_diagnostics::CompileDiagnostic;
use stillwind_source::{FileId, Span, Spanned};

use super::namespace::ThemeNamespace;
use super::parser::offset;

/// How a `@theme` block's declarations are emitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeTokenMode {
    /// Ordinary token: emitted when required and referenced by `var()`.
    Default,
    /// Token inlined into the generated declaration instead of being emitted.
    Inline,
    /// Token included in the required set without a reference; later resets and
    /// registry pruning can still remove it.
    Static,
}

impl ThemeTokenMode {
    /// Reads a `@theme` block modifier; an unrecognized modifier is treated as the
    /// default mode rather than rejected.
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
        /// Namespace the token was declared under.
        namespace: ThemeNamespace,
        /// Token name without its namespace prefix.
        name: LeanStr,
    },
    /// A property that Stillwind emits unchanged but does not use for built-ins.
    Custom,
}

/// One custom-property declaration from a `@theme` block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeToken {
    /// Authored custom-property name with its span in the parsed buffer.
    pub name: Spanned<LeanStr>,
    /// Authored value with its span in the parsed buffer.
    pub value: Spanned<LeanStr>,
    /// Which namespace and name the declaration resolved to.
    pub kind: ThemeTokenKind,
    /// Emission policy: ordinary referenced token, inline value, or static retention.
    pub mode: ThemeTokenMode,
}

/// Reads the declarations of one `@theme` body.
///
/// `body_start` is the body's byte offset in its file, so returned spans refer to the
/// file rather than to the slice. Comments, at-rules, and statements that are not
/// custom-property declarations are skipped; a declaration with an empty value or a
/// non-`--` name is not a token.
///
/// # Errors
///
/// Returns an invalid-offset error when a span would exceed the 32-bit source limit.
pub(crate) fn extract_theme_declarations(
    file: FileId,
    body: &str,
    body_start: usize,
    mode: ThemeTokenMode,
) -> Result<Vec<ThemeToken>, CompileDiagnostic> {
    let end = body_start
        .checked_add(body.len())
        .ok_or(CompileDiagnostic::CssOffsetTooLarge { offset: body_start })?;
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
            } else if let Ok(close) = matching_brace(body, cursor + offset) {
                cursor = close + 1;
            } else {
                // An unclosed or unusable block ends the scan; the caller treats the
                // remaining text as declarations rather than guessing an end.
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
                name.to_lean_str(),
                Span::new(file, offset(name_start)?, offset(name_start + name.len())?),
            ),
            value: Spanned::new(
                value.to_lean_str(),
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

/// Classifies a custom-property name by theme namespace.
///
/// A bare namespace name is the namespace's scale, a namespaced name is a named token,
/// and anything else is custom: a custom property stays available as authored CSS but
/// creates no value for a built-in utility or variant.
fn classify_theme_token(name: &str) -> ThemeTokenKind {
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
        if let Some(token_name) =
            name.strip_prefix(format_args!("{namespace_name}-").to_lean_string().as_str())
        {
            return ThemeTokenKind::Token {
                namespace,
                name: token_name.to_lean_str(),
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
            Err(CompileDiagnostic::CssOffsetTooLarge { .. })
        ));
        assert!(matches!(
            extract_theme_declarations(
                FileId(1),
                "--x:red;",
                u32::MAX as usize,
                ThemeTokenMode::Default
            ),
            Err(CompileDiagnostic::CssOffsetTooLarge { .. })
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
