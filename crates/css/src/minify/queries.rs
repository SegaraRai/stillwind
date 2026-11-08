//! Optimizations of media queries, container queries, and `@supports` conditions.
//!
//! Range rewrites produce the interval syntax of Media Queries Level 4. Lightning CSS
//! lowers an interval to `min-` and `max-` features while printing when the browser
//! targets need that, so these rewrites do not depend on the targets.

use cssparser::{ParseError, Parser, Token};
use lightningcss::media_query::{
    MediaCondition, MediaFeature, MediaFeatureComparison, MediaFeatureId, MediaFeatureName,
    MediaFeatureValue, MediaQuery, Operator, QueryFeature,
};
use lightningcss::rules::container::ContainerCondition;
use lightningcss::rules::supports::SupportsCondition;

/// Shortens boolean media features and merges range pairs in `query`.
///
/// See [`shorten_boolean_feature`] and [`merge_ranges`] for the conditions.
pub(super) fn optimize_media_query(query: &mut MediaQuery<'_>) {
    if let Some(condition) = &mut query.condition {
        optimize_media_condition(condition);
    }
}

fn optimize_media_condition(condition: &mut MediaCondition<'_>) {
    match condition {
        MediaCondition::Feature(feature) => {
            shorten_boolean_feature(feature);
        }
        MediaCondition::Not(inner) => {
            optimize_media_condition(inner);
        }
        MediaCondition::Operation {
            operator,
            conditions,
        } => {
            conditions.iter_mut().for_each(optimize_media_condition);
            if *operator == Operator::And {
                merge_ranges(conditions);
            }
            if conditions.len() == 1
                && let Some(only) = conditions.pop()
            {
                *condition = only;
            }
        }
        MediaCondition::Unknown(_) => {}
    }
}

/// Merges range pairs in `condition`, a `@container` condition.
///
/// See [`merge_ranges`] for the conditions.
pub(super) fn optimize_container_condition(condition: &mut ContainerCondition<'_>) {
    match condition {
        ContainerCondition::Not(inner) => {
            optimize_container_condition(inner);
        }
        ContainerCondition::Operation {
            operator,
            conditions,
        } => {
            conditions.iter_mut().for_each(optimize_container_condition);
            if *operator == Operator::And {
                merge_ranges(conditions);
            }
            if conditions.len() == 1
                && let Some(only) = conditions.pop()
            {
                *condition = only;
            }
        }
        ContainerCondition::Feature(_)
        | ContainerCondition::Style(_)
        | ContainerCondition::ScrollState(_)
        | ContainerCondition::Unknown(_) => {}
    }
}

/// Rewrites `(hover:hover)` as `(hover)` and every other feature whose value is the only
/// value that makes the feature true in a boolean context.
///
/// A boolean context is false for `none`, `no-preference`, zero, and `false`, and true
/// for every other value. `(pointer:fine)` stays, because `(pointer)` also matches
/// `coarse`.
fn shorten_boolean_feature(feature: &mut MediaFeature<'_>) {
    let QueryFeature::Plain { name, value } = feature else {
        return;
    };
    let only_true = match value {
        MediaFeatureValue::Boolean(value) => *value,
        MediaFeatureValue::Ident(ident) => {
            matches!(name, MediaFeatureName::Standard(id)
                if only_true_ident(*id).is_some_and(|only| ident.0.eq_ignore_ascii_case(only)))
        }
        _ => false,
    };
    if only_true {
        *feature = QueryFeature::Boolean { name: name.clone() };
    }
}

/// The one keyword that makes `id` true in a boolean context, for features whose other
/// keywords are all false there.
fn only_true_ident(id: MediaFeatureId) -> Option<&'static str> {
    match id {
        MediaFeatureId::Hover | MediaFeatureId::AnyHover => Some("hover"),
        MediaFeatureId::PrefersReducedMotion
        | MediaFeatureId::PrefersReducedTransparency
        | MediaFeatureId::PrefersReducedData => Some("reduce"),
        MediaFeatureId::ForcedColors => Some("active"),
        MediaFeatureId::InvertedColors => Some("inverted"),
        _ => None,
    }
}

