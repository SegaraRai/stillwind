use super::{AdapterKind, FrameworkSpec, ProjectionDecision, ProjectionExpression, StyleFormat};

pub const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Vue,
    macro_import: "stillwind/vue",
    runtime_import: "stillwind/vue/runtime",
    class_property: "class",
    style_format: StyleFormat::Object,
    projected_input: true,
    normalize,
};

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
