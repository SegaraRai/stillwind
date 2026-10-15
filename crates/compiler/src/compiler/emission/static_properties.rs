//! Finds literal custom properties that can be emitted once without runtime guards.

use std::collections::BTreeMap;

use lean_string::LeanStr;

use stillwind_ir::{ClassExpression, CustomPropertyEntry, GroupEntry, StyleInput};

/// Returns unguarded literal properties that occur once in the complete input tree.
/// Composition disables extraction because a producer may supply the same property.
pub(super) fn static_custom_properties(
    inputs: &[StyleInput],
) -> BTreeMap<LeanStr, CustomPropertyEntry> {
    let mut properties = BTreeMap::new();
    let mut composition = false;
    collect_static_inputs(inputs, false, &mut properties, &mut composition);
    if composition {
        return BTreeMap::new();
    }
    properties
        .into_iter()
        .filter_map(|(name, property)| property.map(|property| (name, property)))
        .collect()
}

fn collect_static_inputs(
    inputs: &[StyleInput],
    guarded: bool,
    properties: &mut BTreeMap<LeanStr, Option<CustomPropertyEntry>>,
    composition: &mut bool,
) {
    for input in inputs {
        match input {
            StyleInput::Array(inputs) => {
                collect_static_inputs(inputs, guarded, properties, composition);
            }
            StyleInput::Class(expression) => collect_static_class(expression, guarded, properties),
            StyleInput::Conditional { branches, .. } => {
                collect_static_inputs(branches.consequent(), true, properties, composition);
                if let Some(alternate) = branches.alternate() {
                    collect_static_inputs(alternate, true, properties, composition);
                }
            }
            StyleInput::Composition { .. } => *composition = true,
            _ => {}
        }
    }
}

fn collect_static_class(
    expression: &ClassExpression,
    guarded: bool,
    properties: &mut BTreeMap<LeanStr, Option<CustomPropertyEntry>>,
) {
    match expression {
        ClassExpression::Array(elements) => {
            for element in elements {
                collect_static_class(element, guarded, properties);
            }
        }
        ClassExpression::Conditional { branches, .. } => {
            for element in branches
                .consequent()
                .iter()
                .chain(branches.alternate().into_iter().flatten())
            {
                collect_static_class(element, true, properties);
            }
        }
        ClassExpression::Group { entries, .. } => {
            for entry in entries {
                match entry {
                    GroupEntry::Class(expression) => {
                        collect_static_class(expression, guarded, properties);
                    }
                    GroupEntry::CustomProperty(property) => {
                        let value =
                            (!guarded && property.static_value.is_some()).then(|| property.clone());
                        properties
                            .entry(property.name.value.clone())
                            .and_modify(|value| *value = None)
                            .or_insert(value);
                    }
                    GroupEntry::Attribute(_) => {}
                }
            }
        }
        _ => {}
    }
}
