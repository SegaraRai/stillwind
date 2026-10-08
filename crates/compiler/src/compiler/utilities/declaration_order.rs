//! Declaration-level ordering inside one rule.
//!
//! Utilities are ordered by the key the snapshot's utility ordering mode produced, which
//! makes shorthand and longhand precedence reproducible. Equal keys fall back to a
//! candidate-name comparison and branches of one candidate keep their resolved order, so
//! output is stable for a given input. Combining, rather than concatenating, the
//! declarations is also where semantic declaration composition applies, so a caller must
//! always go through `combine_declarations_for_theme` instead of joining declaration
//! strings itself.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use cssparser::{ParseError, Parser, Token};

use stillwind_css::declaration_ranges;
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

/// Combines the declarations of several utilities into one block body.
//
// A theme that carries its own declaration formats overrides the built-in set entirely, which is
// how a project can redefine how a property composes without patching the registry.
pub(in crate::compiler) fn combine_declarations_for_theme(
    utilities: &[&CompiledUtility],
    theme: &ThemeIndex,
) -> String {
    let formats = if theme.declaration_formats().is_empty() {
        builtin_declaration_formats(theme)
    } else {
        theme.declaration_formats()
    };
    combine_declarations_with_formats(utilities, formats)
}

/// The combination itself, with declaration formats supplied by the caller.
///
/// `compose-space` appends a later value at the first occurrence of its property.
/// Other declarations retain their resolved values without output optimization.
pub(in crate::compiler) fn combine_declarations_with_formats(
    utilities: &[&CompiledUtility],
    formats: &BTreeMap<String, String>,
) -> String {
    enum Declaration<'a> {
        Raw(&'a str),
        Composed {
            property: &'a str,
            values: Vec<&'a str>,
            important: bool,
        },
    }
    let mut composed = HashMap::<&str, usize>::new();
    let mut ordered = Vec::<Declaration<'_>>::new();
    for utility in order_utilities(utilities.to_vec()) {
        for range in declaration_ranges(&utility.declaration) {
            let declaration = &utility.declaration[range];
            if let Some((property, value)) = declaration.split_once(':')
                && declaration_format(formats, property.trim()).is_some_and(|format| {
                    format
                        .split_whitespace()
                        .any(|item| item == "compose-space")
                })
            {
                let (value, important) = composition_value(value);
                if let Some(index) = composed.get(property.trim()).copied() {
                    if let Declaration::Composed {
                        values,
                        important: priority,
                        ..
                    } = &mut ordered[index]
                    {
                        values.push(value);
                        *priority |= important;
                    }
                } else {
                    composed.insert(property.trim(), ordered.len());
                    ordered.push(Declaration::Composed {
                        property,
                        values: vec![value],
                        important,
                    });
                }
            } else if !declaration.trim().is_empty() {
                ordered.push(Declaration::Raw(declaration));
            }
        }
    }
    let mut output = String::new();
    for declaration in ordered {
        match declaration {
            Declaration::Raw(declaration) => output.push_str(declaration),
            Declaration::Composed {
                property,
                values,
                important,
            } => {
                output.push_str(property);
                output.push(':');
                output.push_str(&values.join(" "));
                if important {
                    output.push_str("!important");
                }
            }
        }
        output.push(';');
    }
    output
}

/// Separates priority before composing values, keeping it on the final declaration.
fn composition_value(value: &str) -> (&str, bool) {
    let mut parser = Parser::new(value);
    let mut bang = None;
    let mut priority = None;
    loop {
        let position = parser.position().byte_index();
        let Ok(token) = parser.next() else {
            break;
        };
        priority = if matches!(token, Token::Ident(name) if name.eq_ignore_ascii_case("important"))
        {
            bang
        } else {
            None
        };
        bang = matches!(token, Token::Delim('!')).then_some(position);
        if matches!(
            token,
            Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock
        ) {
            let _: Result<(), ParseError<()>> = parser.parse_nested_block(|inner| {
                while inner.next_including_whitespace_and_comments().is_ok() {}
                Ok(())
            });
        }
    }
    priority.map_or((value.trim(), false), |start| (value[..start].trim(), true))
}

