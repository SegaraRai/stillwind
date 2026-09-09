use super::utilities::CompiledUtility;

pub(super) fn prepend_missing_declarations(
    declaration: &mut String,
    supporting: &[stillwind_tailwind::VariantDeclaration],
) {
    let declared = declaration
        .split_terminator(';')
        .filter_map(|declaration| declaration.split_once(':').map(|(name, _)| name.trim()))
        .collect::<std::collections::BTreeSet<_>>();
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
pub(super) fn combine_declarations(utilities: &[&CompiledUtility]) -> String {
    combine_declarations_with_formats(utilities, stillwind_tailwind::builtin_declaration_formats())
}

pub(super) fn combine_declarations_for_theme(
    utilities: &[&CompiledUtility],
    theme: &stillwind_css::ThemeIndex,
) -> String {
    let formats = if theme.declaration_formats.is_empty() {
        stillwind_tailwind::builtin_declaration_formats()
    } else {
        &theme.declaration_formats
    };
    combine_declarations_with_formats(utilities, formats)
}

pub(super) fn combine_declarations_with_formats(
    utilities: &[&CompiledUtility],
    formats: &std::collections::BTreeMap<String, String>,
) -> String {
    let mut declarations = String::new();
    let mut composed = std::collections::HashMap::<String, usize>::new();
    let mut ordered = Vec::<String>::new();
    for utility in order_builtin_utilities(utilities.to_vec()) {
        for declaration in utility.declaration.split_terminator(';') {
            let compose_space = declaration
                .split_once(':')
                .and_then(|(property, _)| stillwind_css::declaration_format(formats, property))
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
                    && stillwind_css::declaration_format(formats, property)
                        .is_some_and(|format| format == "empty-space")
                {
                    format!("{property}: ")
                } else if let Some((property, value)) = declaration.split_once(':')
                    && stillwind_css::declaration_format(formats, property).is_some_and(|format| {
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
                let declaration = stillwind_css::fold_calc_expressions(&declaration);
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

pub(super) fn order_builtin_utilities(
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
