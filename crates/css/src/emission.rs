//! Lossless rule occurrences and conservative declaration scheduling.
//!
//! Parsing preserves opaque syntax. Optimization crosses only declaration blocks
//! whose property effects are understood. Conditional and registration rules remain
//! barriers. [`CssRule::parse`] records source ranges, [`optimize_rules`] merges and
//! schedules rules, and [`stylesheet_fragments`] splits transportable fragments.

use std::ops::Range;

use cssparser::{ParseError, Parser, Token};
use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use crate::{csswg_formal_syntax, declaration_ranges, properties_interfere};

/// One rule occurrence in a stylesheet, independent of its content identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssRule {
    /// Original UTF-8 byte range. Merging expands it to cover contributing occurrences.
    pub range: Range<usize>,
    /// Lossless selector, at-rule header, or complete statement.
    pub prelude: LeanStr,
    /// Contents of a brace block, when present.
    pub body: Option<LeanStr>,
}

impl CssRule {
    /// Serializes a rule without changing tokens or whitespace.
    pub fn to_css(&self) -> LeanStr {
        self.body.as_ref().map_or_else(
            || self.prelude.clone(),
            |body| format_args!("{}{{{body}}}", self.prelude).to_lean_str(),
        )
    }

    /// Parses top-level occurrences while preserving nested blocks as indivisible units.
    ///
    /// # Errors
    ///
    /// Returns the start byte of the block contents if nested-block parsing fails.
    pub fn parse(css: &str) -> Result<Vec<Self>, usize> {
        let mut parser = Parser::new(css);
        let mut start = 0;
        let mut rules = Vec::new();
        while let Ok(token) = parser.next_including_whitespace_and_comments() {
            if matches!(token, Token::CurlyBracketBlock) {
                let body_start = parser.position().byte_index();
                let body_end = parser
                    .parse_nested_block(|nested| {
                        while nested.next_including_whitespace_and_comments().is_ok() {}
                        Ok::<_, ParseError<()>>(nested.position().byte_index())
                    })
                    .map_err(|_| body_start)?;
                let end = parser.position().byte_index();
                rules.push(Self {
                    range: start..end,
                    prelude: css[start..body_start - 1].into(),
                    body: Some(css[body_start..body_end].into()),
                });
                start = end;
            } else if matches!(token, Token::Semicolon) {
                let end = parser.position().byte_index();
                rules.push(Self {
                    range: start..end,
                    prelude: css[start..end].into(),
                    body: None,
                });
                start = end;
            }
        }
        if start < css.len() {
            rules.push(Self {
                range: start..css.len(),
                prelude: css[start..].into(),
                body: None,
            });
        }
        Ok(rules)
    }

    /// Returns understood declarations for a simple generated class rule.
    /// Arbitrary selectors and nested syntax conservatively block scheduling.
    pub fn declarations(&self) -> Option<Vec<Declaration>> {
        let mut selector = Parser::new(self.prelude.trim());
        if !matches!(selector.next().ok()?, Token::Delim('.'))
            || !matches!(selector.next().ok()?, Token::Ident(_))
            || !selector.is_exhausted()
        {
            return None;
        }
        Declaration::parse(self.body.as_deref()?)
    }
}

/// A declaration with its original token spelling and property effect.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    /// Standard properties are lowercase; custom properties preserve case.
    pub property: LeanStr,
    /// Complete declaration without its terminating semicolon.
    pub css: LeanStr,
}

impl Declaration {
    /// Parses flat declaration lists. Unsupported syntax is an optimization barrier.
    pub fn parse(css: &str) -> Option<Vec<Self>> {
        declaration_ranges(css)
            .into_iter()
            .filter_map(|range| {
                let text = css[range].trim();
                (!text.is_empty()).then_some(text)
            })
            .map(|text| {
                let mut parser = Parser::new(text);
                let Token::Ident(name) = parser.next().ok()? else {
                    return None;
                };
                let property = if name.starts_with("--") {
                    name.to_lean_string()
                } else {
                    name.to_lean_string().to_ascii_lowercase()
                };
                if !property.starts_with("--") && csswg_formal_syntax(&property).is_none() {
                    return None;
                }
                if !matches!(parser.next().ok()?, Token::Colon) {
                    return None;
                }
                while let Ok(token) = parser.next() {
                    if matches!(token, Token::CurlyBracketBlock | Token::AtKeyword(_)) {
                        return None;
                    }
                }
                Some(Self {
                    property: (property).into(),
                    css: text.into(),
                })
            })
            .collect()
    }

