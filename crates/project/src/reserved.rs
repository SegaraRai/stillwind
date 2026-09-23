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
//! `@property` registrations, `@layer` statements and blocks, `@keyframes` names, class
//! selectors, and the custom-ident properties that name anchors, containers, and view
//! transitions. It does not resolve imports, which the caller does before scanning, and it
//! does not interpret values.
//!
//! A block the tokenizer cannot close contributes no names. The walk is best-effort and has no
//! error channel, because the CSS it scans was parsed by the design system before it got here.
//!
//! The scan covers the text it is given: the design system's own sources, and whatever a host
//! supplies for a producer through `ProjectSession::reserve_authored_css`. CSS a producer authors
//! never reaches the compiler by itself, so a host that does not supply it leaves those names out.

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
    /// Class tokens written as class selectors.
    pub classes: BTreeSet<String>,
    /// `anchor-name`, `position-anchor`, and `anchor-scope` values.
    pub anchor_names: BTreeSet<String>,
    /// `container-name` values and `@container` prelude names.
    pub container_names: BTreeSet<String>,
    /// `view-transition-class` values, the namespace generated transition tokens are written in.
    ///
    /// `view-transition-name` values are pairing keys rather than allocated names, so an
    /// authored name is deliberately not reserved.
    pub view_transition_classes: BTreeSet<String>,
}

