//! Checks of a `var()` value written without a type hint against the utility that
//! receives it.
//!
//! The type of a custom property is known when it is an active theme token of a
//! namespace with declared value types, or when an authored `@property` rule registers
//! it with a simple syntax. A known type that the receiving property rejects is
//! recorded as a [`VariableMismatch`]. Branch selection never depends on the type, so
//! the utility keeps Tailwind's choice and only the report changes.

use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{CssProperty, ValueType};

use super::super::values::theme_arbitrary_types;
use super::resolve::{UtilityArbitraryUse, UtilityTokenUse};
use crate::{ThemeIndex, ThemeNamespace};

/// How the type of a custom property is known.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariableTypeSource {
    /// The property is an active theme token of this namespace, such as `text` for
    /// `--text-sm`.
    Theme(LeanStr),
    /// An authored `@property` rule registers the property.
    Registration,
}

/// A `var()` value without a type hint whose known type the receiving property rejects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariableMismatch {
    /// Custom-property name, including its leading `--`.
    pub variable: LeanStr,
    /// The type as declared: the namespace's type hints joined with ` or `, such as
    /// `font-size`, or the `@property` syntax, such as `<length>`.
    pub value_type: LeanStr,
    /// How the type is known.
    pub source: VariableTypeSource,
    /// Type hints of the utility that accept a value of the variable's type, in the
    /// order of the utility's type hints.
    pub type_hints: Vec<LeanStr>,
}

/// The value types that a custom property is known to hold.
struct VariableType {
    /// See [`VariableMismatch::value_type`].
    label: LeanStr,
    source: VariableTypeSource,
    /// Representative values of the type; a value of the variable matches one of them.
    samples: Vec<&'static str>,
}

/// Returns the mismatch of the variable `name` with the base declaration that receives
/// it, or `None` when its type is unknown or the declaration's property accepts it.
///
/// `uses` are the token selections of the resolution, and `hints` are the type hints
/// that the utility accepts. The returned capture names the base property that selects
/// the variable. A custom property or a property without a known grammar accepts every
/// type.
pub(super) fn variable_mismatch(
    name: &str,
    uses: &[UtilityTokenUse],
    hints: &[LeanStr],
    theme: &ThemeIndex,
) -> Option<(UtilityArbitraryUse, VariableMismatch)> {
    let variable = variable_type(name, theme)?;
    let usage = uses
        .iter()
        .find(|usage| usage.base && usage.tokens.iter().any(|token| token == name))?;
    let property = CssProperty::from_name(&usage.property)?;
    if variable
        .samples
        .iter()
        .any(|sample| property.accepts(sample))
    {
        return None;
    }
    let type_hints = hints
        .iter()
        .filter(|hint| {
            ValueType::from_hint(hint).is_some_and(|value_type| {
                variable
                    .samples
                    .iter()
                    .any(|sample| value_type.accepts(sample))
            })
        })
        .cloned()
        .collect();
    Some((
        UtilityArbitraryUse {
            base: true,
            property: usage.property.clone(),
            value: format_args!("var({name})").to_lean_str(),
            color: false,
        },
        VariableMismatch {
            variable: name.to_lean_str(),
            value_type: variable.label,
            source: variable.source,
            type_hints,
        },
    ))
}

/// Returns the custom-property name of `value` when it is exactly one `var()` reference
/// without a fallback.
pub(super) fn single_variable(value: &str) -> Option<&str> {
    let name = value.strip_prefix("var(")?.strip_suffix(')')?.trim();
    (name.starts_with("--")
        && !name.contains([',', '(', ')'])
        && !name.contains(char::is_whitespace))
    .then_some(name)
}

