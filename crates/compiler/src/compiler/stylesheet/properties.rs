use std::collections::BTreeSet;
use std::fmt::Write;

use stillwind_css::{PropertyUsageError, custom_property_usage};
use stillwind_design_system::{ThemeIndex, builtin_property_definitions};

use crate::CompilerOptions;

#[allow(clippy::too_many_lines)]
pub(in crate::compiler) fn render_used_properties(
    theme: &ThemeIndex,
    css: &str,
    options: &CompilerOptions,
) -> Result<String, PropertyUsageError> {
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
    if !builtin_initials.is_empty() {
        output.push_str("@layer properties{@supports (((-webkit-hyphens:none)) and (not (margin-trim:inline))) or ((-moz-orient:inline) and (not (color:rgb(from red r g b)))){*,:before,:after,::backdrop{");
        output.push_str(&builtin_initials.concat());
        output.push_str("}}}\n");
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
    Ok(output)
}