impl ReservedNames {
    /// Whether nothing was reserved.
    pub fn is_empty(&self) -> bool {
        self.custom_properties.is_empty()
            && self.layers.is_empty()
            && self.keyframes.is_empty()
            && self.classes.is_empty()
            && self.anchor_names.is_empty()
            && self.container_names.is_empty()
            && self.view_transition_classes.is_empty()
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
                let mut next_at_declaration = false;
                if at_declaration {
                    if is_custom_property(name) {
                        reserved.custom_properties.insert(name.to_string());
                    } else if let Some(names) = ident_names(name) {
                        // A property whose value is a custom-ident names something in the document,
                        // so the value belongs to that name's pool.
                        next_at_declaration = collect_ident_values(reserved, parser, names);
                    }
                }
                at_declaration = next_at_declaration;
            }
            Token::Delim('.') => {
                // A dot directly before an ident is a class selector: an identifier cannot start
                // with a dot, so a value can never spell one, and `.5px` is a number instead.
                if let Ok(Token::Ident(name)) = parser.next() {
                    reserved.classes.insert(name.to_string());
                }
            }
            Token::Semicolon => at_declaration = true,
            Token::CurlyBracketBlock => {
                // A malformed block drops the names inside it; see the module documentation.
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
                    "layer" => {
                        collect_layer(reserved, parser);
                    }
                    "keyframes" => {
                        if let Ok(name) = parser.expect_ident_cloned() {
                            reserved.keyframes.insert(name.to_string());
                        }
                    }
                    "container" => collect_container_queries(reserved, parser),
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

/// The pool one custom-ident property's values are reserved in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NamePool {
    Anchor,
    Container,
    ViewTransitionClass,
}

impl ReservedNames {
    /// Reserves one custom-ident value in the pool that owns it.
    fn insert_ident(&mut self, pool: NamePool, name: &str) {
        let names = match pool {
            NamePool::Anchor => &mut self.anchor_names,
            NamePool::Container => &mut self.container_names,
            NamePool::ViewTransitionClass => &mut self.view_transition_classes,
        };
        names.insert(name.to_string());
    }
}

/// The pool a property's values belong to, or `None` when it takes no custom-ident.
fn ident_names(property: &str) -> Option<NamePool> {
    match property.to_ascii_lowercase().as_str() {
        "anchor-name" | "position-anchor" | "anchor-scope" => Some(NamePool::Anchor),
        "container-name" => Some(NamePool::Container),
        "view-transition-class" => Some(NamePool::ViewTransitionClass),
        _ => None,
    }
}

/// Reads one declaration's custom-ident values into the pool that owns them.
///
/// The values are the declaration's own idents: a block such as `var()` is stepped over, because a
/// name it mentions belongs to that name's own property, and CSS-wide keywords and `none` name
/// nothing. The loop ends at the declaration's semicolon or at the end of the enclosing block, and
/// returns whether the declaration used a semicolon, which is where the next declaration begins.
fn collect_ident_values(
    reserved: &mut ReservedNames,
    parser: &mut Parser<'_>,
    pool: NamePool,
) -> bool {
    loop {
        match parser.next_including_whitespace_and_comments() {
            Ok(Token::Ident(name)) => {
                if !is_css_wide_or_none(name) {
                    reserved.insert_ident(pool, name);
                }
            }
            Ok(token) if is_block(token) => {
                let _ = parser.parse_nested_block(|_| Ok::<(), ParseError<()>>(()));
            }
            Ok(Token::Semicolon) => return true,
            Err(_) => return false,
            Ok(_) => {}
        }
    }
}

/// Reads the names a `@container` prelude gives before its query.
///
/// A prelude names containers first, so idents before the query are names; the query's own grammar
/// is stepped over, and `not`, `and`, and `or` are operators rather than names.
fn collect_container_queries(reserved: &mut ReservedNames, parser: &mut Parser<'_>) {
    loop {
        match parser.next_including_whitespace_and_comments() {
            Ok(Token::Ident(name)) if !is_condition_operator(name) => {
                reserved.container_names.insert(name.to_string());
            }
            Ok(token) if is_block(token) => {
                if matches!(token, Token::CurlyBracketBlock) {
                    // The block holds the rules the query applies to, so its names are this scan's
                    // business too.
                    let _ = parser.parse_nested_block(|body| {
                        walk(reserved, body, true);
                        Ok::<(), ParseError<()>>(())
                    });
                    return;
                }
                let _ = parser.parse_nested_block(|_| Ok::<(), ParseError<()>>(()));
            }
            Ok(Token::Semicolon) | Err(_) => return,
            Ok(_) => {}
        }
    }
}

/// Whether a token opens a block this scan does not interpret.
fn is_block(token: &Token<'_>) -> bool {
    matches!(
        token,
        Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock
    )
}

/// Whether one ident is a query operator rather than a name.
fn is_condition_operator(name: &str) -> bool {
    ["not", "and", "or"]
        .iter()
        .any(|operator| name.eq_ignore_ascii_case(operator))
}

/// Whether one ident is a CSS-wide keyword or `none`, which name nothing.
fn is_css_wide_or_none(name: &str) -> bool {
    [
        "none",
        "initial",
        "inherit",
        "unset",
        "revert",
        "revert-layer",
        "revert-rule",
    ]
    .iter()
    .any(|keyword| name.eq_ignore_ascii_case(keyword))
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
            Ok(Token::Comma) if !next => {
                next = true;
            }
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
        assert_eq!(
            reserved.classes.iter().cloned().collect::<Vec<_>>(),
            vec!["x".to_owned()]
        );
        assert!(reserved.custom_properties.is_empty(), "{reserved:?}");
        assert!(reserved.layers.is_empty() && reserved.keyframes.is_empty());
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

    #[test]
    fn collects_class_selectors_and_custom_ident_properties() {
        let mut reserved = ReservedNames::default();
        reserved.collect(
            ".card,.card:is(.wide){anchor-name:--panel;position-anchor:--trigger, --other;\
             container-name:card-box;view-transition-name:hero;view-transition-class:slide;\
             width:var(--panel);height:.5px;}",
        );
        assert_eq!(
            reserved.classes.iter().cloned().collect::<Vec<_>>(),
            vec!["card".to_owned(), "wide".to_owned()]
        );
        assert_eq!(
            reserved.anchor_names.iter().cloned().collect::<Vec<_>>(),
            vec![
                "--other".to_owned(),
                "--panel".to_owned(),
                "--trigger".to_owned()
            ]
        );
        assert_eq!(
            reserved.container_names.iter().cloned().collect::<Vec<_>>(),
            vec!["card-box".to_owned()]
        );
        assert_eq!(
            reserved
                .view_transition_classes
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["slide".to_owned()]
        );
    }

    #[test]
    fn collects_container_query_names_without_their_operators() {
        let mut reserved = ReservedNames::default();
        reserved.collect(
            "@container panel (width > 1px){.x{color:red}}\
             @container not (width > 1px){@container sidebar (width > 2px){.y{color:blue}}}",
        );
        assert_eq!(
            reserved.container_names.iter().cloned().collect::<Vec<_>>(),
            vec!["panel".to_owned(), "sidebar".to_owned()]
        );
        assert_eq!(
            reserved.classes.iter().cloned().collect::<Vec<_>>(),
            vec!["x".to_owned(), "y".to_owned()]
        );
    }

    #[test]
    fn keeps_none_and_css_wide_keywords_out_of_the_pools() {
        let mut reserved = ReservedNames::default();
        reserved.collect(
            ".x{anchor-name:none;container-name:INHERIT;view-transition-name:initial;\
             view-transition-class:revert-layer;}",
        );
        assert!(
            reserved.anchor_names.is_empty(),
            "{:?}",
            reserved.anchor_names
        );
        assert!(reserved.container_names.is_empty());
        assert!(reserved.view_transition_classes.is_empty());
    }

    /// The recursive walk keeps the declaration position across nested rules and
    /// semicolons, so only property positions contribute custom properties.
    #[test]
    fn walk_tracks_the_declaration_position_across_blocks() {
        let mut reserved = ReservedNames::default();
        let css = "--a:1; .nested { --b:2; content:'--c:3'; } margin:calc(1px - -2px); --d:4;";
        walk(&mut reserved, &mut Parser::new(css), true);
        assert_eq!(
            reserved
                .custom_properties
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            ["--a", "--b", "--d"]
        );
        // Starting outside a declaration position reads the value as text.
        let mut reserved = ReservedNames::default();
        walk(&mut reserved, &mut Parser::new("--a:1;"), false);
        assert!(reserved.custom_properties.is_empty());
    }
}
