//! Lexical discovery of `@import` statements and their conditional wrappers.
//!
//! Only the authored spelling is interpreted here: which name a specifier denotes
//! belongs to the host. Tokenization skips comments, strings, and nested blocks, and
//! decodes CSS escapes, so `"@import 'x';"` inside a string or a prelude is not a
//! dependency.
//!
//! Classless CSS import modifiers are normalized into the wrapper at-rule text that
//! expansion emits: `layer(...)`, `supports(...)`, and a trailing media query, in that
//! order. Modifiers that would change how the imported file is interpreted in its own
//! right (`theme`, `prefix`, `source`, `reference`, `important`) are rejected instead
//! of being silently ignored.

use cssparser::{ParseError, Parser, Token};

use stillwind_diagnostics::CompileErrorKind;

/// One authored import: its byte range in the buffer, its decoded specifier, and the
/// wrappers derived from its modifiers.
#[derive(Clone, Debug)]
pub(super) struct Import {
    /// Byte offset where the statement starts in the buffer: the first byte of
    /// the token run that ends at the `@import`, so whitespace and comments
    /// before the keyword are included in the replaced range.
    pub(super) start: usize,
    /// Byte offset just past the statement, including the semicolon.
    pub(super) end: usize,
    /// Decoded specifier text, which the host resolves.
    pub(super) specifier: String,
    /// Wrapper at-rule preludes without braces, outermost first; expansion
    /// nests them in this order, and an empty vector means no condition.
    pub(super) wrappers: Vec<String>,
}

/// Finds every authored `@import` at the top level of one buffer, in source
/// order. Blocks and function arguments are consumed as units, so an `@import`
/// inside a rule or a parenthesized condition is body text, not a dependency.
/// Unparsable top-level CSS ends the scan with the imports found so far.
///
/// # Errors
///
/// Returns [`CompileErrorKind::InvalidImportBlock`] for an unbalanced block, and the
/// [`CompileErrorKind::InvalidImportSpecifier`] for an invalid import target, and
/// modifier errors from [`import_wrappers`]. Offsets refer to `css`; this layer does
/// not attach source labels or names.
pub(super) fn imports(css: &str) -> Result<Vec<Import>, CompileErrorKind> {
    let mut parser = Parser::new(css);
    let mut imports = Vec::new();
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            break;
        };
        if matches!(
            token,
            Token::CurlyBracketBlock
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::Function(_)
        ) {
            parser
                .parse_nested_block(|nested| {
                    while nested.next_including_whitespace_and_comments().is_ok() {}
                    Ok::<_, ParseError<()>>(())
                })
                .map_err(|error| CompileErrorKind::InvalidImportBlock {
                    offset: start,
                    detail: format!("{:?}", error.kind),
                })?;
            continue;
        }
        if !matches!(token, Token::AtKeyword(name) if name.eq_ignore_ascii_case("import")) {
            continue;
        }
        let specifier = parser
            .expect_url_or_string()
            .map_err(|error| CompileErrorKind::InvalidImportSpecifier {
                offset: start,
                detail: error.kind.to_string(),
            })?
            .to_string();
        let tail_start = parser.position();
        loop {
            match parser.next_including_whitespace_and_comments() {
                Ok(Token::Semicolon) => {
                    break;
                }
                Ok(_) => (),
                Err(_) => break,
            }
        }
        let tail = parser.slice(tail_start..parser.position());
        let wrappers = import_wrappers(
            tail.strip_suffix(';').unwrap_or(tail),
            tail_start.byte_index(),
        )?;
        imports.push(Import {
            start,
            end: parser.position().byte_index(),
            specifier,
            wrappers,
        });
    }
    Ok(imports)
}

