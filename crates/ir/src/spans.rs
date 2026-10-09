//! Transactional conversion of IR coordinates at a frontend boundary.
//!
//! A frontend that analyzes a converted or shadow buffer produces spans in that
//! buffer's coordinates, but every consumer downstream edits the original file. This
//! module walks an entire `ModuleIr` and rewrites each span through one supplied
//! mapping function.
//!
//! The conversion is all-or-nothing: the receiver is untouched and the first error is
//! returned, because a partially mapped IR would silently mix two coordinate systems.
//! Frontends own the mapping itself, including correspondence validation and their
//! provenance records; this module only enforces that no span is forgotten.
//!
//! [`map_input`] deliberately matches exhaustively with no wildcard arm so that a new
//! IR variant fails to compile until its mapping policy is stated.

use stillwind_source::{Span, Spanned};

use crate::{
    AnchorScope, AssetReference, ClassExpression, ConditionalBranches, GroupEntry, KeyframesBody,
    KeyframesDefinition, ModuleIr, RefDefinition, RefProjection, RuleExpression, StyleInput,
    StyleVariant, ViewTransitionKey, ViewTransitionName,
};

impl ModuleIr {
    /// Returns a copy with local analysis spans mapped to original source coordinates
    /// while preserving declaration identities. The frontend owns correspondence
    /// validation and keeps its provenance.
    ///
    /// The mapping function is called for each stored span occurrence, and the same
    /// coordinates can be visited more than once when roles share a span; it sees only
    /// the coordinates and must map them consistently.
    ///
    /// # Errors
    ///
    /// Returns the first error from the supplied source correspondence function.
    pub fn try_map_spans<E>(
        &self,
        mut map: impl FnMut(Span) -> Result<Span, E>,
    ) -> Result<Self, E> {
        let mut result = self.clone();
        for span in &mut result.imports.consumed_uses {
            *span = map(*span)?;
        }
        for source in &mut result.imports.module_css {
            *source = map(*source)?;
        }
        for import in &mut result.imports.macros {
            import.span = map(import.span)?;
            import.source_span = map(import.source_span)?;
            for specifier in &mut import.specifiers {
                specifier.span = map(specifier.span)?;
                for reference in &mut specifier.references {
                    *reference = map(*reference)?;
                }
            }
        }
        for call in &mut result.style_calls {
            call.span = map(call.span)?;
            for input in &mut call.inputs {
                map_input(input, &mut map)?;
            }
        }
        for cva in &mut result.cva_calls {
            cva.span = map(cva.span)?;
            cva.callee_span = map(cva.callee_span)?;
            for value in &mut cva.values {
                value.span = map(value.span)?;
                for input in &mut value.inputs {
                    map_input(input, &mut map)?;
                }
            }
        }
        for definition in &mut result.ref_definitions {
            map_ref(definition, &mut map)?;
        }
        for keyframes in &mut result.keyframes_declarations {
            map_animation(keyframes, &mut map)?;
        }
        for transition in &mut result.view_transition_declarations {
            map_value(&mut transition.binding, &mut map)?;
            transition.call_span = map(transition.call_span)?;
            if let Some(label) = &mut transition.label {
                map_value(label, &mut map)?;
            }
            for value in [
                &mut transition.group,
                &mut transition.old,
                &mut transition.new,
                &mut transition.image_pair,
            ]
            .into_iter()
            .flatten()
            {
                map_value(value, &mut map)?;
            }
        }
        Ok(result)
    }
}

fn map_value<T, E>(
    value: &mut Spanned<T>,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    value.span = map(value.span)?;
    Ok(())
}

fn map_ref<E>(
    definition: &mut RefDefinition,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    map_value(&mut definition.binding, map)?;
    if let Some(label) = &mut definition.label {
        map_value(label, map)?;
    }
    definition.call_span = map(definition.call_span)?;
    Ok(())
}

