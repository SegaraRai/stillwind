//! Svelte adapter policy and bindings for helpers supplied by svelte2tsx.

use super::{AdapterKind, FrameworkSpec, OutputFormat, ProjectionDecision, ProjectionExpression};

/// Svelte receives custom properties as a serialized style string.
pub(super) const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Svelte,
    macro_import: "stillwind/svelte",
    output: OutputFormat::ClassString,
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
