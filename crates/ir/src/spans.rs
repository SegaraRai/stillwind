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
    AnimationBody, AnimationDefinition, AssetReference, ClassExpression, GroupEntry,
    HandleDefinition, HandleProjection, ModuleIr, ScopeDefinition, StyleInput, StyleVariant,
    ViewTransitionKey, ViewTransitionName,
};

impl ModuleIr {
    /// Maps every local source span, retaining this IR unchanged on failure. The
    /// frontend owns correspondence validation and keeps its provenance.
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
        for source in &mut result.imports.module_css {
            *source = map(*source)?;
        }
        for import in &mut result.imports.macros {
            import.span = map(import.span)?;
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
        for handle in &mut result.handle_definitions {
            map_handle(handle, &mut map)?;
        }
        for scope in &mut result.scope_definitions {
            map_scope(scope, &mut map)?;
        }
        for animation in &mut result.animation_declarations {
            map_animation(animation, &mut map)?;
        }
        for transition in &mut result.view_transition_declarations {
            map_value(&mut transition.name, &mut map)?;
            transition.call_span = map(transition.call_span)?;
            for value in [
                &mut transition.group,
                &mut transition.old,
                &mut transition.new,
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

fn map_value<E>(
    value: &mut Spanned<String>,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    value.span = map(value.span)?;
    Ok(())
}

fn map_scope<E>(
    scope: &mut ScopeDefinition,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    map_value(&mut scope.name, map)?;
    if let Some(label) = &mut scope.label {
        map_value(label, map)?;
    }
    scope.call_span = map(scope.call_span)?;
    Ok(())
}

fn map_handle<E>(
    handle: &mut HandleDefinition,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    map_value(&mut handle.name, map)?;
    if let Some(label) = &mut handle.label {
        map_value(label, map)?;
    }
    if let Some(scope) = &mut handle.scope {
        map_scope(scope, map)?;
    }
    handle.call_span = map(handle.call_span)?;
    Ok(())
}

fn map_animation<E>(
    animation: &mut AnimationDefinition,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    map_value(&mut animation.name, map)?;
    animation.call_span = map(animation.call_span)?;
    map_animation_body(&mut animation.body, map)
}

fn map_animation_body<E>(
    body: &mut AnimationBody,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    for frame in &mut body.frames {
        map_value(&mut frame.selector, map)?;
        map_value(&mut frame.utilities, map)?;
    }
    for branch in &mut body.branches {
        map_variant(&mut branch.variant, map)?;
        map_animation_body(&mut branch.body, map)?;
    }
    Ok(())
}

fn map_asset<E>(
    asset: &mut AssetReference,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    asset.span = map(asset.span)?;
    if let Some(declaration) = asset.local_declaration {
        let declaration = map(declaration)?;
        asset.identity.start = declaration.start.0;
        asset.identity.end = declaration.end.0;
        asset.local_declaration = Some(declaration);
    }
    Ok(())
}

fn map_projection<E>(
    projection: &mut HandleProjection,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    if let HandleProjection::Container(name) = projection {
        map_value(name, map)?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // Exhaustive match makes new IR variants require an explicit mapping policy.
fn map_input<E>(
    input: &mut StyleInput,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    match input {
        StyleInput::Class(expression) => map_class_expression(expression, map)?,
        StyleInput::ScopeAttachment(scope) => {
            map_scope(scope, map)?;
        }
        StyleInput::HandleAttachment(handle) => {
            map_handle(handle, map)?;
        }
        StyleInput::DynamicScopeAttachment { value, scopes } => {
            *value = map(*value)?;
            for scope in scopes {
                map_scope(scope, map)?;
            }
        }
        StyleInput::DynamicHandleAttachment { value, handles } => {
            *value = map(*value)?;
            for handle in handles {
                map_handle(handle, map)?;
            }
        }
        StyleInput::Composition { value, candidates } => {
            for candidate in candidates {
                let span = map(Span::new(value.file, candidate.start, candidate.end))?;
                candidate.start = span.start.0;
                candidate.end = span.end.0;
            }
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

fn map_class_expression<E>(
    expression: &mut ClassExpression,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    match expression {
        ClassExpression::Utilities(value) => map_value(value, map)?,
        ClassExpression::Array(elements) => {
            for element in elements {
                map_class_expression(element, map)?;
            }
        }
        ClassExpression::Conditional {
            test,
            consequent,
            alternate,
            ..
        } => {
            *test = map(*test)?;
            for expression in consequent.iter_mut().chain(alternate.iter_mut()) {
                map_class_expression(expression, map)?;
            }
        }
        ClassExpression::Variant { variant, inputs } => {
            map_variant(variant, map)?;
            for input in inputs {
                map_class_expression(input, map)?;
            }
        }
        ClassExpression::Group { span, entries } => {
            *span = map(*span)?;
            for entry in entries {
                map_group_entry(entry, map)?;
            }
        }
        ClassExpression::External { span, arguments } => {
            *span = map(*span)?;
            for argument in arguments {
                *argument = map(*argument)?;
            }
        }
        ClassExpression::HandleProjection { handle, projection } => {
            map_handle(handle, map)?;
            map_projection(projection, map)?;
        }
        ClassExpression::DynamicHandleProjection {
            value,
            handles,
            projection,
        } => {
            *value = map(*value)?;
            for handle in handles {
                map_handle(handle, map)?;
            }
            map_projection(projection, map)?;
        }
        ClassExpression::AnimationUse { animation, options } => {
            map_asset(animation, map)?;
            map_value(options, map)?;
        }
        ClassExpression::DynamicAnimationUse {
            value,
            animations,
            options,
        } => {
            *value = map(*value)?;
            for animation in animations {
                map_asset(animation, map)?;
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
        StyleVariant::Ref { handles, prefix }
        | StyleVariant::Container {
            handles,
            query: prefix,
        } => {
            map_value(prefix, map)?;
            for handle in handles {
                map_handle(handle, map)?;
            }
        }
    }
    Ok(())
}