    /// Whether swapping these declarations could change a cascaded property.
    pub fn interferes(&self, other: &Self) -> bool {
        if self.property.starts_with("--") || other.property.starts_with("--") {
            return self.property == other.property;
        }
        properties_interfere(&self.property, &other.property)
    }
}

/// Orders declarations by property name where adjacent declarations do not interfere.
fn schedule_declarations(declarations: &mut [Declaration]) {
    for index in 1..declarations.len() {
        let mut cursor = index;
        while cursor > 0
            && declarations[cursor].property < declarations[cursor - 1].property
            && !declarations[cursor].interferes(&declarations[cursor - 1])
        {
            declarations.swap(cursor - 1, cursor);
            cursor -= 1;
        }
    }
}

/// Optimizes understood rule lists using a bounded look-back window. Unknown
/// selectors, at-rules and overlapping properties retain their reference order.
pub fn optimize_rules(rules: &mut Vec<CssRule>) {
    optimize_nested(rules, 0);
}

fn optimize_nested(rules: &mut Vec<CssRule>, depth: usize) {
    if depth >= 64 {
        return;
    }
    for rule in rules.iter_mut() {
        let header = rule.prelude.trim_start();
        let group = [
            "@layer ",
            "@media ",
            "@supports ",
            "@container ",
            "@scope ",
            "@starting-style",
        ]
        .iter()
        .any(|prefix| header.starts_with(prefix));
        if group {
            if let Some(body) = &rule.body
                && let Ok(mut children) = CssRule::parse(body)
            {
                optimize_nested(&mut children, depth + 1);
                rule.body = Some(children.iter().map(CssRule::to_css).collect());
            }
        } else if let Some(mut declarations) = rule.declarations() {
            schedule_declarations(&mut declarations);
            rule.body = Some(
                declarations
                    .iter()
                    .fold(LeanString::new(), |mut text, declaration| {
                        text.push_str(&declaration.css);
                        text.push(';');
                        text
                    })
                    .into(),
            );
        }
    }
    let mut index = 0;
    while index < rules.len() {
        let Some(declarations) = rules[index].declarations() else {
            index += 1;
            continue;
        };
        let mut previous = index;
        let mut merged = false;
        while previous > index.saturating_sub(64) {
            previous -= 1;
            let Some(other) = rules[previous].declarations() else {
                break;
            };
            if rules[previous].body == rules[index].body {
                // A comma list is safe for the plain class selectors accepted above.
                if rules[previous].prelude.trim() != rules[index].prelude.trim() {
                    rules[previous].prelude = format_args!(
                        "{},{}",
                        rules[previous].prelude.trim(),
                        rules[index].prelude.trim()
                    )
                    .to_lean_str();
                }
                rules[previous].range.end = rules[index].range.end;
                rules.remove(index);
                merged = true;
                break;
            }
            if declarations
                .iter()
                .any(|a| other.iter().any(|b| a.interferes(b)))
            {
                break;
            }
        }
        if !merged {
            index += 1;
        }
    }
}

