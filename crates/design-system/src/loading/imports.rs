//! Lexical discovery of `@import` and `@plugin` statements and the conditional wrappers
//! of imports.
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

use cssparser::{ParseError, ParseErrorKind, Parser, Token};
use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_diagnostics::CompileDiagnostic;

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
    pub(super) specifier: LeanStr,
    /// Wrapper at-rule preludes without braces, outermost first; expansion
    /// nests them in this order, and an empty vector means no condition.
    pub(super) wrappers: Vec<LeanStr>,
    /// Plugin options when the statement is `@plugin`, or `None` for `@import`.
    pub(super) plugin: Option<Vec<PluginOption>>,
}

/// One `name: value;` declaration of a `@plugin` options block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginOption {
    /// Option name with CSS escapes decoded.
    pub name: LeanStr,
    /// Authored value text without the semicolon, trimmed. Each run of whitespace and
    /// comments between top-level tokens becomes one space. Strings and the contents of
    /// parentheses, brackets, and functions keep their authored text, comments
    /// included.
    pub value: LeanStr,
}

/// Finds every authored `@import` at the top level of one buffer, in source
/// order. Blocks and function arguments are consumed as units, so an `@import`
/// inside a rule or a parenthesized condition is body text, not a dependency.
/// Unparsable top-level CSS ends the scan with the imports found so far.
///
/// # Errors
///
/// Returns [`CompileDiagnostic::InvalidCssBlock`] for an unbalanced block,
/// [`CompileDiagnostic::InvalidCssImportSpecifier`] for an invalid import target, or
/// modifier errors from [`import_wrappers`]. Offsets refer to `css`. This layer does
/// not attach source labels or names.
pub(super) fn imports(css: &str) -> Result<Vec<Import>, CompileDiagnostic> {
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
                .map_err(|_| CompileDiagnostic::InvalidCssBlock { offset: start })?;
            continue;
        }
        if matches!(token, Token::AtKeyword(name) if name.eq_ignore_ascii_case("plugin")) {
            imports.push(plugin(&mut parser, start)?);
            continue;
        }
        if !matches!(token, Token::AtKeyword(name) if name.eq_ignore_ascii_case("import")) {
            continue;
        }
        let specifier = parser
            .expect_url_or_string()
            .map_err(|error| CompileDiagnostic::InvalidCssImportSpecifier {
                offset: start,
                detail: error.kind.to_lean_str(),
            })?
            .to_lean_string();
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
            specifier: (specifier).into(),
            wrappers,
            plugin: None,
        });
    }
    Ok(imports)
}

/// Reads the rest of a `@plugin "specifier";` statement or a
/// `@plugin "specifier" { options }` rule after its keyword. An absent block gives
/// empty options.
///
/// # Errors
///
/// Returns [`CompileDiagnostic::InvalidCssImportSpecifier`] for a missing string
/// specifier, for other tokens before the semicolon or block, and for an options block
/// that [`plugin_options`] rejects. Returns [`CompileDiagnostic::InvalidCssBlock`] for
/// an unbalanced block.
fn plugin(parser: &mut Parser<'_>, start: usize) -> Result<Import, CompileDiagnostic> {
    let invalid = |detail: &str| CompileDiagnostic::InvalidCssImportSpecifier {
        offset: start,
        detail: detail.into(),
    };
    let specifier = parser
        .expect_string()
        .map_err(|error| invalid(&error.kind.to_lean_str()))?
        .to_lean_string();
    let options = match parser.next() {
        Ok(Token::Semicolon) | Err(_) => Vec::new(),
        Ok(Token::CurlyBracketBlock) => {
            parser
                .parse_nested_block(plugin_options)
                .map_err(|error| match error.kind {
                    ParseErrorKind::Custom(detail) => invalid(detail),
                    ParseErrorKind::Basic(_) => {
                        CompileDiagnostic::InvalidCssBlock { offset: start }
                    }
                })?
        }
        Ok(_) => return Err(invalid("expected `;` or an options block after the plugin")),
    };
    Ok(Import {
        start,
        end: parser.position().byte_index(),
        specifier: specifier.into(),
        wrappers: Vec::new(),
        plugin: Some(options),
    })
}

