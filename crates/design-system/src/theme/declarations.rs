//! Theme value extraction: which custom properties a `@theme` block declares, what each
//! one means, and where it was written.
//!
//! A declaration is read textually so its span can point at the authored name and value
//! inside the buffer under parse. The span math is explicit: offsets are validated
//! before arithmetic, so an overflowing base or a buffer beyond the 32-bit source limit
//! is an error rather than a wrapped coordinate.
//!
//! Extraction is syntax-level. Namespace resets and duplicate resolution are decided
//! later, because they depend on the whole token list rather than on one declaration.

use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{CssBanner, CssNodeKind, banner_blocks, parse_structure};
use stillwind_diagnostics::CompileDiagnostic;
use stillwind_source::{FileId, Span, Spanned};

use super::parser::offset;

/// The purpose of a custom property declared by `@theme`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ThemeValueRole {
    /// A design token available to theme namespaces, functions, and token policies.
    #[default]
    Token,
    /// A shared value referenced by CSS without joining theme-token selection.
    Asset,
}

impl ThemeValueRole {
    /// Returns `Asset` when `modifier` contains the `asset` word, and `Token`
    /// otherwise.
    pub fn parse(modifier: &str) -> Self {
        if modifier.split_whitespace().any(|word| word == "asset") {
            Self::Asset
        } else {
            Self::Token
        }
    }
}

/// How a `@theme` block's declarations are emitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeValueMode {
    /// Emitted when required and referenced by `var()`.
    Default,
    /// Inlined into the generated declaration instead of being emitted.
    Inline,
    /// Included in the required set without a reference; later resets and
    /// registry pruning can still remove it.
    Static,
}

impl ThemeValueMode {
    /// Reads a `@theme` block modifier; an unrecognized modifier is treated as the
    /// default mode rather than rejected.
    pub fn parse(modifier: &str) -> Self {
        let words = modifier.split_whitespace().collect::<Vec<_>>();
        if words.contains(&"inline") {
            Self::Inline
        } else if words.contains(&"static") {
            Self::Static
        } else {
            Self::Default
        }
    }
}

/// Which definition of a value wins, independent of source order. A definition replaces
/// an earlier one of the same or lower precedence. Resets remove values of their own
/// role when those values have the same or lower precedence.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum ThemeValuePrecedence {
    /// `@theme default`, which yields to every other definition, as in Tailwind.
    Default,
    /// Theme configuration of a Tailwind plugin, which the plugin adapter writes as
    /// `@stillwind-plugin-theme`. It replaces `@theme default` definitions and yields
    /// to ordinary ones.
    Plugin,
    /// Ordinary `@theme` definition.
    #[default]
    Ordinary,
}

impl ThemeValuePrecedence {
    /// Reads the precedence of a `@theme` block modifier, such as `default inline`.
    #[must_use]
    pub fn parse(modifier: &str) -> Self {
        if modifier.split_whitespace().any(|word| word == "default") {
            Self::Default
        } else {
            Self::Ordinary
        }
    }
}

/// One custom-property declaration from a `@theme` block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeValue {
    /// Notices required when this value contributes to output.
    pub banners: Vec<CssBanner>,
    /// Authored custom-property name with its span in the parsed buffer.
    pub name: Spanned<LeanStr>,
    /// Authored value with its span in the parsed buffer.
    pub value: Spanned<LeanStr>,
    /// Whether the definition participates in theme-token selection or stores an asset.
    pub role: ThemeValueRole,
    /// Emission policy: referenced value, inline substitution, or static retention.
    pub mode: ThemeValueMode,
    /// Which definition wins, independent of source order.
    pub precedence: ThemeValuePrecedence,
}

/// Reads the declarations of one `@theme` body.
///
/// `body_start` is the body's byte offset in its file, so returned spans refer to the
/// file rather than to the slice. Comments, at-rules, and statements that are not
/// custom-property declarations are skipped; a declaration with an empty value or a
/// non-`--` name is skipped. Definitions start with the token role and ordinary
/// precedence; the containing block's parser applies its role and precedence.
///
/// # Errors
///
/// Returns an invalid-offset error when a span would exceed the 32-bit source limit.
pub(crate) fn extract_theme_declarations(
    file: FileId,
    body: &str,
    body_start: usize,
    mode: ThemeValueMode,
) -> Result<Vec<ThemeValue>, CompileDiagnostic> {
    let end = body_start
        .checked_add(body.len())
        .ok_or(CompileDiagnostic::CssOffsetTooLarge { offset: body_start })?;
    offset(end)?;
    let banners: Vec<_> = banner_blocks(body)
        .map_err(|error| CompileDiagnostic::CssParse {
            detail: error.to_lean_str(),
        })?
        .into_iter()
        .map(|block| block.banner)
        .collect();
    let mut tokens = Vec::new();
    for node in parse_structure(body) {
        let CssNodeKind::Declaration {
            property,
            value: value_range,
        } = node.kind
        else {
            continue;
        };
        let name = &body[property.clone()];
        let value = &body[value_range.clone()];
        if !name.starts_with("--") || value.is_empty() {
            continue;
        }
        let name_start = body_start + property.start;
        let value_start = body_start + value_range.start;
        tokens.push(ThemeValue {
            banners: banners.clone(),
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
            mode,
            role: ThemeValueRole::Token,
            precedence: ThemeValuePrecedence::Ordinary,
        });
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_overflowing_base_offsets_before_token_arithmetic() {
        assert!(matches!(
            extract_theme_declarations(FileId(1), "--x:red;", usize::MAX, ThemeValueMode::Default),
            Err(CompileDiagnostic::CssOffsetTooLarge { .. })
        ));
        assert!(matches!(
            extract_theme_declarations(
                FileId(1),
                "--x:red;",
                u32::MAX as usize,
                ThemeValueMode::Default
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
            ThemeValueMode::Inline,
        )
        .unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].name.value, "--color-a");
        assert_eq!(tokens[0].name.span.start.0, 10);
        assert_eq!(tokens[0].value.value, "red");
        assert_eq!(tokens[0].mode, ThemeValueMode::Inline);
        assert_eq!(ThemeValueMode::parse("static"), ThemeValueMode::Static);
        assert_eq!(ThemeValueMode::parse("unknown"), ThemeValueMode::Default);
    }
}
