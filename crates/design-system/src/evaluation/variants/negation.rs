//! Negation of a resolved variant application for the `not-*` variant.
//!
//! A resolved application is a boolean formula: `Compound` lists alternatives (OR), and
//! `Sequence` lists conditions that must all hold (AND). [`negate`] moves the negation
//! to each selector and at-rule condition by De Morgan's laws. The result is a
//! `Compound` of branches, and each branch requires all of its conditions. Negated
//! selectors of one branch share one compound selector, such as
//! `&:not(:hover):not(:focus)`.
//!
//! Negation rejects applications that it cannot express as a condition on the styled
//! element: variant declarations, `@starting-style`, selectors with pseudo-elements,
//! selectors that style another element, and results with more than [`MAX_BRANCHES`]
//! branches.

use cssparser::{Parser, Token};
use lean_string::{LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{
    SelectorTarget, selector_is_element_condition, selector_target, split_top_level,
    subject_as_universal,
};

use super::application::{VariantApplication, VariantObstacle};

/// Maximum number of alternatives in a negated application.
///
/// Negating several alternatives multiplies the branches of their negations, so a long
/// variant chain such as `not-*` around many multi-block custom variants could
/// otherwise emit an exponential number of rules.
const MAX_BRANCHES: usize = 32;

/// Returns the application that applies exactly where `application` does not apply.
///
/// Returns `None` when a part of `application` has no negated form or when the result
/// would need more than [`MAX_BRANCHES`] alternatives.
pub(super) fn negate(application: &VariantApplication) -> Option<VariantApplication> {
    let mut alternatives = negated_branches(application)?
        .into_iter()
        .map(branch_application)
        .collect::<Option<Vec<_>>>()?;
    match alternatives.len() {
        0 => None,
        1 => alternatives.pop(),
        _ => Some(VariantApplication::Compound(alternatives)),
    }
}

/// Returns the part of `application` that [`negate`] cannot negate.
///
/// The first part without a negated form is reported, in the order the application
/// lists its parts. When every part has a negated form, the negation failed because it
/// needs more than [`MAX_BRANCHES`] alternatives.
pub(super) fn negation_obstacle(application: &VariantApplication) -> VariantObstacle {
    part_obstacle(application).unwrap_or(VariantObstacle::TooManyAlternatives)
}

/// Returns the obstacle of the first part of `application` without a negated form.
fn part_obstacle(application: &VariantApplication) -> Option<VariantObstacle> {
    match application {
        VariantApplication::Compound(children) | VariantApplication::Sequence(children) => {
            children.iter().find_map(part_obstacle)
        }
        VariantApplication::SelectorSuffix(suffix) => {
            selector_obstacle(&format_args!("&{suffix}").to_lean_string())
        }
        VariantApplication::SelectorTemplate(selector)
        | VariantApplication::BareSelector { selector, .. }
        | VariantApplication::SelectorMedia { selector, .. } => selector_obstacle(selector),
        VariantApplication::SelectorRule { selector, .. }
        | VariantApplication::SelectorMediaRule { selector, .. } => {
            Some(selector_obstacle(selector).unwrap_or(VariantObstacle::Declarations))
        }
        VariantApplication::StartingStyle => Some(VariantObstacle::StartingStyle),
        VariantApplication::SelectorCondition(_) => Some(VariantObstacle::Unsupported),
        VariantApplication::Media(_)
        | VariantApplication::Container(_)
        | VariantApplication::Supports(_)
        | VariantApplication::Conditional(_) => None,
    }
}

/// Returns why [`negate_selector`] rejects `template`, or `None` when it accepts it.
fn selector_obstacle(template: &str) -> Option<VariantObstacle> {
    if negated_selector_list(template).is_some() {
        return None;
    }
    VariantObstacle::of_selector(template)
}

/// One condition of a negated branch.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Literal {
    /// Pseudo-class that the branch appends to the subject, such as `:not(:hover)`.
    Selector(LeanString),
    /// Complete `@media` prelude.
    Media(LeanString),
    /// Complete `@supports` prelude.
    Supports(LeanString),
    /// Complete `@container` prelude, including any container name.
    Container(LeanString),
    /// Canonical runtime condition text.
    Conditional(LeanString),
}

