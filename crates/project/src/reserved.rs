//! Names authored CSS already uses, reserved before any generated name is allocated.
//!
//! [`ReservedNames`] collects what a stylesheet already declares in the namespaces the
//! compiler allocates names in. Generated identifiers are checked against these, so a
//! generated name can never silently change the meaning of an authored declaration or
//! reference. Names are stored in the canonical spelling the pools compare, which is the
//! decoded one the CSS tokenizer already produces.
//!
//! The scan is a token walk, not a text search: `cssparser` splits the stylesheet into
//! tokens, so a comment or a string can never be mistaken for a declaration, and escapes
//! are decoded for free. It reads custom property declarations, `var()` references,
//! `@property` registrations, `@layer` statements and blocks, and `@keyframes` names.
//! It does not resolve imports, which the caller does before scanning, and it does not
//! interpret values.
//!
//! TODO: CSS a module authors rather than imports is not scanned here. The compiler
//! receives its declarations already resolved, so that text only exists on the host
//! side; reserving it requires the host to supply those names with the analysis.

use std::collections::BTreeSet;

use cssparser::{ParseError, Parser, Token};

/// Names one document already uses, by CSS namespace.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReservedNames {
    /// Custom property names, including the leading double hyphen, from declarations,
    /// `var()` references, and `@property` registrations.
    pub custom_properties: BTreeSet<String>,
    /// `@layer` names, both statement and block forms.
    pub layers: BTreeSet<String>,
    /// `@keyframes` names.
    pub keyframes: BTreeSet<String>,
}

impl ReservedNames {
    /// Whether nothing was reserved.
    pub fn is_empty(&self) -> bool {
        self.custom_properties.is_empty() && self.layers.is_empty() && self.keyframes.is_empty()
    }

    /// Reserves every name `css` uses.
    ///
    /// Repeated scans accumulate; a name already reserved is kept once. An unparsable
    /// region ends the walk instead of guessing, because a wrong reservation is worse
    /// than a missing one: it would rename a name the host chose.
    pub fn collect(&mut self, css: &str) {
        let mut parser = Parser::new(css);
        walk(self, &mut parser, true);
    }
}

/// Walks one stylesheet, rule body, or declaration body.
///
/// `at_declaration` is true where a custom property name may appear: at the start of a
/// rule body and after every semicolon. Anywhere else a leading double hyphen is part of
/// a value, such as the subtraction in `calc(1px - -2px)`, and is ignored.
fn walk(reserved: &mut ReservedNames, parser: &mut Parser<'_>, mut at_declaration: bool) {
    loop {
        let Ok(token) = parser.next() else {
            return;
        };
        match token {
            Token::Ident(name) => {
                if at_declaration && is_custom_property(name) {
                    reserved.custom_properties.insert(name.to_string());
                }
                at_declaration = false;
            }
            Token::Semicolon => at_declaration = true,
            Token::CurlyBracketBlock => {
                let _ = parser.parse_nested_block(|body| {
                    walk(reserved, body, true);
                    Ok::<(), ParseError<()>>(())
                });
                // A nested rule ends where the next declaration may begin, so the
                // position survives it.
                at_declaration = true;
            }
            Token::ParenthesisBlock | Token::SquareBracketBlock => {
                let _ = parser.parse_nested_block(|body| {
                    walk(reserved, body, false);
                    Ok::<(), ParseError<()>>(())
                });
            }
            Token::AtKeyword(name) => {
                let name = name.to_ascii_lowercase();
                match name.as_ref() {
                    "layer" => collect_layer(reserved, parser),
                    "keyframes" => {
                        if let Ok(name) = parser.expect_ident_cloned() {
                            reserved.keyframes.insert(name.to_string());
                        }
                    }
                    "property" => {
                        if let Ok(Token::Ident(name)) = parser.next()
                            && is_custom_property(name)
                        {
                            reserved.custom_properties.insert(name.to_string());
                        }
                    }
                    _ => {}
                }
                at_declaration = true;
            }
            Token::Function(name) if name.eq_ignore_ascii_case("var") => {
                let _ = parser.parse_nested_block(|arguments| {
                    if let Ok(Token::Ident(name)) = arguments.next()
                        && is_custom_property(name)
                    {
                        reserved.custom_properties.insert(name.to_string());
                    }
                    walk(reserved, arguments, false);
                    Ok::<(), ParseError<()>>(())
                });
                at_declaration = false;
            }
            Token::Function(_) => {
                let _ = parser.parse_nested_block(|arguments| {
                    walk(reserved, arguments, false);
                    Ok::<(), ParseError<()>>(())
                });
                at_declaration = false;
            }
            _ => at_declaration = false,
        }
    }
}

