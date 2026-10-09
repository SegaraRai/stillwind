//! Svelte adapter policy and bindings for helpers supplied by svelte2tsx.

use super::{AdapterKind, FrameworkSpec, ProjectionDecision, ProjectionExpression, StyleFormat};

/// Svelte receives custom properties as a serialized style string.
pub(super) const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Svelte,
    macro_import: "stillwind/svelte",
    runtime_import: "stillwind/runtime/svelte",
    runtime_import_dev: "stillwind/runtime/svelte/development",
    class_property: "class",
    style_property: "style",
    style_format: StyleFormat::String,
    projected_input: true,
    normalize,
};

fn normalize(expression: ProjectionExpression<'_>) -> ProjectionDecision {
    match expression {
        ProjectionExpression::Reference { name }
            if name.starts_with("__sveltets_2_") || name.starts_with("__sveltets_$$") =>
        {
            ProjectionDecision::BindGlobalHelper
        }
        _ => ProjectionDecision::Preserve,
    }
}
