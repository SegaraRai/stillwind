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
//! (`theme`, `prefix`, `source`, `reference`, `important`) and its top-level `@config`,
//! `@source`, and `@reference` statements are skipped and recorded as
//! [`IgnoredTailwindFeature`]s, which stylesheet lint reports. An `@import` or
//! `@plugin` statement with an invalid specifier, options, or modifiers is removed from
//! the expansion and recorded as a skipped statement.

use std::ops::Range;

use cssparser::{ParseError, Parser, Token};
use lean_string::{LeanStr, ToLeanStr, ToLeanString};

use stillwind_css::{ParsedDeclaration, parse_declaration_list};
use stillwind_diagnostics::CompileDiagnostic;

use crate::SkipReason;
use crate::skipped::SkippedRange;

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
    /// Byte range of the statement, including its semicolon, or of the import modifier
    /// or `@theme` option, including its arguments.
    pub range: Range<usize>,
    /// What the range contains.
    pub kind: IgnoredTailwindFeatureKind,
}

/// Kind of an [`IgnoredTailwindFeature`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IgnoredTailwindFeatureKind {
    /// A top-level `@config` statement. Expansion removes it.
    Config,
    /// A top-level `@source` statement. Expansion removes it.
    Source,
    /// A top-level `@reference` statement. Expansion removes it without loading the
    /// file.
    Reference,
    /// An import modifier that changes how Tailwind interprets the imported file. The
    /// import is loaded without it.
    ImportModifier(
        /// Modifier name in lowercase: `important`, `prefix`, `reference`, `source`, or
        /// `theme`.
        LeanStr,
    ),
    /// A `prefix()` option of an `@theme` block. The block is loaded without it.
    ThemePrefix,
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
    /// Invalid `@import` and `@plugin` statements in source order, each with its whole
    /// statement range.
    pub(super) skipped: Vec<SkippedRange>,
}

impl Scan {
    /// Returns the byte ranges that expansion removes, in source order: the `@config`,
    /// `@source`, and `@reference` statements and the skipped import statements.
    pub(super) fn removed(&self) -> Vec<Range<usize>> {
        let mut removed = self
            .ignored
            .iter()
            .filter(|feature| {
                matches!(
                    feature.kind,
                    IgnoredTailwindFeatureKind::Config
                        | IgnoredTailwindFeatureKind::Source
                        | IgnoredTailwindFeatureKind::Reference
                )
            })
            .map(|feature| feature.range.clone())
            .chain(self.skipped.iter().map(|(range, _)| range.clone()))
            .collect::<Vec<_>>();
        removed.sort_by_key(|range| range.start);
        removed
    }
}

/// Finds every authored `@import`, `@plugin`, `@config`, `@source`, and `@reference` at
/// the top level of one buffer, in source order. Blocks and function arguments are
/// consumed as units, so an `@import` inside a rule or a parenthesized condition is
/// body text, not a dependency. Unparsable top-level CSS ends the scan with the
/// statements found so far.
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
        if let Some(kind) = ignored_statement(token) {
            // A block, which these statements never have, ends the statement as well,
            // so the removed range cannot swallow the rules after it.
            while let Ok(token) = parser.next_including_whitespace_and_comments() {
                if matches!(token, Token::Semicolon | Token::CurlyBracketBlock) {
                    break;
                }
            }
            scan.ignored.push(IgnoredTailwindFeature {
                range: start..parser.position().byte_index(),
                kind,
            });
            continue;
        }
        if matches!(token, Token::AtKeyword(name) if name.eq_ignore_ascii_case("theme")) {
            theme_prelude(&mut parser, start, &mut scan.ignored)?;
            continue;
        }
        if matches!(token, Token::AtKeyword(name) if name.eq_ignore_ascii_case("plugin")) {
            match plugin(&mut parser, start)? {
                Ok(import) => {
                    scan.imports.push(import);
                }
                Err(reason) => {
                    scan.skipped
                        .push((start..parser.position().byte_index(), reason));
                }
            }
            continue;
        }
        if !matches!(token, Token::AtKeyword(name) if name.eq_ignore_ascii_case("import")) {
            continue;
        }
        // A failed parse is rewound, so a misplaced `;` still ends this statement.
        let specifier = parser
            .try_parse(|parser| {
                parser
                    .expect_url_or_string()
                    .map(|specifier| specifier.to_lean_string())
            })
            .map_err(|error| SkipReason::ImportSpecifier {
                detail: error.kind.to_lean_str(),
            });
        let tail_start = parser.position();
        skip_statement(&mut parser);
        let tail = parser.slice(tail_start..parser.position());
        let end = parser.position().byte_index();
        let import = specifier.and_then(|specifier| {
            Ok(Import {
                start,
                end,
                specifier: specifier.into(),
                wrappers: import_wrappers(
                    tail.strip_suffix(';').unwrap_or(tail),
                    tail_start.byte_index(),
                    &mut scan.ignored,
                )?,
                plugin: None,
            })
        });
        match import {
            Ok(import) => {
                scan.imports.push(import);
            }
            Err(reason) => {
                scan.skipped.push((start..end, reason));
            }
        }
    }
    Ok(scan)
}