/// Reads the names of a `@layer` statement or block and walks its body.
///
/// A statement carries a comma-separated list and ends at its semicolon; a block carries
/// names followed by rules.
fn collect_layer(reserved: &mut ReservedNames, parser: &mut Parser<'_>) {
    let mut next = true;
    loop {
        match parser.next_including_whitespace_and_comments() {
            Ok(Token::WhiteSpace(_) | Token::Comment(_)) => {}
            Ok(Token::Ident(name)) if next => {
                reserved.layers.insert(name.to_string());
                next = false;
            }
            Ok(Token::Comma) if !next => next = true,
            Ok(Token::CurlyBracketBlock) => {
                let _ = parser.parse_nested_block(|body| {
                    walk(reserved, body, true);
                    Ok::<(), ParseError<()>>(())
                });
                return;
            }
            _ => return,
        }
    }
}

/// Whether a token names a custom property, which always starts with two hyphens.
fn is_custom_property(name: &str) -> bool {
    name.starts_with("--") && name.len() > 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_declarations_references_and_registrations_once() {
        let mut reserved = ReservedNames::default();
        reserved.collect(
            ":root{--sw-user:1;--sw-user:2;} .x{color:var(--sw-user);width:var(--w, 1px);}",
        );
        assert_eq!(
            reserved
                .custom_properties
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["--sw-user".to_owned(), "--w".to_owned()]
        );
    }

    #[test]
    fn collects_layers_statement_blocks_and_keyframes() {
        let mut reserved = ReservedNames::default();
        reserved.collect("@layer base, theme;@layer sw-anon-1{@keyframes spin{to{opacity:0}}}");
        assert_eq!(
            reserved.layers.iter().cloned().collect::<Vec<_>>(),
            vec![
                "base".to_owned(),
                "sw-anon-1".to_owned(),
                "theme".to_owned()
            ]
        );
        assert_eq!(
            reserved.keyframes.iter().cloned().collect::<Vec<_>>(),
            vec!["spin".to_owned()]
        );
    }

    #[test]
    fn decodes_escapes_so_reservations_compare_in_one_spelling() {
        let mut reserved = ReservedNames::default();
        reserved.collect(".x{--a\\:b:1;} .y{color:var(--a\\:b);}");
        assert_eq!(
            reserved
                .custom_properties
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["--a:b".to_owned()]
        );
    }

    #[test]
    fn ignores_comments_strings_and_spaced_hyphens() {
        let mut reserved = ReservedNames::default();
        reserved
            .collect("/* --sw-comment:1; */ .x{content:\"--sw-string\";margin:calc(1px - -2px);}");
        assert!(reserved.is_empty(), "{reserved:?}");
    }

    #[test]
    fn collects_property_registrations() {
        let mut reserved = ReservedNames::default();
        reserved.collect("@property --sw-registered{syntax:'<length>';inherits:false;}");
        assert_eq!(
            reserved
                .custom_properties
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["--sw-registered".to_owned()]
        );
    }

    #[test]
    fn keeps_the_declaration_position_after_nested_rules() {
        let mut reserved = ReservedNames::default();
        reserved.collect(".x{ @media print { --sw-print: 1; } --sw-after: 2; }");
        assert_eq!(
            reserved
                .custom_properties
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["--sw-after".to_owned(), "--sw-print".to_owned()]
        );
    }
}
