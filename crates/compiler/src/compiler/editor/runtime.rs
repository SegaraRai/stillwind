//! Collects authored keys whose view-transition names are supplied at runtime.

use std::collections::BTreeMap;

use stillwind_ir::{
    ClassExpression, GroupEntry, RuleExpression, StyleInput, ViewTransitionKey, ViewTransitionName,
};
use stillwind_source::Span;

/// Adds spans of dynamic view-transition keys inside `input`, including both
/// conditional branches. Keys use inclusive UTF-8 byte starts in one original module,
/// so repeated visits to the same occurrence replace its entry and iteration follows
/// source order. Static keys and runtime-selected asset receivers contribute no spans.
pub(super) fn collect_runtime_names(input: &StyleInput, keys: &mut BTreeMap<u32, Span>) {
    match input {
        StyleInput::Class(class) => collect_class(class, keys),
        StyleInput::Array(inputs) => {
            for input in inputs {
                collect_runtime_names(input, keys);
            }
        }
        StyleInput::Conditional { branches, .. } => {
            for input in branches
                .consequent()
                .iter()
                .chain(branches.alternate().into_iter().flatten())
            {
                collect_runtime_names(input, keys);
            }
        }
        StyleInput::ViewTransitionUse { name, .. }
        | StyleInput::DynamicViewTransitionUse { name, .. } => {
            collect_key(name, keys);
        }
        _ => {}
    }
}

/// Adds dynamic keys from rule-bearing class entries, arrays, and both conditional
/// branches to `keys`. Class values and group text contribute no spans.
fn collect_class(class: &ClassExpression, keys: &mut BTreeMap<u32, Span>) {
    match class {
        ClassExpression::Rule(rule) => {
            collect_rule(rule, keys);
        }
        ClassExpression::Array(classes) => {
            for class in classes {
                collect_class(class, keys);
            }
        }
        ClassExpression::Conditional { branches, .. } => {
            for class in branches
                .consequent()
                .iter()
                .chain(branches.alternate().into_iter().flatten())
            {
                collect_class(class, keys);
            }
        }
        ClassExpression::Group { entries, .. } => {
            for entry in entries {
                if let GroupEntry::Class(class) = entry {
                    collect_class(class, keys);
                }
            }
        }
        ClassExpression::ClassValue { .. } => {}
    }
}

/// Adds dynamic keys from transition uses nested in `rule`, retaining keys from both
/// conditional branches. Variant and group wrappers do not alter the original key
/// spans.
fn collect_rule(rule: &RuleExpression, keys: &mut BTreeMap<u32, Span>) {
    match rule {
        RuleExpression::Array(inputs)
        | RuleExpression::Group { inputs, .. }
        | RuleExpression::Variant { inputs, .. } => {
            for input in inputs {
                collect_rule(input, keys);
            }
        }
        RuleExpression::Conditional { branches, .. } => {
            for rule in branches
                .consequent()
                .iter()
                .chain(branches.alternate().into_iter().flatten())
            {
                collect_rule(rule, keys);
            }
        }
        RuleExpression::ViewTransitionUse { name, .. }
        | RuleExpression::DynamicViewTransitionUse { name, .. } => {
            collect_key(name, keys);
        }
        _ => {}
    }
}

/// Inserts a dynamic key's original span by its UTF-8 byte start. Static and automatic
/// names leave `keys` unchanged.
fn collect_key(name: &ViewTransitionName, keys: &mut BTreeMap<u32, Span>) {
    if let ViewTransitionName::Key(ViewTransitionKey::Dynamic(span)) = name {
        keys.insert(span.start.0, *span);
    }
}
