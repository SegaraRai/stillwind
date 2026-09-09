use cssparser::{BasicParseErrorKind, ParseError, ParseErrorKind, Parser, Token};

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum CssImportError {
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
        if is_framework(&specifier) {
            continue;
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

fn import_wrappers(tail: &str, base_offset: usize) -> Result<Vec<String>, CssImportError> {
    let mut parser = Parser::new(tail);
    let mut wrappers = Vec::new();
    while !parser.is_exhausted() {
        let state = parser.state();
        match parser.next().map_err(|error| CssImportError::Modifiers {
            offset: base_offset + state.position().byte_index(),
            kind: error.kind,
        })? {
            Token::Ident(name) if name.eq_ignore_ascii_case("layer") => {
                wrappers.push("@layer".into());
            }
            Token::Function(name)
                if name.eq_ignore_ascii_case("layer") || name.eq_ignore_ascii_case("supports") =>
            {
                let layer = name.eq_ignore_ascii_case("layer");
                let condition = parser
                    .parse_nested_block(|nested| {
                        let start = nested.position();
                        while nested.next_including_whitespace_and_comments().is_ok() {}
                        Ok::<_, ParseError<()>>(nested.slice_from(start).trim().to_owned())
                    })
                    .map_err(|error| CssImportError::ModifierBlock {
                        offset: base_offset + state.position().byte_index(),
                        kind: error.kind,
                    })?;
                wrappers.push(if layer {
                    format!("@layer {condition}")
                } else {
                    format!("@supports ({condition})")
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
        "tailwindcss"
            | "tailwindcss/theme"
            | "tailwindcss/preflight"
            | "tailwindcss/utilities"
            | "stillwind"
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
        assert!(is_framework("tailwindcss"));
        assert!(!is_framework("tailwindcss-custom.css"));
    }
}
