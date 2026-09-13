//! JavaScript serialization of lowered class expressions.
//!
//! [`render_plain`] proves a class expression fits one class property; [`render_fragment`]
//! retains attributes and runtime values for the merge helper. Both preserve tree order.

use stillwind_ir::ConditionalKind;
use stillwind_source::Span;

use super::props::{Emitted, EmittedEntry};
use crate::runtime::{RuntimeBindings, RuntimeHelper};

/// Renders a plain class value: strings that clsx can join directly.
///
/// Returns `None` when the value cannot be expressed without the fragment runtime, which is what
/// makes Tier 1 a proof obligation rather than a preference. An attribute can never appear inside
/// one class property, so any attribute entry makes the whole value unrenderable this way.
pub(super) fn render_plain(value: &Emitted) -> Option<String> {
    match value {
        Emitted::Expression(text) => Some(text.clone()),
        Emitted::Leaf { names } => Some(format!("\"{}\"", names.join(" "))),
        Emitted::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                parts.push(match item {
                    Some(item) => render_plain(item)?,
                    None => "null".to_owned(),
                });
            }
            Some(format!("[{}]", parts.join(",")))
        }
        Emitted::Conditional {
            test,
            kind,
            consequent,
            alternate,
        } => {
            let consequent = match consequent {
                Some(value) => render_plain(value)?,
                None => "null".to_owned(),
            };
            match kind {
                ConditionalKind::LogicalAnd => Some(format!("({test}) && ({consequent})")),
                ConditionalKind::Ternary => {
                    let alternate = match alternate {
                        Some(value) => render_plain(value)?,
                        None => "null".to_owned(),
                    };
                    Some(format!("({test}) ? ({consequent}) : ({alternate})"))
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
                        parts.push(render_plain(value)?);
                    }
                    None => {}
                }
            }
            match parts.len() {
                0 => None,
                1 => parts.pop(),
                _ => Some(format!("[{}]", parts.join(","))),
            }
        }
    }
}

/// Renders a fragment expression that `stillwindMerge` can consume.
///
/// The output is a fresh expression, not a reference to anything already emitted, so the same
/// tree can be rendered once per tier decision without sharing state.
pub(super) fn render_fragment(
    value: &Emitted,
    runtime: &mut RuntimeBindings<'_>,
    span: Span,
) -> String {
    match value {
        Emitted::Expression(text) => {
            let key = runtime.require(RuntimeHelper::ClassKey, span);
            format!("{{[{key}]:({text})}}")
        }
        Emitted::Leaf { names } => fragment(runtime, span, &[names.join(" ")]),
        Emitted::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                parts.push(match item {
                    Some(item) => render_fragment(item, runtime, span),
                    None => "null".to_owned(),
                });
            }
            format!("[{}]", parts.join(","))
        }
        Emitted::Conditional {
            test,
            kind,
            consequent,
            alternate,
        } => {
            let consequent = match consequent {
                Some(value) => render_fragment(value, runtime, span),
                None => "null".to_owned(),
            };
            match kind {
                ConditionalKind::LogicalAnd => format!("({test}) && ({consequent})"),
                ConditionalKind::Ternary => {
                    let alternate = match alternate {
                        Some(value) => render_fragment(value, runtime, span),
                        None => "null".to_owned(),
                    };
                    format!("({test}) ? ({consequent}) : ({alternate})")
                }
            }
        }
        Emitted::Group(entries) => {
            let mut parts = Vec::new();
            for entry in entries {
                match entry {
                    Some(EmittedEntry::CustomProperty(text)) => {
                        let key = runtime.require(RuntimeHelper::CustomPropertiesKey, span);
                        parts.push(format!("{{[{key}]:{{{text}}}}}"));
                    }
                    Some(EmittedEntry::Attribute(text)) => parts.push(format!("{{{text}}}")),
                    Some(EmittedEntry::Class(value)) => {
                        parts.push(render_fragment(value, runtime, span));
                    }
                    None => parts.push("null".to_owned()),
                }
            }
            if parts.len() == 1 {
                return parts.remove(0);
            }
            format!("[{}]", parts.join(","))
        }
    }
}

/// Builds a fragment object for one already-joined class string.
fn fragment(runtime: &mut RuntimeBindings<'_>, span: Span, values: &[String]) -> String {
    let key = runtime.require(RuntimeHelper::ClassKey, span);
    format!("{{[{key}]:\"{}\"}}", values.join(" "))
}