/// Conditions that must all hold.
type Branch = Vec<Literal>;

/// Returns the negation of `application` as alternative branches.
///
/// Selector-only branches come before branches with at-rule conditions within each
/// alternative list, which matches the rule order that Tailwind emits for `not-*`.
fn negated_branches(application: &VariantApplication) -> Option<Vec<Branch>> {
    match application {
        VariantApplication::Compound(children) => {
            let mut branches = vec![Branch::new()];
            for child in children {
                branches = conjoin(&branches, &negated_branches(child)?)?;
            }
            Some(branches)
        }
        VariantApplication::Sequence(children) => disjoin(
            children
                .iter()
                .map(negated_branches)
                .collect::<Option<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect(),
        ),
        VariantApplication::SelectorMedia {
            selector,
            condition,
        } => disjoin(vec![
            vec![negate_selector(selector)?],
            negate_media(condition)?,
        ]),
        VariantApplication::SelectorSuffix(suffix) => Some(vec![vec![negate_selector(
            &format_args!("&{suffix}").to_lean_string(),
        )?]]),
        VariantApplication::SelectorTemplate(selector)
        | VariantApplication::BareSelector { selector, .. } => {
            Some(vec![vec![negate_selector(selector)?]])
        }
        VariantApplication::Media(condition) => Some(vec![negate_media(condition)?]),
        VariantApplication::Supports(condition) => {
            Some(vec![vec![Literal::Supports(negate_condition(condition)?)]])
        }
        VariantApplication::Container(condition) => Some(vec![negate_container(condition)?]),
        VariantApplication::Conditional(condition) => Some(vec![vec![Literal::Conditional(
            condition.strip_prefix('!').map_or_else(
                || format_args!("!{condition}").to_lean_string(),
                ToLeanString::to_lean_string,
            ),
        )]]),
        VariantApplication::SelectorCondition(_)
        | VariantApplication::SelectorRule { .. }
        | VariantApplication::SelectorMediaRule { .. }
        | VariantApplication::StartingStyle => None,
    }
}

/// Combines alternatives into one list, with selector-only branches first and duplicate
/// branches removed.
fn disjoin(branches: Vec<Branch>) -> Option<Vec<Branch>> {
    let (selectors, conditions): (Vec<_>, Vec<_>) = branches.into_iter().partition(|branch| {
        branch
            .iter()
            .all(|literal| matches!(literal, Literal::Selector(_)))
    });
    let mut combined = Vec::new();
    for branch in selectors.into_iter().chain(conditions) {
        if !combined.contains(&branch) {
            combined.push(branch);
        }
    }
    (combined.len() <= MAX_BRANCHES).then_some(combined)
}

/// Returns the branches where both `left` and `right` hold, by pairing each branch of
/// `left` with each branch of `right`.
fn conjoin(left: &[Branch], right: &[Branch]) -> Option<Vec<Branch>> {
    if left.len().checked_mul(right.len())? > MAX_BRANCHES {
        return None;
    }
    let mut branches = Vec::new();
    for left in left {
        for right in right {
            let mut branch = left.clone();
            for literal in right {
                if !branch.contains(literal) {
                    branch.push(literal.clone());
                }
            }
            if !branches.contains(&branch) {
                branches.push(branch);
            }
        }
    }
    Some(branches)
}