fn map_animation<E>(
    keyframes: &mut KeyframesDefinition,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    map_value(&mut keyframes.binding, map)?;
    keyframes.call_span = map(keyframes.call_span)?;
    if let Some(label) = &mut keyframes.label {
        map_value(label, map)?;
    }
    map_keyframes_body(&mut keyframes.body, map)
}

fn map_keyframes_body<E>(
    body: &mut KeyframesBody,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    for frame in &mut body.frames {
        map_value(&mut frame.selector, map)?;
        map_value(&mut frame.utilities, map)?;
    }
    for branch in &mut body.branches {
        map_variant(&mut branch.variant, map)?;
        map_keyframes_body(&mut branch.body, map)?;
    }
    Ok(())
}

fn map_asset<E>(
    asset: &mut AssetReference,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    asset.span = map(asset.span)?;
    if let Some(label) = &mut asset.label {
        map_value(label, map)?;
    }
    Ok(())
}

/// Rewrites the spans in an explicit container type or anchor scope in place.
///
/// `map` must accept the coordinate space and file identity of each stored span.
/// Projections without arguments need no mapping.
///
/// # Errors
///
/// Returns the first error from `map`. Spans already rewritten keep their new values.
fn map_projection<E>(
    projection: &mut RefProjection,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    if let RefProjection::AsContainer(Some(name)) = projection {
        map_value(name, map)?;
    }
    if let RefProjection::AsAnchor(Some(scope)) = projection {
        map_anchor_scope(scope, map)?;
    }
    Ok(())
}

/// Rewrites the declaration spans of each ref in `scope` in place.
///
/// `map` must handle the file identities and coordinate spaces of those declarations.
/// Keyword scopes need no mapping.
///
/// # Errors
///
/// Returns the first error from `map`. Refs or fields already rewritten keep their
/// new values.
fn map_anchor_scope<E>(
    scope: &mut AnchorScope,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    if let AnchorScope::Refs(refs) = scope {
        for definition in refs {
            map_ref(definition, map)?;
        }
    }
    Ok(())
}

/// Rewrites every span in one root input. Returns success after all occurrences have
/// been mapped.
///
/// # Errors
///
/// Returns the first error from the supplied mapping function.
#[allow(clippy::too_many_lines)] // Exhaustive match makes new IR variants require an explicit mapping policy.
fn map_input<E>(
    input: &mut StyleInput,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    match input {
        StyleInput::Array(inputs) => {
            for input in inputs {
                map_input(input, map)?;
            }
        }
        StyleInput::Conditional { test, branches } => {
            *test = map(*test)?;
            match branches {
                ConditionalBranches::And { consequent } => {
                    for input in consequent {
                        map_input(input, map)?;
                    }
                }
                ConditionalBranches::Ternary {
                    consequent,
                    alternate,
                } => {
                    for input in consequent.iter_mut().chain(alternate) {
                        map_input(input, map)?;
                    }
                }
            }
        }
        StyleInput::Class(expression) => map_class_expression(expression, map)?,
        StyleInput::RefAttachment { definition, value } => {
            *value = map(*value)?;
            map_ref(definition, map)?;
        }
        StyleInput::DynamicRefAttachment { value, refs } => {
            *value = map(*value)?;
            for definition in refs {
                map_ref(definition, map)?;
            }
        }
        StyleInput::Composition { value, .. } => {
            *value = map(*value)?;
        }
        StyleInput::ViewTransitionUse { transition, name } => {
            map_asset(transition, map)?;
            map_view_transition_name(name, map)?;
        }
        StyleInput::DynamicViewTransitionUse {
            value,
            transitions,
            name,
        } => {
            *value = map(*value)?;
            for transition in transitions {
                map_asset(transition, map)?;
            }
            map_view_transition_name(name, map)?;
        }
    }
    Ok(())
}

/// Projects the source spans a view-transition use names itself with.
fn map_view_transition_name<E>(
    name: &mut ViewTransitionName,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    if let ViewTransitionName::Key(ViewTransitionKey::Dynamic(key)) = name {
        *key = map(*key)?;
    }
    Ok(())
}

