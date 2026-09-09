use std::collections::BTreeSet;

use stillwind_ir::HandleProjection;
use stillwind_semantic::{AssetKind, Candidates, ExportedValue, IdentityId};
use stillwind_source::{SourceFile, Spanned};

use super::symbols::{asset_identity, ref_symbol_name};

pub(super) fn render_handle_projection(
    handle: &stillwind_ir::HandleDefinition,
    projection: &HandleProjection,
) -> String {
    let name = ref_symbol_name(handle);
    match projection {
        HandleProjection::Anchor => format!("anchor-name:{name};"),
        HandleProjection::Position => format!("position-anchor:{name};"),
        HandleProjection::Scope => format!("anchor-scope:{name};"),
        HandleProjection::Container(kind) => {
            format!("container-name:{name};container-type:{};", kind.value)
        }
    }
}

pub(super) fn handle_marker_class(handle: &stillwind_ir::HandleDefinition) -> String {
    let hash = &blake3::hash(asset_identity(&handle.module_path, "ref", &handle.name).as_bytes())
        .to_hex()[..8];
    match &handle.label {
        Some(label) => format!("s_{}_{}", label.value, hash),
        None => format!("s_{hash}"),
    }
}

pub(super) fn handle_parent_selector(
    handle: &stillwind_ir::HandleDefinition,
    selector: &str,
) -> String {
    let marker = handle_marker_class(handle);
    let reference = match selector.strip_prefix('&') {
        Some("") => format!(".{marker}"),
        Some(suffix) => format!(".{marker}:where({suffix})"),
        None => unreachable!("the extractor requires ref selectors to begin with `&`"),
    };
    match &handle.scope {
        Some(scope) => format!("{}:has({reference}) &", scope_marker_class(scope)),
        None => format!("{reference} &"),
    }
}

pub(super) fn scope_marker_class(scope: &stillwind_ir::ScopeDefinition) -> String {
    let hash = &blake3::hash(asset_identity(&scope.module_path, "scope", &scope.name).as_bytes())
        .to_hex()[..8];
    match &scope.label {
        Some(label) => format!(".s_{}_{}", label.value, hash),
        None => format!(".s_{hash}"),
    }
}

pub(super) fn ref_identity(handle: &stillwind_ir::HandleDefinition) -> IdentityId {
    IdentityId::new(
        AssetKind::Ref,
        &handle.module_path,
        handle.name.span.start.0,
        handle.name.span.end.0,
        handle.name.value.clone(),
    )
}

pub(super) fn scope_identity(scope: &stillwind_ir::ScopeDefinition) -> IdentityId {
    IdentityId::new(
        AssetKind::Scope,
        &scope.module_path,
        scope.name.span.start.0,
        scope.name.span.end.0,
        scope.name.value.clone(),
    )
}

pub(super) fn named_identity(
    source: &SourceFile,
    kind: AssetKind,
    name: &Spanned<String>,
) -> IdentityId {
    IdentityId::new(
        kind,
        source.path(),
        name.span.start.0,
        name.span.end.0,
        name.value.clone(),
    )
}

pub(super) fn summary_runtime_assets(
    summary: &stillwind_semantic::ModuleSummary,
) -> BTreeSet<IdentityId> {
    let mut identities = BTreeSet::new();
    for value in summary.exports.values() {
        match value {
            ExportedValue::Refs(alternatives) => {
                for alternative in alternatives {
                    identities.insert(alternative.reference.clone());
                    if let Some(scopes) = &alternative.permitted_scopes
                        && let Candidates::Finite(scopes) = &scopes.candidates
                    {
                        identities.extend(scopes.iter().cloned());
                    }
                }
            }
            ExportedValue::Scopes(value)
            | ExportedValue::Animations(value)
            | ExportedValue::ViewTransitions(value) => {
                if let Candidates::Finite(candidates) = &value.candidates {
                    identities.extend(candidates.iter().cloned());
                }
            }
        }
    }
    identities
}
