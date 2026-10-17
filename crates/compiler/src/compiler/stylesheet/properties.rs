//! Selection and serialization of `@property` registrations.
//!
//! [`render_used_properties`] combines CSS usage with explicit registration requests,
//! evaluates definition-specific activation rules, and follows dependency and group
//! relationships to a fixed point. It returns registration rules and fallback
//! declarations as separate [`PropertyOutput`] contributions.
//!
//! Authored registrations suppress generated definitions of the same name. This
//! module does not place selectors or compatibility conditions: the stylesheet
//! template determines where each contribution is emitted.

use std::collections::BTreeSet;
use std::fmt::Write;

use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_css::{CustomPropertyUsage, PropertyInheritance};
use stillwind_design_system::{ThemeIndex, builtin_property_definitions};

use crate::CompilerOptions;

/// Selected registration rules and fallback declarations, before CSS-owned placement.
#[derive(Default)]
pub(super) struct PropertyOutput {
    /// Complete `@property` rules in registry order after duplicate-name selection.
    pub(super) registrations: LeanStr,
    /// Builtin fallback declarations in the same selected order, without a selector.
    pub(super) initials: Vec<PropertyInitial>,
}

impl PropertyOutput {
    /// Concatenates the fallback declarations that `inheritance` selects, in registry
    /// order.
    pub(super) fn initials(&self, inheritance: PropertyInheritance) -> LeanString {
        self.initials
            .iter()
            .filter(|initial| inheritance.includes(initial.inherits))
            .map(|initial| initial.declaration.as_str())
            .collect()
    }
}

/// The fallback declaration of one selected builtin registration.
pub(super) struct PropertyInitial {
    /// `name:value;`, where the value is the registered initial value or `initial`.
    pub(super) declaration: LeanStr,
    /// The registered `inherits` descriptor. A registration without one does not
    /// inherit, as CSS requires the descriptor and Stillwind's builtins state it.
    pub(super) inherits: bool,
}

/// Selects registration rules and builtin fallback declarations required by `usage`.
///
/// `theme` supplies definitions and activation metadata; `options` rewrites generated
/// names. `explicit` supplies additional required names, whose existence the caller
/// validates. Unknown dependency names have no definition to emit. Empty demand returns
/// two empty strings. Input definitions and usage are not modified.
///
/// Duplicates are resolved while walking the registry backwards, so a theme definition
/// wins over a built-in of the same name, and the list is reversed once more before
/// emission, so the output keeps the registry's own order.
///
/// Values are not checked against the registered `@property` syntax descriptor.
#[allow(clippy::too_many_lines)]
pub(super) fn render_used_properties(
    theme: &ThemeIndex,
    usage: &CustomPropertyUsage,
    options: &CompilerOptions,
    explicit: &[LeanStr],
) -> PropertyOutput {
    let builtin_properties = builtin_property_definitions(theme);
    let definitions = builtin_properties.iter().chain(theme.properties());
    let mut required = definitions
        .clone()
        .filter_map(|property| {
            let name = options.rewrite_generated(&property.name);
            (property.activation.is_active(
                usage.declared.contains(name.as_str()),
                usage.referenced.contains(name.as_str()),
            ) || property
                .activation_properties
                .iter()
                .any(|name| usage.declaration_properties.contains(name)))
            .then_some(name)
        })
        .collect::<BTreeSet<_>>();
    // Explicit utility dependencies can require registration even when the same
    // declaration text in another utility deliberately does not (e.g. transform).
    for name in explicit {
        required.insert(options.rewrite_generated(name));
    }
    let mut changed = true;
    while changed {
        changed = false;
        for property in definitions.clone() {
            let name = options.rewrite_generated(&property.name);
            if required.contains(name.as_str()) {
                for dependency in &property.dependencies {
                    changed |= required.insert(options.rewrite_generated(dependency));
                }
                for group in &property.groups {
                    for member in definitions
                        .clone()
                        .filter(|member| member.groups.contains(group))
                    {
                        changed |= required.insert(options.rewrite_generated(&member.name));
                    }
                }
            }
        }
    }
    let mut seen = BTreeSet::new();
    let mut properties = definitions
        .rev()
        .filter_map(|property| {
            let name = options.rewrite_generated(&property.name);
            // Authored @property declarations take precedence over compiler
            // defaults, including when they intentionally reuse a builtin name.
            if !required.contains(name.as_str())
                || theme.authored_properties().contains(name.as_str())
                || !seen.insert(name.clone())
            {
                return None;
            }
            let builtin = builtin_properties
                .iter()
                .any(|candidate| candidate.name == property.name);
            Some((name, property.body.as_str(), builtin))
        })
        .collect::<Vec<_>>();
    properties.reverse();
    let mut output = LeanString::new();
    let mut builtin_initials = Vec::new();
    for (name, body, builtin) in &properties {
        if *builtin {
            let initial = body
                .lines()
                .find_map(|line| line.trim().strip_prefix("initial-value:"))
                .map(|value| value.trim().trim_end_matches(';').trim_end())
                .filter(|value| !value.is_empty())
                .unwrap_or("initial");
            let inherits = body.lines().any(|line| {
                line.trim()
                    .strip_prefix("inherits:")
                    .is_some_and(|value| value.trim().trim_end_matches(';').trim() == "true")
            });
            builtin_initials.push(PropertyInitial {
                declaration: format_args!("{name}:{initial};").to_lean_string().into(),
                inherits,
            });
        }
    }
    for (name, body, _) in properties {
        if !output.is_empty() {
            output.push('\n');
        }
        let _ = writeln!(output, "@property {name} {{");
        for line in body.lines() {
            let _ = writeln!(output, "  {}", line.trim());
        }
        output.push_str("}\n");
    }
    PropertyOutput {
        registrations: (output).into(),
        initials: builtin_initials,
    }
}