fn map_group_entry<E>(
    entry: &mut GroupEntry,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    match entry {
        GroupEntry::Class(expression) => {
            map_class_expression(expression, map)?;
        }
        GroupEntry::Attribute(attribute) => {
            map_value(&mut attribute.name, map)?;
            attribute.value = map(attribute.value)?;
        }
        GroupEntry::CustomProperty(property) => {
            map_value(&mut property.name, map)?;
            property.value = map(property.value)?;
        }
    }
    Ok(())
}

/// Rewrites every span in one mixed class expression. Returns success after its
/// descendants are mapped.
///
/// # Errors
///
/// Returns the first error from the supplied mapping function.
fn map_class_expression<E>(
    expression: &mut ClassExpression,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    match expression {
        ClassExpression::Rule(rule) => {
            map_rule_expression(rule, map)?;
        }
        ClassExpression::Array(elements) => {
            for element in elements {
                map_class_expression(element, map)?;
            }
        }
        ClassExpression::Conditional { test, branches } => {
            *test = map(*test)?;
            match branches {
                ConditionalBranches::And { consequent } => {
                    for expression in consequent {
                        map_class_expression(expression, map)?;
                    }
                }
                ConditionalBranches::Ternary {
                    consequent,
                    alternate,
                } => {
                    for expression in consequent.iter_mut().chain(alternate.iter_mut()) {
                        map_class_expression(expression, map)?;
                    }
                }
            }
        }
        ClassExpression::Group { span, entries } => {
            *span = map(*span)?;
            for entry in entries {
                map_group_entry(entry, map)?;
            }
        }
        ClassExpression::ClassValue { span, arguments } => {
            *span = map(*span)?;
            for argument in arguments {
                *argument = map(*argument)?;
            }
        }
    }
    Ok(())
}

/// Rewrites every span in one variant-capable rule subtree. Returns success after its
/// descendants are mapped.
///
/// # Errors
///
/// Returns the first error from the supplied mapping function.
fn map_rule_expression<E>(
    expression: &mut RuleExpression,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    match expression {
        RuleExpression::Utilities(value) => {
            map_value(value, map)?;
        }
        RuleExpression::Array(elements) => {
            for element in elements {
                map_rule_expression(element, map)?;
            }
        }
        RuleExpression::Group { span, inputs } => {
            *span = map(*span)?;
            for input in inputs {
                map_rule_expression(input, map)?;
            }
        }
        RuleExpression::Conditional { test, branches } => {
            *test = map(*test)?;
            match branches {
                ConditionalBranches::And { consequent } => {
                    for element in consequent {
                        map_rule_expression(element, map)?;
                    }
                }
                ConditionalBranches::Ternary {
                    consequent,
                    alternate,
                } => {
                    for element in consequent.iter_mut().chain(alternate.iter_mut()) {
                        map_rule_expression(element, map)?;
                    }
                }
            }
        }
        RuleExpression::Variant { variant, inputs } => {
            map_variant(variant, map)?;
            for input in inputs {
                map_rule_expression(input, map)?;
            }
        }
        RuleExpression::AnchorScope { scope, span } => {
            *span = map(*span)?;
            map_anchor_scope(scope, map)?;
        }
        RuleExpression::RefProjection {
            definition,
            value,
            projection,
        } => {
            map_ref(definition, map)?;
            *value = map(*value)?;
            map_projection(projection, map)?;
        }
        RuleExpression::DynamicRefProjection {
            value,
            refs,
            projection,
        } => {
            *value = map(*value)?;
            for definition in refs {
                map_ref(definition, map)?;
            }
            map_projection(projection, map)?;
        }
        RuleExpression::ViewTransitionUse { transition, name } => {
            map_asset(transition, map)?;
            map_view_transition_name(name, map)?;
        }
        RuleExpression::DynamicViewTransitionUse {
            value,
            transitions,
            name,
        } => {
            *value = map(*value)?;
            for transition in transitions {
                map_asset(transition, map)?;
            }
            map_view_transition_name(name, map)?;
        }
        RuleExpression::KeyframesUse { keyframes, options } => {
            map_asset(keyframes, map)?;
            map_value(options, map)?;
        }
        RuleExpression::DynamicKeyframesUse {
            value,
            keyframes,
            options,
        } => {
            *value = map(*value)?;
            for keyframes in keyframes {
                map_asset(keyframes, map)?;
            }
            map_value(options, map)?;
        }
    }
    Ok(())
}

