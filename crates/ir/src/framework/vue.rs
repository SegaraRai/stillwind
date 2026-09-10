use super::{AdapterKind, FrameworkSpec};

pub const SPEC: FrameworkSpec = FrameworkSpec {
    id: AdapterKind::Vue,
    macro_import: "stillwind/vue",
    runtime_import: "stillwind/vue/runtime",
    class_property: "class",
    projected_input: false,
    normalize,
};

fn normalize(expression: super::ProjectionExpression<'_>) -> super::ProjectionDecision {
    match expression {
        super::ProjectionExpression::Member {
            receiver: "__VLS_ctx",
            property,
            optional: false,
            ..
        } => super::ProjectionDecision::Retain(property),
        super::ProjectionExpression::ExpandedShorthand {
            receiver: "__VLS_ctx",
            key,
            key_name,
            value_name,
            ..
        } if key_name == value_name => super::ProjectionDecision::Retain(key),
        _ => super::ProjectionDecision::Preserve,
    }
}