/// Orders utilities by shared precedence while retaining branches of the same candidate.
pub(in crate::compiler) fn order_utilities(
    mut utilities: Vec<&CompiledUtility>,
) -> Vec<&CompiledUtility> {
    utilities.sort_by(|left, right| {
        left.order
            .compare(&left.candidate, &right.order, &right.candidate)
    });
    utilities
}

/// Test-only combination against the built-in fixture registry.
#[cfg(test)]
fn combine_declarations(utilities: &[&CompiledUtility]) -> String {
    use stillwind_design_system::parse_declaration_formats;
    use stillwind_test_support::defaults::BUILTIN_REGISTRY_CSS;

    combine_declarations_with_formats(utilities, &parse_declaration_formats(BUILTIN_REGISTRY_CSS))
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::UtilityOrder;

    use super::super::resolve::UtilitySource;
    use super::*;

    #[test]
    fn orders_conflicting_shorthands_like_tailwind() {
        let declarations = [
            CompiledUtility {
                source: UtilitySource::default(),
                required_properties: Vec::new(),
                variants: Vec::new(),
                variant_applications: Vec::new(),
                declaration: "margin-left:0.5rem;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
                order: UtilityOrder::Authored(Some(2)),
                candidate: "ml-2".to_owned(),
            },
            CompiledUtility {
                source: UtilitySource::default(),
                required_properties: Vec::new(),
                variants: Vec::new(),
                variant_applications: Vec::new(),
                declaration: "margin-inline:0.75rem;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
                order: UtilityOrder::Authored(Some(1)),
                candidate: "mx-3".to_owned(),
            },
            CompiledUtility {
                source: UtilitySource::default(),
                required_properties: Vec::new(),
                variants: Vec::new(),
                variant_applications: Vec::new(),
                declaration: "margin:1rem;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
                order: UtilityOrder::Authored(Some(0)),
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
                source: UtilitySource::default(),
                required_properties: Vec::new(),
                variants: Vec::new(),
                variant_applications: Vec::new(),
                declaration: "all:unset;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
                order: UtilityOrder::Authored(Some(usize::MAX)),
                candidate: "[all:unset]".to_owned(),
            },
            CompiledUtility {
                source: UtilitySource::default(),
                required_properties: Vec::new(),
                variants: Vec::new(),
                variant_applications: Vec::new(),
                declaration: "--tw-skew-x:skewX(6deg);transform:var(--tw-skew-x,);".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
                order: UtilityOrder::Authored(Some(1)),
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
                source: UtilitySource::default(),
                required_properties: Vec::new(),
                variants: Vec::new(),
                variant_applications: Vec::new(),
                declaration: "color:red;display:block;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
                order: UtilityOrder::Authored(None),
                candidate: String::new(),
            },
            CompiledUtility {
                source: UtilitySource::default(),
                required_properties: Vec::new(),
                variants: Vec::new(),
                variant_applications: Vec::new(),
                declaration: "opacity:0.5;".to_owned(),
                selector: None,
                container: None,
                nested: Vec::new(),
                order: UtilityOrder::Authored(None),
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

    #[test]
    fn composed_values_emit_priority_once_after_all_components() {
        let make = |declaration: &str| CompiledUtility {
            source: UtilitySource::default(),
            required_properties: Vec::new(),
            variants: Vec::new(),
            variant_applications: Vec::new(),
            declaration: declaration.into(),
            selector: None,
            container: None,
            nested: Vec::new(),
            order: UtilityOrder::Authored(None),
            candidate: String::new(),
        };
        let first = make("transform:rotate(1deg)!important;");
        let second = make("transform:scale(2) ! IMPORTANT;");
        assert_eq!(
            combine_declarations(&[&first, &second]),
            "transform:rotate(1deg) scale(2)!important;"
        );
        assert_eq!(
            composition_value("var(--x, '!important')"),
            ("var(--x, '!important')", false)
        );
    }
}
