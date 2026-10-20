//! JavaScript serialization of lowered class expressions.
//!
//! [`is_plain`] checks structural compatibility without side effects; [`render_plain`]
//! serializes accepted trees while retaining code mappings and registering deferred
//! helpers. Property-bearing groups belong to fragment rendering. These operations
//! do not analyze source-value types or decide whether evaluations can be reordered.

use stillwind_ir::ConditionalBranches;
use stillwind_source::JoinText;

use super::fragment::{Emitted, EmittedEntry, ValueExpr};
use crate::generated::{Code, code, js_string};
use crate::runtime::RuntimeBindings;

/// Collects clsx arguments in evaluation order, removing only provably empty values.
/// Compiler-owned arrays and class groups are expanded; guards and opaque expressions
/// remain single arguments so conditional evaluation and helper boundaries survive.
/// Callers must establish `is_plain` before collecting a tree containing groups.
pub(crate) fn collect_class_arguments<'a>(value: &'a Emitted, arguments: &mut Vec<&'a Emitted>) {
    if value.is_empty() {
        return;
    }
    match value {
        Emitted::Array(items) => {
            for item in items.iter().flatten() {
                collect_class_arguments(item, arguments);
            }
        }
        Emitted::Group(entries) => {
            for entry in entries.iter().flatten() {
                if let EmittedEntry::Class(value) = entry {
                    collect_class_arguments(value, arguments);
                }
            }
        }
        _ => {
            arguments.push(value);
        }
    }
}

/// Serializes a class-value tree without introducing framework property keys.
///
/// Source mappings and branch/array order are retained; missing slots become `null`.
/// Returns `None` for groups with custom properties, style payloads, or no retained
/// class entries. Opaque expressions are emitted verbatim, not normalized or
/// type-checked. Deferred helpers visited before a failure may already have been
/// registered, so production callers establish structural compatibility before
/// rendering.
pub(crate) fn render_plain(value: &Emitted, runtime: &mut RuntimeBindings<'_>) -> Option<Code> {
    match value {
        Emitted::Expression(expression) => Some(expression.render(runtime)),
        Emitted::Leaf { names } => Some(js_string(&names.join_text(" ")).into()),
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
            branches,
            ..
        } => {
            let mut render_arm = |value: &Option<Box<Emitted>>| match value {
                Some(value) => render_plain(value, runtime),
                None => Some("null".into()),
            };
            match branches {
                ConditionalBranches::And { consequent } => {
                    Some(code!["(", test, ") && (", render_arm(consequent)?, ")"])
                }
                ConditionalBranches::Ternary {
                    consequent,
                    alternate,
                } => {
                    let consequent = render_arm(consequent)?;
                    let alternate = render_arm(alternate)?;
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
                    Some(EmittedEntry::CustomProperty(_) | EmittedEntry::StylePayload(_)) => {
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
        Emitted::Conditional { branches, .. } => branches
            .consequent()
            .iter()
            .chain(branches.alternate().into_iter().flatten())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argument_collection_preserves_value_guards_and_discards_empty_values() {
        let empty_guard = || Emitted::Conditional {
            code: "test()".into(),
            branches: ConditionalBranches::And { consequent: None },
        };
        let guarded_array = Emitted::Conditional {
            code: "flag".into(),
            branches: ConditionalBranches::Ternary {
                consequent: Some(Box::new(Emitted::Array(vec![Some(Emitted::Leaf {
                    names: vec!["a".into()],
                })]))),
                alternate: None,
            },
        };
        let value = Emitted::Array(vec![
            None,
            Some(Emitted::Array(vec![
                Some(Emitted::Leaf { names: vec![] }),
                Some(empty_guard()),
                Some(Emitted::Group(vec![
                    None,
                    Some(EmittedEntry::Class(empty_guard())),
                ])),
                Some(guarded_array.clone()),
                Some(Emitted::Expression("source()".into())),
            ])),
        ]);
        let mut arguments = Vec::new();
        collect_class_arguments(&value, &mut arguments);
        assert_eq!(
            arguments,
            vec![&guarded_array, &Emitted::Expression("source()".into())]
        );
    }
}
