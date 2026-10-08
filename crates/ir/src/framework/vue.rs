//! Vue adapter policy and its Volar projection rewrite.
//!
//! Vue SFC sources reach the analyzer through a converter, so the policy has to tell
//! synthetic wrappers apart from user code. The frontend validates these candidates
//! against the converter map before retaining expressions or ignoring import reads.
//! Plain Vue JSX uses the same props and runtime without projection normalization.

use super::{AdapterKind, FrameworkSpec, ProjectionDecision, ProjectionExpression, StyleFormat};

/// Vue uses `class` and object styles in both SFC templates and JSX.
pub(super) const SPEC: FrameworkSpec = FrameworkSpec {
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
/// The frontend must validate source correspondence before applying the decision.
fn normalize(expression: ProjectionExpression<'_>) -> ProjectionDecision {
    match expression {
        ProjectionExpression::SyntheticTypeAlias {
            name: "__VLS_SetupExposed" | "__VLS_Bindings",
            definition,
        } if definition.starts_with("import('vue').ShallowUnwrapRef<{")
            || definition.starts_with("__VLS_ProxyRefs<{") =>
        {
            ProjectionDecision::IgnoreReferences
        }
        ProjectionExpression::SyntheticReferenceList {
            preceding_line: "// @ts-ignore",
        }
        | ProjectionExpression::SyntheticReturn {
            binding: "__VLS_self",
            method: "setup",
            mirrored_bindings: true,
        } => ProjectionDecision::IgnoreReferences,
        ProjectionExpression::Member {
            receiver: "__VLS_ctx" | "__VLS_dollars",
            whole,
            property,
            optional: false,
            runtime,
            ..
        } => ProjectionDecision::Keep {
            whole,
            retained: property,
            checker: runtime.then_some(whole),
        },
        ProjectionExpression::Member {
            whole,
            object,
            property_name: "value",
            optional: false,
            runtime,
            ..
        } => ProjectionDecision::Keep {
            whole,
            retained: object,
            checker: runtime.then_some(whole),
        },
        ProjectionExpression::ExpandedShorthand {
            whole,
            value,
            key_name,
            value_name,
            ..
        } if key_name == value_name => ProjectionDecision::Keep {
            whole,
            retained: value,
            checker: Some(value),
        },
        ProjectionExpression::Call {
            callee,
            whole,
            value,
            witness:
                ", {} as import('vue').Ref<unknown>)"
                | ", /** @type {import('vue').Ref<unknown>} */ ({}))",
        } => match callee {
            "__VLS_unwrap" => ProjectionDecision::Keep {
                whole,
                retained: value,
                checker: Some(whole),
            },
            "__VLS_withDotValue" => ProjectionDecision::IgnoreReferences,
            _ => ProjectionDecision::Preserve,
        },
        ProjectionExpression::ConstructorOperand { whole, operand } => ProjectionDecision::Keep {
            whole,
            retained: operand,
            checker: Some(whole),
        },
        _ => ProjectionDecision::Preserve,
    }
}
