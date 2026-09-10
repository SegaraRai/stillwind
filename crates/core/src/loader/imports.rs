use cssparser::{BasicParseErrorKind, ParseError, ParseErrorKind, Parser, Token};

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum CssImportError {
    #[error("unsupported import modifier at byte {offset}: {modifier}")]
    UnsupportedModifier { offset: usize, modifier: String },
    #[error("invalid import modifier order at byte {offset}")]
    ModifierOrder { offset: usize },
    #[error("invalid CSS block at byte {offset}: {kind:?}")]
    Block {
        offset: usize,
        kind: ParseErrorKind<()>,
    },
    #[error("invalid @import at byte {offset}: {kind}")]
    Specifier {
        offset: usize,
        kind: BasicParseErrorKind,
    },
    #[error("invalid import modifiers at byte {offset}: {kind}")]
    Modifiers {
        offset: usize,
        kind: BasicParseErrorKind,
    },
    #[error("invalid import modifier at byte {offset}: {kind:?}")]
    ModifierBlock {
        offset: usize,
        kind: ParseErrorKind<()>,
    },
}

#[derive(Clone, Debug)]
pub(super) struct Import {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) specifier: String,
    pub(super) wrappers: Vec<String>,
}

/// Tokenization skips comments, strings and nested blocks, and decodes CSS escapes.
pub(super) fn imports(css: &str) -> Result<Vec<Import>, CssImportError> {
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
                .map_err(|error| CssImportError::Block {
                    offset: start,
                    kind: error.kind,
                })?;
            continue;
        }
        if !matches!(token, Token::AtKeyword(name) if name.eq_ignore_ascii_case("import")) {
            continue;
        }
        let specifier = parser
            .expect_url_or_string()
            .map_err(|error| CssImportError::Specifier {
                offset: start,
                kind: error.kind,
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
        if is_framework(&specifier) {
            // The legacy framework selector can represent only a named layer.
            // Do not silently erase conditions while the CSS-entry migration
            // replaces that selector with ordinary resolved source files.
            if wrappers
                .iter()
                .any(|wrapper| !wrapper.starts_with("@layer "))
            {
                return Err(CssImportError::UnsupportedModifier {
                    offset: tail_start.byte_index(),
                    modifier: format!("conditional or anonymous-layer import of {specifier}"),
                });
            }
            continue;
        }
        imports.push(Import {
            start,
            end: parser.position().byte_index(),
            specifier,
            wrappers,
        });
    }
    Ok(imports)
}

fn import_wrappers(tail: &str, base_offset: usize) -> Result<Vec<String>, CssImportError> {
    let mut parser = Parser::new(tail);
    let mut wrappers = Vec::new();
    let mut supports_seen = false;
    while !parser.is_exhausted() {
        let state = parser.state();
        match parser.next().map_err(|error| CssImportError::Modifiers {
            offset: base_offset + state.position().byte_index(),
            kind: error.kind,
        })? {
            Token::Ident(name) if name.eq_ignore_ascii_case("layer") => {
                if !wrappers.is_empty() {
                    return Err(CssImportError::ModifierOrder {
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
                    return Err(CssImportError::ModifierOrder {
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
                    .map_err(|error| CssImportError::ModifierBlock {
                        offset: base_offset + state.position().byte_index(),
                        kind: error.kind,
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
                return Err(CssImportError::UnsupportedModifier {
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

fn is_framework(name: &str) -> bool {
    matches!(
        name,
        "stillwind"
            | "stillwind/theme.css"
            | "stillwind/preflight.css"
            | "stillwind/utilities.css"
            | "stillwind/builtins.css"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_ignore_comments_strings_nested_rules_and_framework_entries() {
        let css = "/* @import 'no'; */ .x{content:\"@import 'no';\";} @import 'stillwind'; @import 'a.css';";
        let found = imports(css).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].specifier, "a.css");
        assert_eq!(&css[found[0].start..found[0].end], "@import 'a.css';");
        assert!(matches!(
            imports("@import 42;"),
            Err(CssImportError::Specifier { .. })
        ));
    }

    #[test]
    fn modifier_wrappers_preserve_layer_supports_and_media_order() {
        assert_eq!(
            import_wrappers("layer(foo) supports(display:grid) screen", 0).unwrap(),
            vec!["@layer foo", "@supports (display:grid)", "@media screen"]
        );
        assert!(!is_framework("tailwindcss"));
        assert!(!is_framework("tailwindcss-custom.css"));
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
                Err(CssImportError::UnsupportedModifier { .. })
            ));
        }
        assert!(matches!(
            imports("@import 'stillwind' supports(display:grid);"),
            Err(CssImportError::UnsupportedModifier { .. })
        ));
    }
}