/// A condition of a `@media` or `@container` query that may be one feature.
trait QueryNode<'i>: Sized {
    type FeatureId: PartialEq + Clone;

    fn feature(&self) -> Option<&QueryFeature<'i, Self::FeatureId>>;

    fn feature_mut(&mut self) -> Option<&mut QueryFeature<'i, Self::FeatureId>>;

    fn into_feature(self) -> Option<QueryFeature<'i, Self::FeatureId>>;
}

impl<'i> QueryNode<'i> for MediaCondition<'i> {
    type FeatureId = MediaFeatureId;

    fn feature(&self) -> Option<&QueryFeature<'i, Self::FeatureId>> {
        match self {
            Self::Feature(feature) => Some(feature),
            _ => None,
        }
    }

    fn feature_mut(&mut self) -> Option<&mut QueryFeature<'i, Self::FeatureId>> {
        match self {
            Self::Feature(feature) => Some(feature),
            _ => None,
        }
    }

    fn into_feature(self) -> Option<QueryFeature<'i, Self::FeatureId>> {
        match self {
            Self::Feature(feature) => Some(feature),
            _ => None,
        }
    }
}

impl<'i> QueryNode<'i> for ContainerCondition<'i> {
    type FeatureId = lightningcss::rules::container::ContainerSizeFeatureId;

    fn feature(&self) -> Option<&QueryFeature<'i, Self::FeatureId>> {
        match self {
            Self::Feature(feature) => Some(feature),
            _ => None,
        }
    }

    fn feature_mut(&mut self) -> Option<&mut QueryFeature<'i, Self::FeatureId>> {
        match self {
            Self::Feature(feature) => Some(feature),
            _ => None,
        }
    }

    fn into_feature(self) -> Option<QueryFeature<'i, Self::FeatureId>> {
        match self {
            Self::Feature(feature) => Some(feature),
            _ => None,
        }
    }
}

/// Which end of a range a `(width>=40rem)` or `(width<60rem)` feature limits.
#[derive(Clone, Copy, PartialEq)]
enum Bound {
    Lower,
    Upper,
}

fn bound<'a, 'i, F>(
    feature: &'a QueryFeature<'i, F>,
) -> Option<(&'a MediaFeatureName<'i, F>, Bound)> {
    match feature {
        QueryFeature::Range { name, operator, .. } => match operator {
            MediaFeatureComparison::GreaterThan | MediaFeatureComparison::GreaterThanEqual => {
                Some((name, Bound::Lower))
            }
            MediaFeatureComparison::LessThan | MediaFeatureComparison::LessThanEqual => {
                Some((name, Bound::Upper))
            }
            MediaFeatureComparison::Equal => None,
        },
        _ => None,
    }
}

/// Merges a lower and an upper range of the same feature, which `conditions` joins with
/// `and`, into one interval such as `(40rem<=width<60rem)`.
///
/// The interval takes the place of the first range of the pair, and the second range is
/// removed. `and` is commutative, so the order of the other conditions does not matter.
fn merge_ranges<'i, C: QueryNode<'i>>(conditions: &mut Vec<C>) {
    let mut index = 0;
    while index < conditions.len() {
        let partner = conditions[index]
            .feature()
            .and_then(bound)
            .and_then(|(name, side)| {
                conditions[index + 1..].iter().position(|other| {
                    other
                        .feature()
                        .and_then(bound)
                        .is_some_and(|(other_name, other_side)| {
                            other_name == name && other_side != side
                        })
                })
            });
        if let Some(offset) = partner
            && let Some(second) = conditions.remove(index + 1 + offset).into_feature()
            && let Some(first) = conditions[index].feature_mut()
            && let Some(merged) = interval(first, second)
        {
            *first = merged;
        }
        index += 1;
    }
}

