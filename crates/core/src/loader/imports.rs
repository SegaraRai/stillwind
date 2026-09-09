use super::{Import, Parser, Token};

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

/// Tokenization skips comments, strings and nested blocks, and decodes CSS escapes.
pub(super) fn imports(css: &str) -> Result<Vec<Import>, String> {
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
                    Ok::<_, cssparser::ParseError<()>>(())
                })
                .map_err(|error| format!("invalid CSS block: {error:?}"))?;
            continue;
        }
        if !matches!(token, Token::AtKeyword(name) if name.eq_ignore_ascii_case("import")) {
            continue;
        }
        let specifier = parser
            .expect_url_or_string()
            .map_err(|error| format!("invalid @import: {error:?}"))?
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
        let wrappers = import_wrappers(tail.strip_suffix(';').unwrap_or(tail))?;
        imports.push(Import {
            start,
            end: parser.position().byte_index(),
            specifier,
            wrappers,
        });
    }
    Ok(imports)
}

fn import_wrappers(tail: &str) -> Result<Vec<String>, String> {
    let mut parser = Parser::new(tail);
    let mut wrappers = Vec::new();
    while !parser.is_exhausted() {
        let state = parser.state();
        match parser
            .next()
            .map_err(|error| format!("invalid import modifiers: {error:?}"))?
        {
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
                        Ok::<_, cssparser::ParseError<()>>(
                            nested.slice_from(start).trim().to_owned(),
                        )
                    })
                    .map_err(|error| format!("invalid import modifier: {error:?}"))?;
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