/// Builds the application for one branch.
///
/// At-rule conditions keep their order and precede one selector template that contains
/// every negated selector of the branch. An empty branch has no condition and returns
/// `None`, because the variant would then apply everywhere.
fn branch_application(branch: Branch) -> Option<VariantApplication> {
    let mut applications = Vec::new();
    let mut selector = LeanString::new();
    for literal in branch {
        match literal {
            Literal::Selector(pseudo_class) => {
                selector.push_str(&pseudo_class);
            }
            Literal::Media(condition) => {
                applications.push(VariantApplication::Media(condition.to_lean_str()));
            }
            Literal::Supports(condition) => {
                applications.push(VariantApplication::Supports(condition.to_lean_str()));
            }
            Literal::Container(condition) => {
                applications.push(VariantApplication::Container(condition.to_lean_str()));
            }
            Literal::Conditional(condition) => {
                applications.push(VariantApplication::Conditional(condition.to_lean_str()));
            }
        }
    }
    if !selector.is_empty() {
        applications.push(VariantApplication::SelectorTemplate(
            format_args!("&{selector}").to_lean_str(),
        ));
    }
    match applications.len() {
        0 => None,
        1 => applications.pop(),
        _ => Some(VariantApplication::Sequence(applications)),
    }
}

/// Negates a selector template as a pseudo-class on the subject.
///
/// A template that is exactly `&:not(<list>)` negates to `:is(<list>)`. Every other
/// template becomes `:not(<template>)` with `&` bound to the subject. Selector lists
/// stay in one `:not()`. Returns `None` for a template with a pseudo-element, because
/// `:not()` cannot test one.
///
/// Returns `None` when a branch of the template selects an element other than `&`, such
/// as `& > *`, `& .item`, or the custom variant shorthand `(:hover)`, which resolves to
/// `& :hover`. The utility then styles a descendant, and `&:not(...)` on the styled
/// element would not express "no such descendant is selected". A template without `&`,
/// such as `::selection` or `:root`, also returns `None`, because it selects no element
/// relative to the subject.
fn negate_selector(template: &str) -> Option<Literal> {
    if let Some(list) = negated_selector_list(template) {
        return Some(Literal::Selector(
            format_args!(":is({list})").to_lean_string(),
        ));
    }
    if selector_target(template) != SelectorTarget::Attachment {
        return None;
    }
    let argument = subject_as_universal(template);
    selector_is_element_condition(&argument, false)
        .then(|| Literal::Selector(format_args!(":not({argument})").to_lean_string()))
}

/// Returns `<list>` when `template` is exactly `&:not(<list>)`.
fn negated_selector_list(template: &str) -> Option<&str> {
    let mut parser = Parser::new(template);
    if !matches!(parser.next_including_whitespace(), Ok(Token::Delim('&')))
        || !matches!(parser.next_including_whitespace(), Ok(Token::Colon))
        || !matches!(
            parser.next_including_whitespace(),
            Ok(Token::Function(name)) if name.eq_ignore_ascii_case("not")
        )
    {
        return None;
    }
    let start = parser.position().byte_index();
    let end = parser
        .parse_nested_block(|inner| {
            while inner.next_including_whitespace_and_comments().is_ok() {}
            Ok::<_, cssparser::ParseError<()>>(inner.position().byte_index())
        })
        .ok()?;
    parser.is_exhausted().then(|| &template[start..end])
}

/// Negates a media query list.
///
/// A list matches when any query matches, so its negation requires every negated query.
fn negate_media(condition: &str) -> Option<Branch> {
    split_top_level(condition, |token| matches!(token, Token::Comma))
        .into_iter()
        .map(|query| negate_media_query(query).map(Literal::Media))
        .collect()
}

/// Negates one media query.
///
/// A query with a media type, such as `print` or `screen and (color)`, negates as a
/// whole with the `not` modifier. `only` is replaced by `not`.
fn negate_media_query(query: &str) -> Option<LeanString> {
    let query = query.trim();
    let mut parser = Parser::new(query);
    match parser.next() {
        Ok(Token::Ident(name)) if name.eq_ignore_ascii_case("only") => {
            let rest = query[parser.position().byte_index()..].trim();
            (!rest.is_empty()).then(|| format_args!("not {rest}").to_lean_string())
        }
        Ok(Token::Ident(name)) if !name.eq_ignore_ascii_case("not") => {
            Some(format_args!("not {query}").to_lean_string())
        }
        _ => negate_condition(query),
    }
}