/// Reads the `name: value;` declarations of a `@plugin` options block in authored
/// order. Empty statements are skipped, and the last declaration may omit its
/// semicolon.
///
/// # Errors
///
/// Returns a custom error with its explanation when a statement does not start with an
/// identifier and a colon, when a value is empty, or when a value contains a `{}`
/// block.
fn plugin_options(parser: &mut Parser<'_>) -> Result<Vec<PluginOption>, ParseError<&'static str>> {
    let invalid = |detail| ParseError {
        kind: ParseErrorKind::Custom(detail),
    };
    let mut options = Vec::new();
    loop {
        let name = match parser.next() {
            Err(_) => return Ok(options),
            Ok(Token::Semicolon) => continue,
            Ok(Token::Ident(name)) => name.to_lean_str(),
            Ok(_) => return Err(invalid("expected a plugin option name")),
        };
        if parser.expect_colon().is_err() {
            return Err(invalid("expected `:` after a plugin option name"));
        }
        let mut value = LeanString::new();
        loop {
            let token_start = parser.position();
            let block = match parser.next_including_whitespace_and_comments() {
                Err(_) | Ok(Token::Semicolon) => break,
                Ok(Token::WhiteSpace(_) | Token::Comment(_)) => {
                    if !value.is_empty() && !value.ends_with(' ') {
                        value.push(' ');
                    }
                    continue;
                }
                Ok(Token::CurlyBracketBlock) => {
                    return Err(invalid("plugin option values cannot contain `{}`"));
                }
                Ok(Token::ParenthesisBlock | Token::SquareBracketBlock | Token::Function(_)) => {
                    true
                }
                Ok(_) => false,
            };
            if block {
                parser.parse_nested_block(|nested| {
                    while nested.next_including_whitespace_and_comments().is_ok() {}
                    Ok::<_, ParseError<&'static str>>(())
                })?;
            }
            value.push_str(parser.slice_from(token_start));
        }
        let value = value.trim_end();
        if value.is_empty() {
            return Err(invalid("expected a plugin option value"));
        }
        options.push(PluginOption {
            name,
            value: value.to_lean_str(),
        });
    }
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
/// Returns [`CompileDiagnostic::InvalidCssImportModifierOrder`] for a misplaced or repeated
/// modifier, [`CompileDiagnostic::UnsupportedCssImportModifier`] for a modifier that would
/// change how the imported file is interpreted in its own right, and
/// [`CompileDiagnostic::InvalidCssImportModifiers`] or [`CompileDiagnostic::InvalidCssImportModifierBlock`]
/// when a modifier cannot be tokenized or its block is malformed.
fn import_wrappers(tail: &str, base_offset: usize) -> Result<Vec<LeanStr>, CompileDiagnostic> {
    let mut parser = Parser::new(tail);
    let mut wrappers: Vec<LeanStr> = Vec::new();
    let mut supports_seen = false;
    while !parser.is_exhausted() {
        let state = parser.state();
        match parser
            .next()
            .map_err(|error| CompileDiagnostic::InvalidCssImportModifiers {
                offset: base_offset + state.position().byte_index(),
                detail: error.kind.to_lean_str(),
            })? {
            Token::Ident(name) if name.eq_ignore_ascii_case("layer") => {
                if !wrappers.is_empty() {
                    return Err(CompileDiagnostic::InvalidCssImportModifierOrder {
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
                    return Err(CompileDiagnostic::InvalidCssImportModifierOrder {
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
                        Ok::<_, ParseError<()>>(nested.slice_from(start).trim().to_lean_string())
                    })
                    .map_err(|error| CompileDiagnostic::InvalidCssImportModifierBlock {
                        offset: base_offset + state.position().byte_index(),
                        detail: format_args!("{:?}", error.kind).to_lean_str(),
                    })?;
                wrappers.push(if layer {
                    format_args!("@layer {condition}").to_lean_str()
                } else {
                    let mut declaration = Parser::new(&condition);
                    if declaration
                        .try_parse(|p| {
                            p.expect_ident()?;
                            p.expect_colon()
                        })
                        .is_ok()
                    {
                        format_args!("@supports ({condition})").to_lean_str()
                    } else {
                        format_args!("@supports {condition}").to_lean_str()
                    }
                });
            }
            Token::Function(name) | Token::Ident(name)
                if ["prefix", "reference", "source", "theme", "important"]
                    .iter()
                    .any(|modifier| name.eq_ignore_ascii_case(modifier)) =>
            {
                return Err(CompileDiagnostic::UnsupportedCssImportModifier {
                    offset: base_offset + state.position().byte_index(),
                    modifier: name.to_lean_str(),
                });
            }
            _ => {
                wrappers.push(
                    format_args!("@media {}", tail[state.position().byte_index()..].trim())
                        .to_lean_str(),
                );
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
        let css = "/* @import 'no'; */ .x{content:\"@import 'no';\";} @import 'stillwind'; @import 'a.css'; @import url(\"b.css\");";
        let found = imports(css).unwrap();
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].specifier, "stillwind");
        assert_eq!(found[1].specifier, "a.css");
        assert_eq!(found[2].specifier, "b.css");
        assert_eq!(&css[found[1].start..found[1].end], "@import 'a.css';");
        assert!(matches!(
            imports("@import 42;"),
            Err(CompileDiagnostic::InvalidCssImportSpecifier { .. })
        ));
    }

    #[test]
    fn plugins_are_dependencies_with_their_options() {
        let css = ".a{} @plugin \"@stillwind/plugin-iconify\"; @plugin 'local' { prefix: i; scale: 1.2; } .b{}";
        let found = imports(css).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].specifier, "@stillwind/plugin-iconify");
        assert_eq!(found[0].plugin.as_deref(), Some(&[][..]));
        assert_eq!(
            &css[found[0].start..found[0].end],
            "@plugin \"@stillwind/plugin-iconify\";"
        );
        assert_eq!(
            found[1].plugin.as_deref(),
            Some(&[option("prefix", "i"), option("scale", "1.2")][..])
        );
        assert!(css[found[1].start..found[1].end].ends_with("scale: 1.2; }"));
        assert!(imports("@plugin;").is_err());
        assert!(imports("@plugin 'a' b;").is_err());
    }

    #[test]
    fn plugin_options_keep_quoted_delimiters_and_omit_comments() {
        let css = r#"@plugin "p" {
            /* note: ignored; */
            label: "a;b: c";
            icon: url("data:image/svg+xml;utf8,<svg/>");
            size: 1 /* inline */ 2;
            list: fn(a; b), [c];
            last: x
        }"#;
        let found = imports(css).unwrap();
        assert_eq!(
            found[0].plugin.as_deref(),
            Some(
                &[
                    option("label", "\"a;b: c\""),
                    option("icon", "url(\"data:image/svg+xml;utf8,<svg/>\")"),
                    option("size", "1 2"),
                    option("list", "fn(a; b), [c]"),
                    option("last", "x"),
                ][..]
            )
        );
        for invalid in [
            "@plugin 'p' { prefix }",
            "@plugin 'p' { prefix: ; }",
            "@plugin 'p' { 'prefix': i; }",
            "@plugin 'p' { .a { color: red; } }",
        ] {
            assert!(
                matches!(
                    imports(invalid),
                    Err(CompileDiagnostic::InvalidCssImportSpecifier { offset: 0, .. })
                ),
                "{invalid}"
            );
        }
    }

    fn option(name: &str, value: &str) -> PluginOption {
        PluginOption {
            name: name.into(),
            value: value.into(),
        }
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
                Err(CompileDiagnostic::UnsupportedCssImportModifier { .. })
            ));
        }
        let framework = imports("@import 'stillwind' supports(display:grid);").unwrap();
        assert_eq!(framework[0].specifier, "stillwind");
        assert_eq!(framework[0].wrappers, vec!["@supports (display:grid)"]);
    }
}