fn map_variant<E>(
    variant: &mut StyleVariant,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    match variant {
        StyleVariant::Prefix(prefix) => {
            map_value(prefix, map)?;
        }
        StyleVariant::Ref { condition, scopes } => {
            for condition in std::iter::once(condition).chain(scopes) {
                map_value(&mut condition.prefix, map)?;
                for definition in &mut condition.refs {
                    map_ref(definition, map)?;
                }
            }
        }
        StyleVariant::Container {
            refs,
            query: prefix,
        } => {
            map_value(prefix, map)?;
            for definition in refs {
                map_ref(definition, map)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use stillwind_source::FileId;

    use super::*;

    fn utilities(text: &str, start: u32) -> ClassExpression {
        RuleExpression::Utilities(Spanned::new(
            text.to_owned(),
            Span::new(FileId(1), start, start + u32::try_from(text.len()).unwrap()),
        ))
        .into()
    }

    /// Returns the utility text and span inside a rule-valued class fixture.
    fn utility(expression: &ClassExpression) -> &Spanned<String> {
        let ClassExpression::Rule(rule) = expression else {
            panic!("rule")
        };
        let RuleExpression::Utilities(value) = rule.as_ref() else {
            panic!("utilities")
        };
        value
    }

    /// Every span-bearing node is visited in source order, and each mapped span
    /// replaces the original.
    #[test]
    fn maps_every_span_in_the_tree() {
        let mut expression = ClassExpression::Array(vec![
            utilities("a", 0),
            ClassExpression::Conditional {
                test: Span::new(FileId(1), 10, 14),
                branches: ConditionalBranches::Ternary {
                    consequent: vec![ClassExpression::Array(vec![utilities("b", 20)])],
                    alternate: vec![utilities("c", 30)],
                },
            },
        ]);
        let mut seen = Vec::new();
        map_class_expression(&mut expression, &mut |span| {
            seen.push(span.start.0);
            Ok::<_, ()>(Span::new(span.file, span.start.0 + 100, span.end.0 + 100))
        })
        .unwrap();
        assert_eq!(seen, [0, 10, 20, 30]);
        let ClassExpression::Array(elements) = &expression else {
            panic!("array")
        };
        let first = utility(&elements[0]);
        assert_eq!(first.span, Span::new(FileId(1), 100, 101));
        let ClassExpression::Conditional { test, branches, .. } = &elements[1] else {
            panic!("conditional")
        };
        assert_eq!(*test, Span::new(FileId(1), 110, 114));
        let ClassExpression::Array(inner) = &branches.consequent()[0] else {
            panic!("array")
        };
        let nested = utility(&inner[0]);
        assert_eq!(nested.span, Span::new(FileId(1), 120, 121));
    }

    /// The walk stops at the first failure; nodes mapped before it keep their new
    /// spans, so a failed mapping must be discarded by the caller.
    #[test]
    fn stops_at_the_first_mapping_failure() {
        let mut expression = ClassExpression::Array(vec![utilities("a", 0), utilities("b", 10)]);
        let mut visited = 0;
        let result = map_class_expression(&mut expression, &mut |span| {
            visited += 1;
            if span.start.0 == 10 {
                Err(())
            } else {
                Ok(Span::new(span.file, span.start.0 + 1, span.end.0 + 1))
            }
        });
        assert_eq!(result, Err(()));
        assert_eq!(visited, 2);
        let ClassExpression::Array(elements) = &expression else {
            panic!("array")
        };
        let first = utility(&elements[0]);
        assert_eq!(first.span, Span::new(FileId(1), 1, 2));
        let second = utility(&elements[1]);
        assert_eq!(second.span, Span::new(FileId(1), 10, 11));
    }
}