/// Returns the kind of the ignored Tailwind statement that `token` starts, if any.
fn ignored_statement(token: &Token<'_>) -> Option<IgnoredTailwindFeatureKind> {
    let Token::AtKeyword(name) = token else {
        return None;
    };
    if name.eq_ignore_ascii_case("config") {
        Some(IgnoredTailwindFeatureKind::Config)
    } else if name.eq_ignore_ascii_case("source") {
        Some(IgnoredTailwindFeatureKind::Source)
    } else if name.eq_ignore_ascii_case("reference") {
        Some(IgnoredTailwindFeatureKind::Reference)
    } else {
        None
    }
}

/// Consumes one `@theme` statement after its keyword and records its `prefix()`
/// options.
///
/// `start` is the statement's offset, which identifies an unbalanced block.
///
/// # Errors
///
/// Returns [`CompileDiagnostic::InvalidCssBlock`] for an unbalanced block.
fn theme_prelude(
    parser: &mut Parser<'_>,
    start: usize,
    ignored: &mut Vec<IgnoredTailwindFeature>,
) -> Result<(), CompileDiagnostic> {
    loop {
        let option = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            return Ok(());
        };
        let prefix = matches!(token, Token::Function(name) if name.eq_ignore_ascii_case("prefix"));
        let last = matches!(token, Token::CurlyBracketBlock | Token::Semicolon);
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
        }
        if prefix {
            ignored.push(IgnoredTailwindFeature {
                range: option..parser.position().byte_index(),
                kind: IgnoredTailwindFeatureKind::ThemePrefix,
            });
        }
        if last {
            return Ok(());
        }
    }
}

/// Consumes tokens through the next top-level semicolon or the end of input. A block
/// is consumed as one token.
fn skip_statement(parser: &mut Parser<'_>) {
    while let Ok(token) = parser.next_including_whitespace_and_comments() {
        if matches!(token, Token::Semicolon) {
            break;
        }
    }
}

/// Reads the rest of a `@plugin "specifier";` statement or a
/// `@plugin "specifier" { options }` rule after its keyword. An absent block gives
/// empty options.
///
/// The inner error explains an invalid statement: a missing string specifier, other
/// tokens before the semicolon or block, or an options block with a statement that is
/// not a `name: value` declaration. The parser is then past the whole statement, so the
/// caller can skip it.
///
/// # Errors
///
/// Returns [`CompileDiagnostic::InvalidCssBlock`] for an unbalanced block.
fn plugin(
    parser: &mut Parser<'_>,
    start: usize,
) -> Result<Result<Import, SkipReason>, CompileDiagnostic> {
    let invalid = |detail: &str| SkipReason::ImportSpecifier {
        detail: detail.into(),
    };
    // A failed parse is rewound, so a misplaced `;` still ends this statement.
    let specifier =
        match parser.try_parse(|parser| parser.expect_string().map(ToLeanString::to_lean_string)) {
            Ok(specifier) => specifier,
            Err(error) => {
                let reason = invalid(&error.kind.to_lean_str());
                skip_plugin_statement(parser, start)?;
                return Ok(Err(reason));
            }
        };
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
            match parse_declaration_list(block)
                .into_iter()
                .collect::<Result<Vec<_>, _>>()
            {
                Ok(options) => options,
                Err(error) => return Ok(Err(invalid(&error.to_lean_str()))),
            }
        }
        Ok(_) => {
            skip_plugin_statement(parser, start)?;
            return Ok(Err(invalid(
                "expected `;` or an options block after the plugin",
            )));
        }
    };
    Ok(Ok(Import {
        start,
        end: parser.position().byte_index(),
        specifier: specifier.into(),
        wrappers: Vec::new(),
        plugin: Some(options),
    }))
}