/// Builds the interval that `first` and `second` limit together. Both must be ranges of
/// the same feature that limit opposite ends; otherwise the result is `None`.
fn interval<'i, F: Clone>(
    first: &QueryFeature<'i, F>,
    second: QueryFeature<'i, F>,
) -> Option<QueryFeature<'i, F>> {
    let (
        QueryFeature::Range {
            name,
            operator: first_operator,
            value: first_value,
        },
        QueryFeature::Range {
            operator: second_operator,
            value: second_value,
            ..
        },
    ) = (first, second)
    else {
        return None;
    };
    let ((lower, start), (upper, end)) = match bound(first)?.1 {
        Bound::Lower => (
            (first_operator, first_value.clone()),
            (&second_operator, second_value),
        ),
        Bound::Upper => (
            (&second_operator, second_value),
            (first_operator, first_value.clone()),
        ),
    };
    // A lower range `(width>=a)` reads `a<=width` at the start of an interval.
    let start_operator = match lower {
        MediaFeatureComparison::GreaterThan => MediaFeatureComparison::LessThan,
        MediaFeatureComparison::GreaterThanEqual => MediaFeatureComparison::LessThanEqual,
        _ => return None,
    };
    let end_operator = match upper {
        MediaFeatureComparison::LessThan => MediaFeatureComparison::LessThan,
        MediaFeatureComparison::LessThanEqual => MediaFeatureComparison::LessThanEqual,
        _ => return None,
    };
    Some(QueryFeature::Interval {
        name: name.clone(),
        start,
        start_operator,
        end,
        end_operator,
    })
}

/// Drops whitespace next to commas in the declarations of `condition`, such as
/// `(color:color-mix(in lab, red, red))`.
///
/// Lightning CSS stores these declaration values as source text and prints them
/// unchanged. Other tokens and their spelling stay as written, so a value such as a
/// `color-mix()` call still tests the same feature.
pub(super) fn optimize_supports_condition(condition: &mut SupportsCondition<'_>) {
    match condition {
        SupportsCondition::Not(inner) => {
            optimize_supports_condition(inner);
        }
        SupportsCondition::And(conditions) | SupportsCondition::Or(conditions) => {
            conditions.iter_mut().for_each(optimize_supports_condition);
        }
        SupportsCondition::Declaration { value, .. } => {
            let mut output = String::with_capacity(value.len());
            copy_without_comma_whitespace(&mut Parser::new(value), &mut output);
            if output.len() < value.len() {
                *value = output.into();
            }
        }
        SupportsCondition::Selector(_) | SupportsCondition::Unknown(_) => {}
    }
}

/// Copies the source text of the tokens that `parser` reads to `output`, without the
/// whitespace next to commas. Nested blocks are copied the same way.
fn copy_without_comma_whitespace(parser: &mut Parser<'_>, output: &mut String) {
    let mut whitespace = "";
    let mut after_comma = false;
    loop {
        let start = parser.position();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            break;
        };
        let (separator, block) = match token {
            Token::WhiteSpace(_) => {
                whitespace = parser.slice_from(start);
                continue;
            }
            Token::Comma => (true, false),
            Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock => (false, true),
            _ => (false, false),
        };
        if !separator && !after_comma {
            output.push_str(whitespace);
        }
        whitespace = "";
        after_comma = separator;
        output.push_str(parser.slice_from(start));
        if block {
            let mut inner_end = parser.position();
            let _: Result<(), ParseError<()>> = parser.parse_nested_block(|nested| {
                copy_without_comma_whitespace(nested, output);
                inner_end = nested.position();
                Ok(())
            });
            // The closing bracket is missing when the value ends inside the block.
            output.push_str(parser.slice_from(inner_end));
        }
    }
    if !after_comma {
        output.push_str(whitespace);
    }
}