/// Turns the text after a specifier into the wrapper preludes expansion emits:
/// at most one `layer`, then `supports`, then the media query, in that order.
/// A misplaced or repeated modifier is reported rather than reordered, and the
/// first unrecognized token starts the media prelude, which is kept verbatim.
///
/// `base_offset` locates `tail` in the buffer, so error offsets are absolute.
///
/// # Errors
///
/// Returns [`CompileErrorKind::InvalidImportModifierOrder`] for a misplaced or repeated
/// modifier, [`CompileErrorKind::UnsupportedImportModifier`] for a modifier that would
/// change how the imported file is interpreted in its own right, and
/// [`CompileErrorKind::InvalidImportModifiers`] or [`CompileErrorKind::InvalidImportModifierBlock`]
/// when a modifier cannot be tokenized or its block is malformed.
fn import_wrappers(tail: &str, base_offset: usize) -> Result<Vec<String>, CompileErrorKind> {
    let mut parser = Parser::new(tail);
    let mut wrappers = Vec::new();
    let mut supports_seen = false;
    while !parser.is_exhausted() {
        let state = parser.state();
        match parser
            .next()
            .map_err(|error| CompileErrorKind::InvalidImportModifiers {
                offset: base_offset + state.position().byte_index(),
                detail: error.kind.to_string(),
            })? {
            Token::Ident(name) if name.eq_ignore_ascii_case("layer") => {
                if !wrappers.is_empty() {
                    return Err(CompileErrorKind::InvalidImportModifierOrder {
                        offset: base_offset + state.position().byte_index(),
                    });
                }
                wrappers.push("@layer".into());
            }
            Token::Function(name)
                if name.eq_ignore_ascii_case("layer") || name.eq_ignore_ascii_case("supports") =>
            {
                let layer = name.eq_ignore_ascii_case("layer");
                if (layer && !wrappers.is_empty()) || (!layer && supports_seen) {
                    return Err(CompileErrorKind::InvalidImportModifierOrder {
                        offset: base_offset + state.position().byte_index(),
                    });
                }
                supports_seen |= !layer;
                let condition = parser
                    .parse_nested_block(|nested| {
                        let start = nested.position();
                        if layer {
                            nested.expect_ident()?;
                            while !nested.is_exhausted() {
                                nested.expect_delim('.')?;
                                nested.expect_ident()?;
                            }
                        } else {
                            nested.next()?;
                            while nested.next_including_whitespace_and_comments().is_ok() {}
                        }
                        Ok::<_, ParseError<()>>(nested.slice_from(start).trim().to_owned())
                    })
                    .map_err(|error| CompileErrorKind::InvalidImportModifierBlock {
                        offset: base_offset + state.position().byte_index(),
                        detail: format!("{:?}", error.kind),
                    })?;
                wrappers.push(if layer {
                    format!("@layer {condition}")
                } else {
                    let mut declaration = Parser::new(&condition);
                    if declaration
                        .try_parse(|p| {
                            p.expect_ident()?;
                            p.expect_colon()
                        })
                        .is_ok()
                    {
                        format!("@supports ({condition})")
                    } else {
                        format!("@supports {condition}")
                    }
                });
            }
            Token::Function(name) | Token::Ident(name)
                if ["prefix", "reference", "source", "theme", "important"]
                    .iter()
                    .any(|modifier| name.eq_ignore_ascii_case(modifier)) =>
            {
                return Err(CompileErrorKind::UnsupportedImportModifier {
                    offset: base_offset + state.position().byte_index(),
                    modifier: name.to_string(),
                });
            }
            _ => {
                wrappers.push(format!(
                    "@media {}",
                    tail[state.position().byte_index()..].trim()
                ));
                break;
            }
        }
    }
    Ok(wrappers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_ignore_comments_strings_and_nested_rules_but_resolve_all_packages() {
        let css = "/* @import 'no'; */ .x{content:\"@import 'no';\";} @import 'stillwind'; @import 'a.css';";
        let found = imports(css).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].specifier, "stillwind");
        assert_eq!(found[1].specifier, "a.css");
        assert_eq!(&css[found[1].start..found[1].end], "@import 'a.css';");
        assert!(matches!(
            imports("@import 42;"),
            Err(CompileErrorKind::InvalidImportSpecifier { .. })
        ));
    }

    #[test]
    fn modifier_wrappers_preserve_layer_supports_and_media_order() {
        assert_eq!(
            import_wrappers("layer(foo) supports(display:grid) screen", 0).unwrap(),
            vec!["@layer foo", "@supports (display:grid)", "@media screen"]
        );
        assert_eq!(
            imports("@import 'tailwindcss'; @import 'tailwindcss-custom.css';")
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn supports_conditions_keep_function_and_boolean_syntax() {
        assert_eq!(
            import_wrappers("supports(selector(:has(*)))", 0).unwrap(),
            vec!["@supports selector(:has(*))"]
        );
        assert_eq!(
            import_wrappers("supports(not (display:grid))", 0).unwrap(),
            vec!["@supports not (display:grid)"]
        );
        assert_eq!(
            import_wrappers(
                "layer(a.b) supports((display:grid) and (color:red)) print",
                0
            )
            .unwrap(),
            vec![
                "@layer a.b",
                "@supports (display:grid) and (color:red)",
                "@media print"
            ]
        );
        assert_eq!(import_wrappers("layer", 0).unwrap(), vec!["@layer"]);
        for invalid in [
            "layer()",
            "layer(a b)",
            "layer(a) layer(b)",
            "supports(display:grid) layer(a)",
            "supports()",
            "supports(display:grid) supports(color:red)",
        ] {
            assert!(import_wrappers(invalid, 0).is_err(), "{invalid}");
        }
        for unsupported in ["prefix(tw)", "reference", "source(none)"] {
            assert!(matches!(
                import_wrappers(unsupported, 0),
                Err(CompileErrorKind::UnsupportedImportModifier { .. })
            ));
        }
        let framework = imports("@import 'stillwind' supports(display:grid);").unwrap();
        assert_eq!(framework[0].specifier, "stillwind");
        assert_eq!(framework[0].wrappers, vec!["@supports (display:grid)"]);
    }
}