/// Consumes the rest of an invalid `@plugin` statement through its semicolon or its
/// options block.
///
/// # Errors
///
/// Returns [`CompileDiagnostic::InvalidCssBlock`] for an unbalanced block.
fn skip_plugin_statement(parser: &mut Parser<'_>, start: usize) -> Result<(), CompileDiagnostic> {
    loop {
        match parser.next_including_whitespace_and_comments() {
            Ok(Token::Semicolon) | Err(_) => {
                return Ok(());
            }
            Ok(Token::CurlyBracketBlock) => {
                return parser
                    .parse_nested_block(|nested| {
                        while nested.next_including_whitespace_and_comments().is_ok() {}
                        Ok::<_, ParseError<()>>(())
                    })
                    .map_err(|_| CompileDiagnostic::InvalidCssBlock { offset: start });
            }
            Ok(_) => {}
        }
    }
}

/// Turns the text after a specifier into the wrapper preludes expansion emits: at most
/// one `layer`, then `supports`, then the media query, in that order. A misplaced or
/// repeated modifier is reported rather than reordered, and the first unrecognized
/// token starts the media prelude, which is kept verbatim. Tailwind's `important`,
/// `prefix`, `reference`, `source`, and `theme` modifiers add no wrapper, and each is
/// appended to `ignored`.
///
/// `base_offset` locates `tail` in the buffer, so `ignored` offsets are absolute.
///
/// # Errors
///
/// Returns [`SkipReason::ImportModifiers`] for a misplaced or repeated modifier, a
/// modifier that cannot be tokenized, or a modifier with a malformed block.
fn import_wrappers(
    tail: &str,
    base_offset: usize,
    ignored: &mut Vec<IgnoredTailwindFeature>,
) -> Result<Vec<LeanStr>, SkipReason> {
    let mut parser = Parser::new(tail);
    let mut wrappers: Vec<LeanStr> = Vec::new();
    let mut supports_seen = false;
    while !parser.is_exhausted() {
        parser.skip_whitespace();
        let state = parser.state();
        let token = parser
            .next()
            .map_err(|error| SkipReason::ImportModifiers {
                detail: error.kind.to_lean_str(),
            })?
            .clone();
        match &token {
            Token::Ident(name) if name.eq_ignore_ascii_case("layer") => {
                if !wrappers.is_empty() {
                    return Err(SkipReason::ImportModifiers {
                        detail: "modifiers must be in the order `layer`, `supports`, media query"
                            .into(),
                    });
                }
                wrappers.push("@layer".into());
            }
            Token::Function(name)
                if name.eq_ignore_ascii_case("layer") || name.eq_ignore_ascii_case("supports") =>
            {
                let layer = name.eq_ignore_ascii_case("layer");
                if (layer && !wrappers.is_empty()) || (!layer && supports_seen) {
                    return Err(SkipReason::ImportModifiers {
                        detail: "modifiers must be in the order `layer`, `supports`, media query"
                            .into(),
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
                    .map_err(|error| SkipReason::ImportModifiers {
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
    use stillwind_diagnostics::LintKind;

    use super::*;

    fn imports(css: &str) -> Vec<Import> {
        scan(css).unwrap().imports
    }

    fn skipped(css: &str) -> Vec<SkippedRange> {
        scan(css).unwrap().skipped
    }

    fn wrappers(tail: &str, base_offset: usize) -> Result<Vec<LeanStr>, SkipReason> {
        import_wrappers(tail, base_offset, &mut Vec::new())
    }

    #[test]
    fn imports_ignore_comments_strings_and_nested_rules_but_resolve_all_packages() {
        let css = "/* @import 'no'; */ .x{content:\"@import 'no';\";} @import 'stillwind'; @import 'a.css'; @import url(\"b.css\");";
        let found = imports(css);
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].specifier, "stillwind");
        assert_eq!(found[1].specifier, "a.css");
        assert_eq!(found[2].specifier, "b.css");
        assert_eq!(&css[found[1].start..found[1].end], "@import 'a.css';");
    }

    #[test]
    fn invalid_imports_are_skipped_and_removed_from_the_expansion() {
        for css in [
            "@import 42; .a{}",
            "@import; .a{}",
            "@import 'a.css' layer(x) layer(y); .a{}",
            "@import 'a.css' supports(); .a{}",
        ] {
            let scan = scan(css).unwrap();
            assert!(scan.imports.is_empty(), "{css}");
            let [(range, reason)] = scan.skipped.as_slice() else {
                panic!("{css}: {:?}", scan.skipped);
            };
            assert_eq!(reason.lint_kind(), LintKind::InvalidCssImport, "{css}");
            assert!(css[range.clone()].starts_with("@import"), "{css}");
            assert_eq!(scan.removed(), std::slice::from_ref(range), "{css}");
        }
        // A misplaced `;` ends the invalid statement, so the next import still counts.
        assert_eq!(imports("@import; @import 'b.css';").len(), 1);
    }

    #[test]
    fn plugins_are_dependencies_with_their_options() {
        let css = ".a{} @plugin \"@stillwind/plugin-iconify\"; @plugin 'local' { prefix: i; scale: 1.2; } .b{}";
        let found = imports(css);
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
        for css in ["@plugin;", "@plugin 'a' b;"] {
            assert!(imports(css).is_empty(), "{css}");
            assert!(
                matches!(skipped(css).as_slice(), [(range, SkipReason::ImportSpecifier { .. })] if *range == (0..css.len())),
                "{css}"
            );
        }
    }

    #[test]
    fn plugin_options_reject_statements_that_are_not_declarations() {
        let css = "@plugin 'p' { prefix: i; .a { color: red; } } .b{}";
        assert!(imports(css).is_empty());
        assert!(matches!(
            skipped(css).as_slice(),
            [(range, SkipReason::ImportSpecifier { .. })] if css[range.clone()].ends_with('}') && range.start == 0
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
            imports("@import 'tailwindcss'; @import 'tailwindcss-custom.css';").len(),
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
        let framework = imports("@import 'stillwind' supports(display:grid);");
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
        // A nested statement stays body text.
        let css = "@source '../x'; .a { @source 'y'; } @source inline('flex'); @reference './a.css'; .b{}";
        let found = super::scan(css).unwrap();
        assert_eq!(
            found
                .ignored
                .iter()
                .map(|feature| (&css[feature.range.clone()], feature.kind.clone()))
                .collect::<Vec<_>>(),
            [
                ("@source '../x';", IgnoredTailwindFeatureKind::Source),
                (
                    "@source inline('flex');",
                    IgnoredTailwindFeatureKind::Source
                ),
                (
                    "@reference './a.css';",
                    IgnoredTailwindFeatureKind::Reference
                ),
            ]
        );
        assert_eq!(found.removed().len(), 3);
        assert_eq!(
            ignored_tailwind_features("@theme static prefix(tw) { --a: prefix(b); }")
                .iter()
                .map(|feature| feature.kind.clone())
                .collect::<Vec<_>>(),
            [IgnoredTailwindFeatureKind::ThemePrefix]
        );
        assert_eq!(
            wrappers("prefix(tw) reference source(none) print", 0).unwrap(),
            vec!["@media print"]
        );
        // A skipped import does not hide the features after it.
        assert_eq!(
            ignored_tailwind_features("@import 42; @config 'x';"),
            [IgnoredTailwindFeature {
                range: 12..24,
                kind: IgnoredTailwindFeatureKind::Config,
            }]
        );
    }
}
