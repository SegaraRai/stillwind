//! Declaration-level ordering inside one rule.
//!
//! Built-in utilities are reordered into their registry definition order, which is what makes
//! shorthand and longhand precedence reproducible; a user definition records no position and stays
//! where the author wrote it. Two equal positions fall back to the candidate spelling, so output is
//! stable for a given input. Combining, rather than concatenating, the declarations is also where
//! declaration formats apply and where `calc()` expressions are folded, so a caller must always go
//! through `combine_declarations_for_theme` instead of joining declaration strings itself.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use stillwind_css::functions::find_css_functions;
use stillwind_css::{declaration_ranges, fold_calc_expressions};
use stillwind_design_system::{
    ThemeIndex, VariantDeclaration, builtin_declaration_formats, declaration_format,
};

use super::resolve::CompiledUtility;

/// Prepends the declarations a variant contributes, skipping properties the body already sets.
///
/// A property the body writes explicitly keeps its authored position and value, and repeating the
/// call is a no-op, so a variant that is applied through several paths cannot accumulate
/// duplicates.
pub(in crate::compiler) fn prepend_missing_declarations(
    declaration: &mut String,
    supporting: &[VariantDeclaration],
) {
    let declared = declaration_ranges(declaration)
        .into_iter()
        .map(|range| &declaration[range])
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

/// Test-only combination against the built-in fixture registry.
#[cfg(test)]
fn combine_declarations(utilities: &[&CompiledUtility]) -> String {
    use stillwind_design_system::parse_declaration_formats;

    combine_declarations_with_formats(
        utilities,
        &parse_declaration_formats(crate::test_defaults::BUILTIN_REGISTRY_CSS),
    )
}

/// Combines the declarations of several utilities into one block body.
//
// A theme that carries its own declaration formats overrides the built-in set entirely, which is
// how a project can redefine how a property composes without patching the registry.
pub(in crate::compiler) fn combine_declarations_for_theme(
    utilities: &[&CompiledUtility],
    theme: &ThemeIndex,
) -> String {
    let formats = if theme.declaration_formats.is_empty() {
        builtin_declaration_formats(theme)
    } else {
        &theme.declaration_formats
    };
    combine_declarations_with_formats(utilities, formats)
}

/// The combination itself, with declaration formats supplied by the caller.
///
/// Formats decide three behaviors: `compose-space` appends a later value to the property's first
/// occurrence instead of repeating the property, `empty-space` keeps an intentionally empty value
/// visible, and `comma-space` normalizes fallback `var()` spacing. Declarations that no format
/// mentions are emitted verbatim in their ordered position.
pub(in crate::compiler) fn combine_declarations_with_formats(
    utilities: &[&CompiledUtility],
    formats: &BTreeMap<String, String>,
) -> String {
    let mut declarations = String::new();
    let mut composed = HashMap::<String, usize>::new();
    let mut ordered = Vec::<String>::new();
    for utility in order_builtin_utilities(utilities.to_vec()) {
        for range in declaration_ranges(&utility.declaration) {
            let declaration = &utility.declaration[range];
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
                    format!("{property}:{}", space_var_fallbacks(value))
                } else {
                    declaration.to_owned()
                };
                // Keep fallback `var()` comma spacing generic across all
                // declaration values.
                let declaration = space_var_fallbacks(&declaration);
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

fn space_var_fallbacks(value: &str) -> String {
    let mut output = value.to_owned();
    let mut positions = Vec::new();
    for found in find_css_functions(value, &["var"], 0) {
        let start = found.start;
        if start > 0 && value.as_bytes()[start - 1] == b',' {
            positions.push(start);
        }
    }
    for start in positions.into_iter().rev() {
        output.insert(start, ' ');
    }
    output
}

/// Reorders the utilities that carry a built-in position, leaving every other element in place.
///
/// Only the slots already occupied by built-ins are permuted: a user definition or an arbitrary
/// property keeps its source position, which is what keeps a user's own `@utility` from moving
/// when the registry changes.
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
                variant_applications: Vec::new(),
                declaration: "margin-left:0.5rem;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
                builtin_order: Some(2),
                candidate: "ml-2".to_owned(),
            },
            CompiledUtility {
                variants: Vec::new(),
                variant_applications: Vec::new(),
                declaration: "margin-inline:0.75rem;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
                builtin_order: Some(1),
                candidate: "mx-3".to_owned(),
            },
            CompiledUtility {
                variants: Vec::new(),
                variant_applications: Vec::new(),
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
                variant_applications: Vec::new(),
                declaration: "all:unset;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
                builtin_order: Some(usize::MAX),
                candidate: "[all:unset]".to_owned(),
            },
            CompiledUtility {
                variants: Vec::new(),
                variant_applications: Vec::new(),
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
                variant_applications: Vec::new(),
                declaration: "color:red;display:block;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
                builtin_order: None,
                candidate: String::new(),
            },
            CompiledUtility {
                variants: Vec::new(),
                variant_applications: Vec::new(),
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