/// The known type of the custom property `name`. An `@property` registration takes
/// precedence over a theme namespace.
fn variable_type(name: &str, theme: &ThemeIndex) -> Option<VariableType> {
    if let Some(syntax) = theme.authored_property_syntax(name) {
        let mut samples = Vec::new();
        for component in syntax.split('|') {
            let hint = component.trim().strip_prefix('<')?.strip_suffix('>')?;
            samples.extend(type_samples(ValueType::from_hint(hint)?)?);
        }
        return Some(VariableType {
            label: syntax.to_lean_str(),
            source: VariableTypeSource::Registration,
            samples,
        });
    }
    let namespace = ThemeNamespace::of_token(name)?;
    theme.custom_property(name)?;
    let types = theme_arbitrary_types(namespace, theme);
    let mut samples = Vec::new();
    for &value_type in types {
        samples.extend(type_samples(value_type)?);
    }
    (!samples.is_empty()).then(|| VariableType {
        label: types
            .iter()
            .map(|value_type| value_type.hint())
            .collect::<Vec<_>>()
            .join(" or ")
            .to_lean_str(),
        source: VariableTypeSource::Theme(namespace.css_name().into()),
        samples,
    })
}

/// Representative values of `value_type`, or `None` when the type has no small set of
/// values that identify it.
fn type_samples(value_type: ValueType) -> Option<&'static [&'static str]> {
    Some(match value_type {
        ValueType::Integer | ValueType::ZIndex => &["1"],
        ValueType::Number => &["1.5"],
        ValueType::Percentage | ValueType::FontStretch => &["50%"],
        ValueType::NumberPercentage => &["1.5", "50%"],
        ValueType::Length | ValueType::LineWidth | ValueType::FontSize => &["1rem"],
        ValueType::LengthPercentage => &["1rem", "50%"],
        ValueType::Angle => &["1deg"],
        ValueType::AnglePercentage => &["1deg", "50%"],
        ValueType::Time => &["1s"],
        ValueType::Ratio => &["16 / 9"],
        ValueType::Easing => &["cubic-bezier(0, 0, 1, 1)"],
        ValueType::Color => &["rgb(0 0 0)"],
        ValueType::Image => &["linear-gradient(red, blue)"],
        ValueType::Url => &["url(a.png)"],
        ValueType::Shadow => &["0 0 1px rgb(0 0 0)"],
        ValueType::FontFamily => &["serif"],
        ValueType::FontWeight => &["700"],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use stillwind_source::{FileId, SourceFile};

    use super::*;
    use crate::evaluation::utilities::{InvalidValueReason, resolve_utility};
    use crate::test_support::parse_fixture_theme;

    fn theme() -> ThemeIndex {
        parse_fixture_theme(
            &SourceFile::new(
                FileId(1),
                None,
                None,
                "@property --my-len { syntax: \"<length>\"; inherits: false; initial-value: 0px; } @property --any { syntax: \"*\"; inherits: false; }",
            )
            .unwrap(),
        )
        .unwrap()
    }

    /// Variables of a known type report the property that rejects them, while the
    /// declaration keeps the definition that Tailwind selects.
    #[test]
    fn variables_of_known_types_report_properties_that_reject_them() {
        let theme = theme();
        for (raw, declaration, value_type, source, hints) in [
            (
                "bg-(--text-sm)",
                "background-color:var(--text-sm);",
                "font-size",
                VariableTypeSource::Theme("text".into()),
                &["bg-size", "length", "position"][..],
            ),
            (
                "w-(--color-red-500)",
                "width:var(--color-red-500);",
                "color",
                VariableTypeSource::Theme("color".into()),
                &[],
            ),
            (
                "bg-(--my-len)",
                "background-color:var(--my-len);",
                "<length>",
                VariableTypeSource::Registration,
                &["bg-size", "length", "position"],
            ),
            (
                "text-[var(--text-sm)]",
                "color:var(--text-sm);",
                "font-size",
                VariableTypeSource::Theme("text".into()),
                &["length"],
            ),
        ] {
            let resolved = resolve_utility(raw, &theme).unwrap().unwrap();
            assert_eq!(resolved.declaration, declaration, "{raw}");
            let invalid = resolved.diagnostic_data.invalid_value.unwrap();
            let InvalidValueReason::VariableType(mismatch) = invalid.reason else {
                panic!("{raw}: {:?}", invalid.reason);
            };
            assert_eq!(mismatch.value_type, value_type, "{raw}");
            assert_eq!(mismatch.source, source, "{raw}");
            assert_eq!(mismatch.type_hints, hints, "{raw}");
            assert!(raw[invalid.range].contains(&*mismatch.variable), "{raw}");
        }
        for raw in [
            "bg-(--x)",
            "bg-(--any)",
            "bg-(--color-red-500)",
            "text-(--text-sm--line-height)",
            "w-(--spacing)",
            "bg-(length:--text-sm)",
            "shadow-(--text-sm)",
        ] {
            let resolved = resolve_utility(raw, &theme).unwrap().unwrap();
            assert_eq!(resolved.diagnostic_data.invalid_value, None, "{raw}");
        }
    }

    #[test]
    fn untyped_variables_record_the_hint_of_the_selected_definition() {
        let theme = theme();
        for (raw, value, hint) in [
            ("text-(--x)", "(--x)", "color"),
            ("bg-(--x)", "(--x)", "color"),
            ("border-(--x)", "(--x)", "color"),
            ("shadow-(--x)", "(--x)", "shadow"),
            ("text-[var(--x)]", "[var(--x)]", "color"),
            ("md:text-(--x)/50", "(--x)", "color"),
        ] {
            let resolved = resolve_utility(raw, &theme).unwrap().unwrap();
            let untyped = resolved.diagnostic_data.untyped_variable.expect(raw);
            assert_eq!((&raw[untyped.range], untyped.hint.as_str()), (value, hint));
        }
        for raw in [
            "w-(--x)",
            "text-(color:--x)",
            "text-[var(--x),red]",
            "bg-red-500",
        ] {
            let resolved = resolve_utility(raw, &theme).unwrap().unwrap();
            assert_eq!(resolved.diagnostic_data.untyped_variable, None, "{raw}");
        }
    }

    /// A variable whose known type the receiving property rejects records the mismatch
    /// without the hint of the selected definition, which would state the rejected
    /// type. A variable whose known type matches keeps the hint.
    #[test]
    fn rejected_variable_types_record_no_hint_of_the_selected_definition() {
        let theme = theme();
        for (raw, hint, mismatch) in [
            ("bg-(--text-sm)", None, true),
            ("bg-(--my-len)", None, true),
            ("bg-(--color-red-500)", Some("color"), false),
        ] {
            let resolved = resolve_utility(raw, &theme).unwrap().unwrap();
            assert_eq!(
                resolved
                    .diagnostic_data
                    .untyped_variable
                    .map(|untyped| untyped.hint),
                hint.map(LeanStr::from),
                "{raw}"
            );
            assert_eq!(
                matches!(
                    resolved
                        .diagnostic_data
                        .invalid_value
                        .map(|invalid| invalid.reason),
                    Some(InvalidValueReason::VariableType(_))
                ),
                mismatch,
                "{raw}"
            );
        }
    }

    /// `color-mix()` rejects percentages outside `0%` to `100%`, so both spellings of
    /// such an opacity modifier emit the percentage as written and record it.
    #[test]
    fn opacity_modifiers_outside_the_percentage_range_are_recorded() {
        let theme = theme();
        for (raw, value, modifier) in [
            ("bg-red-500/150", "150%", "150"),
            ("bg-red-500/[150%]", "150%", "[150%]"),
            ("text-red-500/[-5%]", "-5%", "[-5%]"),
        ] {
            let resolved = resolve_utility(raw, &theme).unwrap().unwrap();
            assert!(
                resolved
                    .declaration
                    .contains(&format!(" {value}, transparent)")),
                "{raw}: {}",
                resolved.declaration
            );
            let invalid = resolved.diagnostic_data.invalid_value.unwrap();
            assert_eq!(invalid.reason, InvalidValueReason::OpacityModifier, "{raw}");
            assert_eq!(
                (invalid.value.as_str(), &raw[invalid.range]),
                (value, modifier)
            );
        }
        let resolved = resolve_utility("bg-red-500/50", &theme).unwrap().unwrap();
        assert_eq!(resolved.diagnostic_data.invalid_value, None);
    }

    #[test]
    fn single_variables_have_no_fallback() {
        assert_eq!(single_variable("var(--x)"), Some("--x"));
        assert_eq!(single_variable("var(--x, red)"), None);
        assert_eq!(single_variable("calc(var(--x))"), None);
    }
}
