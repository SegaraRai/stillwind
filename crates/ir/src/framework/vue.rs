//! Vue adapter policy and its Volar projection rewrite.
//!
//! Vue sources reach the analyzer through a converter, so the policy has to tell
//! synthetic wrappers apart from user code: `__VLS_*` scaffolding is removed, while a
//! reference that `__VLS_ctx` wraps is mapped back to the user's original expression.

use super::{AdapterKind, FrameworkSpec, ProjectionDecision, ProjectionExpression, StyleFormat};

/// Vue uses `class`; converted inputs are analyzed through the projection path.
pub const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Vue,
    macro_import: "stillwind/vue",
    runtime_import: "stillwind/vue/runtime",
    runtime_import_dev: "stillwind/runtime/dev",
    class_property: "class",
    style_property: "style",
    style_format: StyleFormat::Object,
    projected_input: true,
    normalize,
};

/// Recognizes the wrappers Volar adds to a compiled SFC script.
///
/// Only the exact scaffolding shapes are discarded. Anything else is preserved, because
/// an unrecognized wrapper may be user code that happens to mention `__VLS_ctx`.
fn normalize(expression: ProjectionExpression<'_>) -> ProjectionDecision {
    match expression {
        ProjectionExpression::SyntheticTypeAlias {
            name: "__VLS_SetupExposed",
            definition,
        } if definition.starts_with("import('vue').ShallowUnwrapRef<{") => {
            ProjectionDecision::DiscardScaffolding
        }
        ProjectionExpression::SyntheticReferenceList {
            preceding_line: "// @ts-ignore",
            next_binding: "__VLS_export",
        } => ProjectionDecision::DiscardScaffolding,
        ProjectionExpression::Member {
            receiver: "__VLS_ctx",
            property,
            optional: false,
            ..
        } => ProjectionDecision::Retain(property),
        ProjectionExpression::ExpandedShorthand {
            receiver: "__VLS_ctx",
            key,
            key_name,
            value_name,
            ..
        } if key_name == value_name => ProjectionDecision::Retain(key),
        _ => ProjectionDecision::Preserve,
    }
}
