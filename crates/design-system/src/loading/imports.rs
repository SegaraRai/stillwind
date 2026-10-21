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
//! order. Tailwind's modifiers that change how the imported file is interpreted
//! (`theme`, `prefix`, `source`, `reference`, `important`) and its top-level `@config`
//! statement are skipped and recorded as [`IgnoredTailwindFeature`]s, which stylesheet
//! lint reports.

use std::ops::Range;

use cssparser::{ParseError, Parser, Token};
use lean_string::{LeanStr, ToLeanStr, ToLeanString};

use stillwind_css::{ParsedDeclaration, parse_declaration_list};
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
    pub(super) plugin: Option<Vec<ParsedDeclaration>>,
}

/// A Tailwind feature in a stylesheet that the loader ignores.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IgnoredTailwindFeature {
    /// Byte range of the `@config` statement, including its semicolon, or of the import
    /// modifier, including its arguments.
    pub range: Range<usize>,
    /// What the range contains.
    pub kind: IgnoredTailwindFeatureKind,
}

/// Kind of an [`IgnoredTailwindFeature`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IgnoredTailwindFeatureKind {
    /// A top-level `@config` statement. Expansion removes it.
    Config,
    /// An import modifier that changes how Tailwind interprets the imported file. The
    /// import is loaded without it.
    ImportModifier(
        /// Modifier name in lowercase: `important`, `prefix`, `reference`, `source`, or
        /// `theme`.
        LeanStr,
    ),
}

/// Lists the Tailwind features of one buffer that the loader ignores, in source order.
///
/// Returns an empty list when the loader rejects the top-level CSS of `css`, because
/// the load then fails with that error.
#[must_use]
pub fn ignored_tailwind_features(css: &str) -> Vec<IgnoredTailwindFeature> {
    scan(css).map(|scan| scan.ignored).unwrap_or_default()
}

/// The dependencies of one buffer and the Tailwind features that loading ignores.
#[derive(Debug, Default)]
pub(super) struct Scan {
    /// `@import` and `@plugin` statements in source order.
    pub(super) imports: Vec<Import>,
    /// Ignored features in source order.
    pub(super) ignored: Vec<IgnoredTailwindFeature>,
}

impl Scan {
    /// Returns the byte ranges of the `@config` statements, which expansion removes, in
    /// source order.
    pub(super) fn removed(&self) -> Vec<Range<usize>> {
        self.ignored
            .iter()
            .filter(|feature| feature.kind == IgnoredTailwindFeatureKind::Config)
            .map(|feature| feature.range.clone())
            .collect()
    }
}

/// Finds every authored `@import`, `@plugin`, and `@config` at the top level of one
/// buffer, in source order. Blocks and function arguments are consumed as units, so an
/// `@import` inside a rule or a parenthesized condition is body text, not a dependency.
/// Unparsable top-level CSS ends the scan with the statements found so far.
///
/// # Errors
///
/// Returns [`CompileDiagnostic::InvalidCssBlock`] for an unbalanced block,
/// [`CompileDiagnostic::InvalidCssImportSpecifier`] for an invalid import target, or
/// modifier errors from [`import_wrappers`]. Offsets refer to `css`. This layer does
/// not attach source labels or names.
pub(super) fn scan(css: &str) -> Result<Scan, CompileDiagnostic> {
    let mut parser = Parser::new(css);
    let mut scan = Scan::default();
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
        if matches!(token, Token::AtKeyword(name) if name.eq_ignore_ascii_case("config")) {
            // A block, which `@config` never has, ends the statement as well, so the
            // removed range cannot swallow the rules after it.
            while let Ok(token) = parser.next_including_whitespace_and_comments() {
                if matches!(token, Token::Semicolon | Token::CurlyBracketBlock) {
                    break;
                }
            }
            scan.ignored.push(IgnoredTailwindFeature {
                range: start..parser.position().byte_index(),
                kind: IgnoredTailwindFeatureKind::Config,
            });
            continue;
        }
        if matches!(token, Token::AtKeyword(name) if name.eq_ignore_ascii_case("plugin")) {
            scan.imports.push(plugin(&mut parser, start)?);
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
            &mut scan.ignored,
        )?;
        scan.imports.push(Import {
            start,
            end: parser.position().byte_index(),
            specifier: (specifier).into(),
            wrappers,
            plugin: None,
        });
    }
    Ok(scan)
}

/// Reads the rest of a `@plugin "specifier";` statement or a
/// `@plugin "specifier" { options }` rule after its keyword. An absent block gives
/// empty options.
///
/// # Errors
///
/// Returns [`CompileDiagnostic::InvalidCssImportSpecifier`] for a missing string
/// specifier, for other tokens before the semicolon or block, and for an options block
/// with a statement that is not a `name: value` declaration. Returns
/// [`CompileDiagnostic::InvalidCssBlock`] for an unbalanced block.
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
            let block = parser
                .parse_nested_block(|nested| {
                    let block_start = nested.position();
                    while nested.next_including_whitespace_and_comments().is_ok() {}
                    Ok::<_, ParseError<()>>(nested.slice_from(block_start))
                })
                .map_err(|_| CompileDiagnostic::InvalidCssBlock { offset: start })?;
            parse_declaration_list(block)
                .into_iter()
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| invalid(&error.to_lean_str()))?
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

