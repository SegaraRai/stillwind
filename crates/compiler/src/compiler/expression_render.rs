//! JavaScript serialization of lowered class expressions.
//!
//! [`is_plain`] checks structural compatibility without side effects; [`render_plain`]
//! serializes accepted trees while retaining code mappings and registering deferred
//! helpers. Property-bearing groups belong to fragment rendering. These operations
//! do not analyze source-value types or decide whether evaluations can be reordered.

use stillwind_ir::ConditionalKind;

use super::fragment::expression::{Emitted, EmittedEntry, ValueExpr};
use crate::generated::{Code, code, js_string};
use crate::runtime::RuntimeBindings;

/// Serializes a class-value tree without introducing framework property keys.
///
/// Source mappings and branch/array order are retained; missing slots become `null`.
/// Returns `None` for groups with ordinary/custom properties or no retained class
/// entries. Opaque expressions are emitted verbatim, not normalized or type-checked.
/// Deferred helpers visited before a failure may already have been registered, so
/// production callers establish structural compatibility before rendering.
pub(crate) fn render_plain(value: &Emitted, runtime: &mut RuntimeBindings<'_>) -> Option<Code> {
    match value {
        Emitted::Expression(expression) => Some(expression.render(runtime)),
        Emitted::Leaf { names } => Some(js_string(&names.join(" ")).into()),
        Emitted::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                parts.push(match item {
                    Some(item) => render_plain(item, runtime)?,
                    None => "null".into(),
                });
            }
            Some(code!["[", Code::join(parts, ","), "]"])
        }
        Emitted::Conditional {
            code: test,
            kind,
            consequent,
            alternate,
            ..
        } => {
            let consequent = match consequent {
                Some(value) => render_plain(value, runtime)?,
                None => "null".into(),
            };
            match kind {
                ConditionalKind::LogicalAnd => Some(code!["(", test, ") && (", consequent, ")"]),
                ConditionalKind::Ternary => {
                    let alternate = match alternate {
                        Some(value) => render_plain(value, runtime)?,
                        None => "null".into(),
                    };
                    Some(code![
                        "(", test, ") ? (", consequent, ") : (", alternate, ")"
                    ])
                }
            }
        }
        Emitted::Group(entries) => {
            let mut parts = Vec::new();
            for entry in entries {
                match entry {
                    // Props and inline styles cannot live inside one class property.
                    Some(EmittedEntry::Attribute(_) | EmittedEntry::CustomProperty(_)) => {
                        return None;
                    }
                    Some(EmittedEntry::Class(value)) => {
                        parts.push(render_plain(value, runtime)?);
                    }
                    None => {}
                }
            }
            match parts.len() {
                0 => None,
                1 => parts.pop(),
                _ => Some(code!["[", Code::join(parts, ","), "]"]),
            }
        }
    }
}

/// Checks whether a tree can remain a class-only atom without registering helpers.
/// Rejects opaque source values and empty or property-bearing groups. Deferred calls
/// and class-only arrays/branches qualify; this does not imply purity or nonemptiness.
pub(crate) fn is_plain(value: &Emitted) -> bool {
    match value {
        Emitted::Expression(ValueExpr::Source(_)) => false,
        Emitted::Expression(_) | Emitted::Leaf { .. } => true,
        Emitted::Array(items) => items.iter().flatten().all(is_plain),
        Emitted::Conditional {
            consequent,
            alternate,
            ..
        } => consequent
            .iter()
            .chain(alternate)
            .all(|value| is_plain(value)),
        Emitted::Group(entries) => {
            entries
                .iter()
                .flatten()
                .all(|entry| matches!(entry, EmittedEntry::Class(value) if is_plain(value)))
                && entries.iter().any(Option::is_some)
        }
    }
}