/// Splits named conditional groups into complete, independently transportable rules.
/// Anonymous layers and unknown constructs remain atomic because splitting them can
/// change their identity or interpretation. Ranges address the original stylesheet.
///
/// # Errors
///
/// Returns the start byte of the block contents if nested-block parsing fails.
pub fn stylesheet_fragments(css: &str) -> Result<Vec<(LeanString, Range<usize>)>, usize> {
    fn visit(
        css: &str,
        offset: usize,
        wrappers: &[LeanString],
        depth: usize,
        output: &mut Vec<(LeanString, Range<usize>)>,
    ) -> Result<(), usize> {
        for rule in CssRule::parse(css)? {
            let header = rule.prelude.trim_start();
            let splittable = ["@layer ", "@media ", "@supports ", "@container "]
                .iter()
                .any(|prefix| header.starts_with(prefix));
            if depth < 64
                && splittable
                && let Some(body) = &rule.body
                && !body.trim().is_empty()
            {
                let mut nested = wrappers.to_vec();
                nested.push((rule.prelude.clone()).into());
                visit(
                    body,
                    offset + rule.range.start + rule.prelude.len() + 1,
                    &nested,
                    depth + 1,
                    output,
                )?;
            } else {
                let mut text = rule.to_css();
                for wrapper in wrappers.iter().rev() {
                    text = format_args!("{wrapper}{{{text}}}").to_lean_str();
                }
                output.push((
                    (text).into(),
                    offset + rule.range.start..offset + rule.range.end,
                ));
            }
        }
        Ok(())
    }
    // Namespace bindings apply to one stylesheet, including default namespaces used
    // by selectors without a prefix. Keep that scope intact across transport modes.
    if CssRule::parse(css)?.iter().any(|rule| {
        matches!(Parser::new(&rule.prelude).next(), Ok(Token::AtKeyword(name)) if name.eq_ignore_ascii_case("namespace"))
    }) {
        return Ok(vec![(css.into(), 0..css.len())]);
    }
    let mut output = Vec::new();
    visit(css, 0, &[], 0, &mut output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn namespace_bindings_keep_the_entire_stylesheet_in_one_fragment() {
        let css = "/* scope */ @NAMESPACE svg url(http://www.w3.org/2000/svg); svg|a{fill:red}.x{color:blue}";
        assert_eq!(
            stylesheet_fragments(css).unwrap(),
            vec![(css.into(), 0..css.len())]
        );
    }

    #[test]
    fn fragments_keep_anonymous_layers_and_unknown_groups_atomic() {
        let css = "@layer a,b;@layer a{@media screen{.a{color:red}.b{color:blue}}}@layer{.a{color:red}.b{color:blue}}";
        let parts = stylesheet_fragments(css).unwrap();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[1].0, "@layer a{@media screen{.a{color:red}}}");
        assert_eq!(parts[3].0, "@layer{.a{color:red}.b{color:blue}}");
        assert_eq!(&css[parts[1].1.clone()], ".a{color:red}");
    }

    #[test]
    fn occurrences_roundtrip_opaque_syntax_and_unicode() {
        for css in [
            "",
            "@layer base;",
            "/* 🧭 */.x{content:'}';color:red;}\n",
            "@unknown x{.y{a:b}}",
            ".x{color:red}.x{color:blue}.x{color:red}",
        ] {
            assert_eq!(
                CssRule::parse(css)
                    .unwrap()
                    .iter()
                    .map(CssRule::to_css)
                    .collect::<LeanString>(),
                css
            );
        }
    }

    fn optimize(css: &str) -> LeanString {
        let mut rules = CssRule::parse(css).unwrap();
        optimize_rules(&mut rules);
        rules.iter().map(CssRule::to_css).collect()
    }

    #[test]
    fn conflicting_occurrences_and_fallbacks_survive() {
        let css = ".x{color:red;}.x{color:blue;}.x{color:red;}";
        assert_eq!(optimize(css), css);
        let css =
            ".x{margin-left:2px;margin:1px;margin-inline-start:3px;color:red;color:var(--x);}";
        let optimized = optimize(css);
        assert!(optimized.find("margin-left").unwrap() < optimized.find("margin:1px").unwrap());
        assert!(optimized.find("margin:1px").unwrap() < optimized.find("margin-inline").unwrap());
        assert!(optimized.find("color:red").unwrap() < optimized.find("color:var").unwrap());
    }

    #[test]
    fn independent_rules_share_declarations_without_crossing_conditions() {
        assert_eq!(
            optimize(".a{color:red}.b{width:1px}.c{color:red}"),
            ".a,.c{color:red;}.b{width:1px;}"
        );
        let css = ".a:hover{color:red}.a:focus{color:blue}.a:hover{color:red}";
        assert_eq!(optimize(css), css);
        assert_eq!(
            optimize(".a{future-prop:x}.b{color:red}.c{future-prop:x}"),
            ".a{future-prop:x}.b{color:red;}.c{future-prop:x}"
        );
    }

    fn cascade(css: &str, classes: &[&str]) -> BTreeMap<LeanStr, LeanStr> {
        let mut result = BTreeMap::new();
        for rule in CssRule::parse(css).unwrap() {
            if rule
                .prelude
                .split(',')
                .any(|selector| classes.contains(&selector.trim().trim_start_matches('.')))
            {
                for declaration in Declaration::parse(rule.body.as_deref().unwrap()).unwrap() {
                    result.insert(declaration.property, declaration.css);
                }
            }
        }
        result
    }

    proptest::proptest! {
        #[test]
        fn scheduling_preserves_every_class_combination(values in proptest::collection::vec((0u8..3, 0u8..3, 0u8..9), 0..25)) {
            let names = ["a", "b", "c"];
            let properties = ["color", "width", "height"];
            let css = values.into_iter().fold(LeanString::new(), |mut css, (class, property, value)| {
                use std::fmt::Write;
                write!(css, ".{}{{{}:{}px;}}", names[class as usize], properties[property as usize], value).unwrap();
                css
            });
            let optimized = optimize(&css);
            for mask in 0..8 {
                let classes: Vec<_> = names.iter().enumerate().filter_map(|(index, name)| (mask & (1 << index) != 0).then_some(*name)).collect();
                proptest::prop_assert_eq!(cascade(&css, &classes), cascade(&optimized, &classes));
            }
        }
    }

    fn optimize_nested_css(css: &str) -> LeanString {
        let mut rules = CssRule::parse(css).unwrap();
        optimize_nested(&mut rules, 0);
        rules.iter().map(CssRule::to_css).collect()
    }

    /// Declarations are re-serialized with one trailing semicolon each, and a group
    /// rule is recursed into instead of being read as declarations.
    #[test]
    fn optimize_nested_normalizes_declarations_and_recurses_into_groups() {
        assert_eq!(optimize_nested_css(".a{color:red}"), ".a{color:red;}");
        assert_eq!(
            optimize_nested_css("@media x{.a{color:red}}"),
            "@media x{.a{color:red;}}"
        );
        assert_eq!(
            optimize_nested_css("@layer a;.b{color:red}"),
            "@layer a;.b{color:red;}"
        );
        assert_eq!(
            optimize_nested_css(".x{color:red;color:blue;}"),
            ".x{color:red;color:blue;}"
        );
    }

    /// Rules with equal bodies merge into a comma list at the earlier position, the
    /// backward search stops at an interfering declaration, and a group rule blocks it.
    #[test]
    fn optimize_nested_merges_equal_bodies_within_the_interference_window() {
        assert_eq!(
            optimize_nested_css(".a{color:red}.b{padding:0}.c{color:red}"),
            ".a,.c{color:red;}.b{padding:0;}"
        );
        assert_eq!(
            optimize_nested_css(".a{color:red}.b{color:blue}.c{color:red}"),
            ".a{color:red;}.b{color:blue;}.c{color:red;}"
        );
        assert_eq!(
            optimize_nested_css(".a{color:red}@media x{.b{padding:0}}.c{color:red}"),
            ".a{color:red;}@media x{.b{padding:0;}}.c{color:red;}"
        );
    }

    /// The backward search is bounded to 64 rules, so a duplicate body further away
    /// stays a separate occurrence.
    #[test]
    fn optimize_nested_merges_only_within_its_lookback_window() {
        let fillers = |count: usize| {
            (0..count).fold(LeanString::new(), |mut css, index| {
                use std::fmt::Write;
                write!(css, ".f{index}{{--v{index}:{index};}}").unwrap();
                css
            })
        };
        let near = format_args!(".a{{color:red}}{}.dup{{color:red}}", fillers(63)).to_lean_string();
        let optimized = optimize_nested_css(&near);
        assert!(
            optimized.contains(".a,.dup{color:red;}"),
            "distance 64 must merge: {optimized}"
        );
        let far = format_args!(".a{{color:red}}{}.dup{{color:red}}", fillers(64)).to_lean_string();
        let optimized = optimize_nested_css(&far);
        assert!(
            optimized.contains(".a{color:red;}") && optimized.contains(".dup{color:red;}"),
            "distance 65 must stay separate: {optimized}"
        );
    }

    /// Recursion stops past the documented depth: one level inside the limit still
    /// merges the equal bodies, one level past it leaves the authored text alone.
    #[test]
    fn optimize_nested_stops_at_the_depth_limit() {
        let body = ".a{color:red}.b{color:red}";
        let shallow =
            format_args!("{}{}{}", "@media a{".repeat(63), body, "}".repeat(63)).to_lean_string();
        assert!(
            optimize_nested_css(&shallow).contains(".a,.b{color:red;}"),
            "at the limit the inner rules are optimized"
        );
        let deep =
            format_args!("{}{}{}", "@media a{".repeat(64), body, "}".repeat(64)).to_lean_string();
        assert_eq!(
            optimize_nested_css(&deep),
            deep,
            "past the limit nothing inside is rewritten"
        );
    }
}