/// Turns the text after a specifier into the wrapper preludes expansion emits: at most
/// one `layer`, then `supports`, then the media query, in that order. A misplaced or
/// repeated modifier is reported rather than reordered, and the first unrecognized
/// token starts the media prelude, which is kept verbatim. Tailwind's `important`,
/// `prefix`, `reference`, `source`, and `theme` modifiers add no wrapper, and each is
/// appended to `ignored`.
///
/// `base_offset` locates `tail` in the buffer, so error and `ignored` offsets are
/// absolute.
///
/// # Errors
///
/// Returns [`CompileDiagnostic::InvalidCssImportModifierOrder`] for a misplaced or repeated
/// modifier, and [`CompileDiagnostic::InvalidCssImportModifiers`] or
/// [`CompileDiagnostic::InvalidCssImportModifierBlock`] when a modifier cannot be tokenized
/// or its block is malformed.
fn import_wrappers(
    tail: &str,
    base_offset: usize,
    ignored: &mut Vec<IgnoredTailwindFeature>,
) -> Result<Vec<LeanStr>, CompileDiagnostic> {
    let mut parser = Parser::new(tail);
    let mut wrappers: Vec<LeanStr> = Vec::new();
    let mut supports_seen = false;
    while !parser.is_exhausted() {
        parser.skip_whitespace();
        let state = parser.state();
        let token = parser
            .next()
            .map_err(|error| CompileDiagnostic::InvalidCssImportModifiers {
                offset: base_offset + state.position().byte_index(),
                detail: error.kind.to_lean_str(),
            })?
            .clone();
        match &token {
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
                if ["important", "prefix", "reference", "source", "theme"]
                    .iter()
                    .any(|modifier| name.eq_ignore_ascii_case(modifier)) =>
            {
                if matches!(token, Token::Function(_)) {
                    // The arguments of a function modifier are skipped with it.
                    let _ = parser.parse_nested_block(|nested| {
                        while nested.next_including_whitespace_and_comments().is_ok() {}
                        Ok::<_, ParseError<()>>(())
                    });
                }
                let name = name.to_ascii_lowercase().to_lean_str();
                ignored.push(IgnoredTailwindFeature {
                    range: base_offset + state.position().byte_index()
                        ..base_offset + parser.position().byte_index(),
                    kind: IgnoredTailwindFeatureKind::ImportModifier(name),
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

    fn imports(css: &str) -> Result<Vec<Import>, CompileDiagnostic> {
        scan(css).map(|scan| scan.imports)
    }

    fn wrappers(tail: &str, base_offset: usize) -> Result<Vec<LeanStr>, CompileDiagnostic> {
        import_wrappers(tail, base_offset, &mut Vec::new())
    }

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
    fn plugin_options_reject_statements_that_are_not_declarations() {
        assert!(matches!(
            imports("@plugin 'p' { prefix: i; .a { color: red; } }"),
            Err(CompileDiagnostic::InvalidCssImportSpecifier { offset: 0, .. })
        ));
    }

    fn option(name: &str, value: &str) -> ParsedDeclaration {
        ParsedDeclaration {
            name: name.into(),
            value: value.into(),
        }
    }

    #[test]
    fn modifier_wrappers_preserve_layer_supports_and_media_order() {
        assert_eq!(
            wrappers("layer(foo) supports(display:grid) screen", 0).unwrap(),
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
            wrappers("supports(selector(:has(*)))", 0).unwrap(),
            vec!["@supports selector(:has(*))"]
        );
        assert_eq!(
            wrappers("supports(not (display:grid))", 0).unwrap(),
            vec!["@supports not (display:grid)"]
        );
        assert_eq!(
            wrappers(
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
        assert_eq!(wrappers("layer", 0).unwrap(), vec!["@layer"]);
        for invalid in [
            "layer()",
            "layer(a b)",
            "layer(a) layer(b)",
            "supports(display:grid) layer(a)",
            "supports()",
            "supports(display:grid) supports(color:red)",
        ] {
            assert!(wrappers(invalid, 0).is_err(), "{invalid}");
        }
        let framework = imports("@import 'stillwind' supports(display:grid);").unwrap();
        assert_eq!(framework[0].specifier, "stillwind");
        assert_eq!(framework[0].wrappers, vec!["@supports (display:grid)"]);
    }

    #[test]
    fn tailwind_config_and_interpretation_modifiers_are_recorded_and_skipped() {
        let css = "@import 'tailwindcss' layer(a) theme(static) important; @config './tw.js'; .a{}";
        let scan = scan(css).unwrap();
        assert_eq!(scan.imports[0].wrappers, vec!["@layer a"]);
        let ignored = scan
            .ignored
            .iter()
            .map(|feature| (&css[feature.range.clone()], feature.kind.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            ignored,
            [
                (
                    "theme(static)",
                    IgnoredTailwindFeatureKind::ImportModifier("theme".into())
                ),
                (
                    "important",
                    IgnoredTailwindFeatureKind::ImportModifier("important".into())
                ),
                ("@config './tw.js';", IgnoredTailwindFeatureKind::Config),
            ]
        );
        assert_eq!(scan.removed(), [scan.ignored[2].range.clone()]);
        assert_eq!(
            wrappers("prefix(tw) reference source(none) print", 0).unwrap(),
            vec!["@media print"]
        );
        assert_eq!(
            ignored_tailwind_features("@import 42; @config 'x';"),
            [] as [IgnoredTailwindFeature; 0]
        );
    }
}
