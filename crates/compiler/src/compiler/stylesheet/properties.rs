//! Selection and serialization of `@property` registrations.
//!
//! Registration is driven by usage: a definition is emitted when the CSS declares or
//! references its name, or when one of its activating properties appears. Activation
//! pulls in each definition's dependencies and group members up to a fixed point. Only
//! registered definitions can be emitted; a missing dependency name has no definition
//! to emit. An authored `@property` rule wins over a built-in of the same name, and
//! built-in definitions are emitted through the compatibility wrapper the built-in
//! stylesheet uses, so a browser that lacks the at-rule still gets fallback values.

use std::collections::BTreeSet;
use std::fmt::Write;

use stillwind_css::{PropertyUsageError, custom_property_usage};
use stillwind_design_system::{ThemeIndex, builtin_property_definitions};

use crate::CompilerOptions;

/// Selected registrations and their fallback rule, before placement in the template.
#[derive(Default)]
pub(super) struct PropertyOutput {
    pub registrations: String,
    pub initials: String,
}

/// Renders the `@property` rules `css` needs, or an empty string when none are needed.
///
/// Duplicates are resolved while walking the registry backwards, so a theme definition
/// wins over a built-in of the same name, and the list is reversed once more before
/// emission, so the output keeps the registry's own order.
///
/// # Errors
///
/// Returns [`PropertyUsageError`] when CSS usage cannot be parsed. Values are not
/// checked against the registered `@property` syntax descriptor.
#[allow(clippy::too_many_lines)]
pub(super) fn render_used_properties(
    theme: &ThemeIndex,
    css: &str,
    options: &CompilerOptions,
    explicit: &[String],
) -> Result<PropertyOutput, PropertyUsageError> {
    let usage = custom_property_usage(css)?;
    let builtin_properties = builtin_property_definitions(theme);
    let definitions = builtin_properties.iter().chain(&theme.properties);
    let mut required = definitions
        .clone()
        .filter_map(|property| {
            let name = options.rewrite_generated(&property.name);
            (property.activation.is_active(
                usage.declared.contains(&name),
                usage.referenced.contains(&name),
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
            if required.contains(&name) {
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
            if !required.contains(&name)
                || theme.authored_properties.contains(&name)
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
    let mut output = String::new();
    let mut builtin_initials = Vec::new();
    for (name, body, builtin) in &properties {
        if *builtin {
            let initial = body
                .lines()
                .find_map(|line| line.trim().strip_prefix("initial-value:"))
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("initial");
            builtin_initials.push(format!("{name}:{initial};"));
        }
    }
    let initials = if builtin_initials.is_empty() {
        String::new()
    } else {
        format!(
            "*,:before,:after,::backdrop{{{}}}",
            builtin_initials.concat()
        )
    };
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
    Ok(PropertyOutput {
        registrations: output,
        initials,
    })
}