/// Negates a container query list, keeping each container name before `not`.
///
/// The list matches when any query matches, so its negation requires every negated
/// query.
fn negate_container(condition: &str) -> Option<Branch> {
    split_top_level(condition, |token| matches!(token, Token::Comma))
        .into_iter()
        .map(|query| {
            let query = query.trim();
            let mut parser = Parser::new(query);
            let name = match parser.next() {
                Ok(Token::Ident(name))
                    if !["not", "and", "or"]
                        .iter()
                        .any(|keyword| name.eq_ignore_ascii_case(keyword)) =>
                {
                    &query[..parser.position().byte_index()]
                }
                _ => "",
            };
            let negated = negate_condition(query[name.len()..].trim())?;
            Some(Literal::Container(if name.is_empty() {
                negated
            } else {
                format_args!("{name} {negated}").to_lean_string()
            }))
        })
        .collect()
}

/// Negates a media, support, or container condition without a media type or name.
///
/// A leading `not` is removed. One parenthesized or functional condition receives
/// `not`, and a combination such as `(a) and (b)` is parenthesized first. Returns
/// `None` for an empty condition.
fn negate_condition(condition: &str) -> Option<LeanString> {
    let condition = condition.trim();
    let mut parser = Parser::new(condition);
    let (negated, block) = match parser.next() {
        Err(_) => return None,
        Ok(Token::Ident(name)) => (name.eq_ignore_ascii_case("not"), false),
        Ok(Token::ParenthesisBlock | Token::Function(_)) => (false, true),
        Ok(_) => (false, false),
    };
    if negated {
        let rest = condition[parser.position().byte_index()..].trim();
        (!rest.is_empty()).then(|| rest.to_lean_string())
    } else if block && parser.is_exhausted() {
        Some(format_args!("not {condition}").to_lean_string())
    } else {
        Some(format_args!("not ({condition})").to_lean_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template(selector: &str) -> VariantApplication {
        VariantApplication::SelectorTemplate(selector.into())
    }

    fn media(condition: &str) -> VariantApplication {
        VariantApplication::Media(condition.into())
    }

    #[test]
    fn selectors_negate_on_the_subject_and_keep_lists_together() {
        for (selector, expected) in [
            ("&:hover", "&:not(:hover)"),
            ("&:hover, &:focus", "&:not(:hover, :focus)"),
            (".foo &", "&:not(.foo *)"),
            (".foo ~ &", "&:not(.foo ~ *)"),
            (":where([data-open]) &", "&:not(:where([data-open]) *)"),
            ("&:has(> img)", "&:not(:has(> img))"),
            ("&:is(:hover)", "&:not(:is(:hover))"),
            (
                "&:is(:where(.group):hover *)",
                "&:not(:is(:where(.group):hover *))",
            ),
            ("&:not(:hover)", "&:is(:hover)"),
            (r#"&[data-x="&"]"#, r#"&:not([data-x="&"])"#),
        ] {
            assert_eq!(
                negate(&template(selector)),
                Some(template(expected)),
                "{selector}"
            );
        }
        assert_eq!(
            negate(&VariantApplication::SelectorSuffix(":focus")),
            Some(template("&:not(:focus)"))
        );
        for selector in [
            "&::before",
            "&:before",
            "&::file-selector-button, &",
            "& .foo",
            "& > *",
            ":is(& > *)",
            ":is(& *)",
            "&:hover, & > span",
            "& :hover",
            "& > .item",
            ":hover",
            ":root",
        ] {
            assert_eq!(negate(&template(selector)), None, "{selector}");
        }
    }

    #[test]
    fn media_queries_negate_types_conditions_and_lists() {
        for (condition, expected) in [
            ("(width >= 40rem)", "not (width >= 40rem)"),
            ("not (hover: hover)", "(hover: hover)"),
            ("print", "not print"),
            ("not print", "print"),
            ("screen and (color)", "not screen and (color)"),
            ("only screen and (color)", "not screen and (color)"),
            ("(a: 1) and (b: 2)", "not ((a: 1) and (b: 2))"),
        ] {
            assert_eq!(
                negate(&media(condition)),
                Some(media(expected)),
                "{condition}"
            );
        }
        assert_eq!(
            negate(&media("print, (color)")),
            Some(VariantApplication::Sequence(vec![
                media("not print"),
                media("not (color)")
            ]))
        );
        assert_eq!(
            negate(&media("(width >= 40rem), (color)")),
            Some(VariantApplication::Sequence(vec![
                media("not (width >= 40rem)"),
                media("not (color)")
            ]))
        );
    }

    #[test]
    fn support_container_and_runtime_conditions_toggle_negation() {
        for (condition, expected) in [
            ("(display:grid)", "not (display:grid)"),
            ("not (display:grid)", "(display:grid)"),
            ("selector(:has(a))", "not selector(:has(a))"),
            ("(a:b) and (c:d)", "not ((a:b) and (c:d))"),
        ] {
            assert_eq!(
                negate(&VariantApplication::Supports(condition.into())),
                Some(VariantApplication::Supports(expected.into())),
                "{condition}"
            );
        }
        for (condition, expected) in [
            ("(width >= 28rem)", "not (width >= 28rem)"),
            ("panel (width >= 28rem)", "panel not (width >= 28rem)"),
            ("panel not (width >= 28rem)", "panel (width >= 28rem)"),
            ("not (width >= 28rem)", "(width >= 28rem)"),
            ("style(--x: 1)", "not style(--x: 1)"),
        ] {
            assert_eq!(
                negate(&VariantApplication::Container(condition.into())),
                Some(VariantApplication::Container(expected.into())),
                "{condition}"
            );
        }
        assert_eq!(
            negate(&VariantApplication::Conditional("--on".into())),
            Some(VariantApplication::Conditional("!--on".into()))
        );
        assert_eq!(
            negate(&VariantApplication::Conditional("!--mode:dark".into())),
            Some(VariantApplication::Conditional("--mode:dark".into()))
        );
    }

    #[test]
    fn alternatives_and_conjunctions_follow_de_morgan() {
        let hover = VariantApplication::SelectorMedia {
            selector: "&:hover".into(),
            condition: "(hover: hover)".into(),
        };
        let not_hover = VariantApplication::Compound(vec![
            template("&:not(:hover)"),
            media("not (hover: hover)"),
        ]);
        assert_eq!(negate(&hover.clone()), Some(not_hover.clone()));
        assert_eq!(
            negate(&not_hover),
            Some(VariantApplication::Sequence(vec![
                media("(hover: hover)"),
                template("&:is(:hover)")
            ]))
        );
        assert_eq!(
            negate(&VariantApplication::Compound(vec![
                template("&:hover"),
                template("&:focus")
            ])),
            Some(template("&:not(:hover):not(:focus)"))
        );
        assert_eq!(
            negate(&VariantApplication::Compound(vec![
                hover,
                template("&:focus")
            ])),
            Some(VariantApplication::Compound(vec![
                template("&:not(:hover):not(:focus)"),
                VariantApplication::Sequence(vec![
                    media("not (hover: hover)"),
                    template("&:not(:focus)")
                ]),
            ]))
        );
    }

    #[test]
    fn declarations_starting_style_and_large_expansions_are_rejected() {
        assert_eq!(negate(&VariantApplication::StartingStyle), None);
        assert_eq!(
            negate(&VariantApplication::SelectorRule {
                selector: "&::before".into(),
                declarations: Vec::new(),
            }),
            None
        );
        assert_eq!(
            negate(&VariantApplication::Sequence(vec![
                media("print"),
                VariantApplication::StartingStyle
            ])),
            None
        );
        let pairs = |count: usize| {
            VariantApplication::Compound(
                (0..count)
                    .map(|index| {
                        VariantApplication::Sequence(vec![
                            media(&format!("(min-width: {index}px)")),
                            template(&format!("&:nth-child({index})")),
                        ])
                    })
                    .collect(),
            )
        };
        let within = pairs(5);
        let beyond = pairs(6);
        assert!(negate(&within).is_some());
        assert_eq!(negate(&beyond), None);
    }
}
