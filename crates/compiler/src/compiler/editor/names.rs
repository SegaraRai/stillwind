//! Reserves declaration names before any source occurrence is rendered.

use stillwind_ir::ModuleAnalysis;

use super::super::symbols::{named_declaration, ref_context};
use crate::identifiers::{
    AnchorNameRequest, ContainerNameRequest, DeclarationContext, KeyframesNameRequest,
    PreviewNames, RefMarkerName, ViewTransitionClassRequest,
};

/// Reserves display names for refs, keyframes, and transitions in `module` and its
/// supporting analyses. Call this for the whole batch in module identity order before
/// rendering any sites, so uses share the declaration's label or original binding.
/// Existing identities keep their names; colliding labels receive distinct suffixes.
/// This updates only `names` and does not call host generators or publish artifacts.
pub(crate) fn reserve_preview_names(module: &ModuleAnalysis, names: &mut PreviewNames) {
    for definition in &module.ir.ref_definitions {
        let context = ref_context(definition, None);
        names.name(&RefMarkerName(DeclarationContext {
            module: context.module.clone(),
            start: context.start,
            end: context.end,
            binding: context.binding.clone(),
            label: context.label.clone(),
        }));
        names.name(&AnchorNameRequest(context.clone()));
        names.name(&ContainerNameRequest(context));
    }
    for definition in &module.ir.keyframes_declarations {
        names.name(&KeyframesNameRequest(named_declaration(
            &module.module_id,
            &definition.binding,
            definition.label.as_ref(),
        )));
    }
    for definition in &module.ir.view_transition_declarations {
        names.name(&ViewTransitionClassRequest(named_declaration(
            &module.module_id,
            &definition.binding,
            definition.label.as_ref(),
        )));
    }
    for support in &module.asset_support {
        reserve_preview_names(support, names);
    }
}
