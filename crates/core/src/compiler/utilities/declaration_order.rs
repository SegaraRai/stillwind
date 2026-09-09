use std::collections::{BTreeMap, BTreeSet, HashMap};

use stillwind_css::{ThemeIndex, declaration_format, fold_calc_expressions};
use stillwind_tailwind::{VariantDeclaration, builtin_declaration_formats};

use super::resolve::CompiledUtility;

pub(in crate::compiler) fn prepend_missing_declarations(
    declaration: &mut String,
    supporting: &[VariantDeclaration],
) {
    let declared = declaration
        .split_terminator(';')
        .filter_map(|declaration| declaration.split_once(':').map(|(name, _)| name.trim()))
        .collect::<BTreeSet<_>>();
    let supporting = supporting
        .iter()
        .filter(|candidate| !declared.contains(candidate.property.as_str()))
        .fold(String::new(), |mut output, candidate| {
            output.push_str(&candidate.property);
            output.push(':');
            output.push_str(&candidate.value);
            output.push(';');
            output
        });
    declaration.insert_str(0, &supporting);
}

#[cfg(test)]
fn combine_declarations(utilities: &[&CompiledUtility]) -> String {
    combine_declarations_with_formats(utilities, builtin_declaration_formats())
}

pub(in crate::compiler) fn combine_declarations_for_theme(
    utilities: &[&CompiledUtility],
    theme: &ThemeIndex,
) -> String {
    let formats = if theme.declaration_formats.is_empty() {
        builtin_declaration_formats()
    } else {
        &theme.declaration_formats
    };
    combine_declarations_with_formats(utilities, formats)
}

pub(in crate::compiler) fn combine_declarations_with_formats(
    utilities: &[&CompiledUtility],
    formats: &BTreeMap<String, String>,
) -> String {
    let mut declarations = String::new();
    let mut composed = HashMap::<String, usize>::new();
    let mut ordered = Vec::<String>::new();
    for utility in order_builtin_utilities(utilities.to_vec()) {
        for declaration in utility.declaration.split_terminator(';') {
            let compose_space = declaration
                .split_once(':')
                .and_then(|(property, _)| declaration_format(formats, property))
                .is_some_and(|format| {
                    format
                        .split_whitespace()
                        .any(|item| item == "compose-space")
                });
            if compose_space {
                if let Some((property, value)) = declaration.split_once(':') {
                    if let Some(index) = composed.get(property).copied() {
                        let existing = &mut ordered[index];
                        existing.push(' ');
                        existing.push_str(value);
                    } else {
                        composed.insert(property.to_owned(), ordered.len());
                        ordered.push(declaration.to_owned());
                    }
                }
            } else if !declaration.is_empty() {
                let declaration = if let Some((property, value)) = declaration.split_once(':')
                    && value.is_empty()
                    && declaration_format(formats, property)
                        .is_some_and(|format| format == "empty-space")
                {
                    format!("{property}: ")
                } else if let Some((property, value)) = declaration.split_once(':')
                    && declaration_format(formats, property).is_some_and(|format| {
                        format.split_whitespace().any(|item| item == "comma-space")
                    })
                {
                    format!("{property}:{}", value.replace(",var(", ", var("))
                } else {
                    declaration.to_owned()
                };
                // Keep fallback `var()` comma spacing generic across all
                // declaration values.
                let declaration = declaration.replace(",var(", ", var(");
                let declaration = fold_calc_expressions(&declaration);
                ordered.push(declaration);
            }
        }
    }
    for declaration in ordered {
        declarations.push_str(&declaration);
        declarations.push(';');
    }
    declarations
}

pub(in crate::compiler) fn order_builtin_utilities(
    mut utilities: Vec<&CompiledUtility>,
) -> Vec<&CompiledUtility> {
    let positions = utilities
        .iter()
        .enumerate()
        .filter_map(|(index, utility)| utility.builtin_order.map(|_| index))
        .collect::<Vec<_>>();
    let mut builtins = positions
        .iter()
        .map(|index| utilities[*index])
        .collect::<Vec<_>>();
    builtins.sort_by(|left, right| {
        left.builtin_order
            .cmp(&right.builtin_order)
            .then_with(|| left.candidate.cmp(&right.candidate))
    });
    for (position, utility) in positions.into_iter().zip(builtins) {
        utilities[position] = utility;
    }
    utilities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_conflicting_shorthands_like_tailwind() {
        let declarations = [
            CompiledUtility {
                variants: Vec::new(),
                declaration: "margin-left:0.5rem;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
                builtin_order: Some(2),
                candidate: "ml-2".to_owned(),
            },
            CompiledUtility {
                variants: Vec::new(),
                declaration: "margin-inline:0.75rem;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
                builtin_order: Some(1),
                candidate: "mx-3".to_owned(),
            },
            CompiledUtility {
                variants: Vec::new(),
                declaration: "margin:1rem;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
                builtin_order: Some(0),
                candidate: "m-4".to_owned(),
            },
        ];

        let actual = combine_declarations(&declarations.iter().collect::<Vec<_>>());
        assert_eq!(
            actual,
            "margin:1rem;margin-inline:0.75rem;margin-left:0.5rem;"
        );
    }

    #[test]
    fn keeps_composed_properties_at_their_cascade_position() {
        let declarations = [
            CompiledUtility {
                variants: Vec::new(),
                declaration: "all:unset;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
                builtin_order: Some(usize::MAX),
                candidate: "[all:unset]".to_owned(),
            },
            CompiledUtility {
                variants: Vec::new(),
                declaration: "--tw-skew-x:skewX(6deg);transform:var(--tw-skew-x,);".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
                builtin_order: Some(1),
                candidate: "skew-x-6".to_owned(),
            },
        ];

        let actual = combine_declarations(&declarations.iter().collect::<Vec<_>>());
        assert_eq!(
            actual,
            "--tw-skew-x:skewX(6deg);transform:var(--tw-skew-x,);all:unset;"
        );
    }

    #[test]
    fn keeps_unrelated_declarations_stable() {
        let declarations = [
            CompiledUtility {
                variants: Vec::new(),
                declaration: "color:red;display:block;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
                builtin_order: None,
                candidate: String::new(),
            },
            CompiledUtility {
                variants: Vec::new(),
                declaration: "opacity:0.5;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
                builtin_order: None,
                candidate: String::new(),
            },
        ];

        let actual = combine_declarations(&declarations.iter().collect::<Vec<_>>());
        assert_eq!(actual, "color:red;display:block;opacity:0.5;");
    }

    #[test]
    fn supporting_declarations_preserve_explicit_values_and_support_order() {
        let mut declaration = "color:red;width:1px;".into();
        let supporting = [
            VariantDeclaration {
                property: "color".into(),
                value: "blue".into(),
            },
            VariantDeclaration {
                property: "display".into(),
                value: "grid".into(),
            },
        ];
        prepend_missing_declarations(&mut declaration, &supporting);
        assert_eq!(declaration, "display:grid;color:red;width:1px;");
        prepend_missing_declarations(&mut declaration, &supporting);
        assert_eq!(declaration, "display:grid;color:red;width:1px;");
    }
}
