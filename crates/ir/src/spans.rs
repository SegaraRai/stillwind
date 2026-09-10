//! Transactional conversion of IR coordinates at a frontend boundary.

use stillwind_source::{Span, Spanned};

use crate::{
    AnimationDefinition, AssetReference, HandleDefinition, HandleProjection, ModuleIr,
    ScopeDefinition, StyleInput,
};

impl ModuleIr {
    /// Maps every local source span, retaining this IR unchanged on failure.
    /// The frontend owns correspondence validation and keeps its provenance.
    ///
    /// # Errors
    /// Returns the first error from the supplied source correspondence function.
    pub fn try_map_spans<E>(
        &self,
        mut map: impl FnMut(Span) -> Result<Span, E>,
    ) -> Result<Self, E> {
        let mut result = self.clone();
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
    for frame in &mut animation.frames {
        map_value(&mut frame.selector, map)?;
        map_value(&mut frame.utilities, map)?;
    }
    Ok(())
}

fn map_asset<E>(
    asset: &mut AssetReference,
    map: &mut impl FnMut(Span) -> Result<Span, E>,
) -> Result<(), E> {
    map_value(&mut asset.name, map)
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
        StyleInput::UtilityString(value) => map_value(value, map)?,
        StyleInput::ConditionalUtilityString { guard, value, .. } => {
            *guard = map(*guard)?;
            map_value(value, map)?;
        }
        StyleInput::Attributes(entries) => {
            for entry in entries {
                map_value(&mut entry.name, map)?;
                entry.value = map(entry.value)?;
            }
        }
        StyleInput::SelectorUtilityString { selector, value } => {
            map_value(selector, map)?;
            map_value(value, map)?;
        }
        StyleInput::ConditionalSelectorUtilityString {
            selector,
            guard,
            value,
            ..
        } => {
            map_value(selector, map)?;
            *guard = map(*guard)?;
            map_value(value, map)?;
        }
        StyleInput::HandleSelectorUtilityString {
            handle,
            selector,
            value,
        } => {
            map_handle(handle, map)?;
            map_value(selector, map)?;
            map_value(value, map)?;
        }
        StyleInput::ConditionalHandleSelectorUtilityString {
            handle,
            selector,
            guard,
            value,
            ..
        } => {
            map_handle(handle, map)?;
            map_value(selector, map)?;
            *guard = map(*guard)?;
            map_value(value, map)?;
        }
        StyleInput::HandleProjection { handle, projection } => {
            map_handle(handle, map)?;
            map_projection(projection, map)?;
        }
        StyleInput::DynamicHandleProjection {
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
        StyleInput::ContainerQuery {
            handle,
            query,
            value,
        } => {
            map_handle(handle, map)?;
            map_value(query, map)?;
            map_value(value, map)?;
        }
        StyleInput::ScopeAttachment(scope) => map_scope(scope, map)?,
        StyleInput::HandleAttachment(handle) => map_handle(handle, map)?,
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
        StyleInput::ExternalClass(value) => *value = map(*value)?,
        StyleInput::AnimationUse { animation, options } => {
            map_asset(animation, map)?;
            map_value(options, map)?;
        }
        StyleInput::DynamicAnimationUse {
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
        StyleInput::SelectorAnimationUse {
            selector,
            animation,
            options,
        } => {
            map_value(selector, map)?;
            map_asset(animation, map)?;
            map_value(options, map)?;
        }
        StyleInput::DynamicSelectorAnimationUse {
            selector,
            value,
            animations,
            options,
        } => {
            map_value(selector, map)?;
            *value = map(*value)?;
            for animation in animations {
                map_asset(animation, map)?;
            }
            map_value(options, map)?;
        }
        StyleInput::ViewTransitionUse { transition, key } => {
            map_asset(transition, map)?;
            if let Some(key) = key {
                *key = map(*key)?;
            }
        }
        StyleInput::DynamicViewTransitionUse {
            value,
            transitions,
            key,
        } => {
            *value = map(*value)?;
            for transition in transitions {
                map_asset(transition, map)?;
            }
            if let Some(key) = key {
                *key = map(*key)?;
            }
        }
    }
    Ok(())
}
