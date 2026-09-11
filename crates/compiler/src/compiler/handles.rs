use std::collections::BTreeSet;

use stillwind_ir::{HandleDefinition, HandleProjection, ScopeDefinition};
use stillwind_semantic::{AssetKind, Candidates, ExportedValue, IdentityId, ModuleSummary};
use stillwind_source::{SourceFile, Spanned};

use super::symbols::{asset_identity, ref_symbol_name};

pub(super) fn render_handle_projection(
    handle: &HandleDefinition,
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

pub(super) fn handle_marker_class(handle: &HandleDefinition) -> String {
    let hash = &blake3::hash(asset_identity(&handle.module_path, "ref", &handle.name).as_bytes())
        .to_hex()[..8];
    match &handle.label {
        Some(label) => format!("s_{}_{}", label.value, hash),
        None => format!("s_{hash}"),
    }
}

pub(super) fn scope_marker_class(scope: &ScopeDefinition) -> String {
    let hash = &blake3::hash(asset_identity(&scope.module_path, "scope", &scope.name).as_bytes())
        .to_hex()[..8];
    match &scope.label {
        Some(label) => format!(".s_{}_{}", label.value, hash),
        None => format!(".s_{hash}"),
    }
}

pub(super) fn ref_identity(handle: &HandleDefinition) -> IdentityId {
    IdentityId::new(
        AssetKind::Ref,
        &handle.module_path,
        handle.name.span.start.0,
        handle.name.span.end.0,
        handle.name.value.clone(),
    )
}

pub(super) fn scope_identity(scope: &ScopeDefinition) -> IdentityId {
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

pub(super) fn summary_runtime_assets(summary: &ModuleSummary) -> BTreeSet<IdentityId> {
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

#[cfg(test)]
mod tests {
    use stillwind_ir::{HandleDefinition, ScopeDefinition};
    use stillwind_source::{FileId, Span};

    use super::*;

    #[test]
    fn projections_share_identity_and_scoped_selectors_bind_the_parent() {
        let name = Spanned::new("ref".into(), Span::new(FileId(1), 0, 3));
        let mut handle = HandleDefinition {
            module_path: "x".into(),
            name: name.clone(),
            label: None,
            scope: None,
            call_span: name.span,
        };
        let marker = handle_marker_class(&handle);
        let anchor = render_handle_projection(&handle, &HandleProjection::Anchor);
        let reference = anchor.strip_prefix("anchor-name:").unwrap();
        assert_eq!(
            render_handle_projection(&handle, &HandleProjection::Position),
            format!("position-anchor:{reference}")
        );
        assert_eq!(
            render_handle_projection(&handle, &HandleProjection::Scope),
            format!("anchor-scope:{reference}")
        );
        assert_eq!(
            render_handle_projection(
                &handle,
                &HandleProjection::Container(Spanned::new("size".into(), name.span))
            ),
            format!("container-name:{reference}container-type:size;")
        );
        handle.scope = Some(ScopeDefinition {
            module_path: "x".into(),
            name: name.clone(),
            label: Some(name.clone()),
            call_span: name.span,
        });
        let scope = scope_marker_class(handle.scope.as_ref().unwrap());
        assert!(scope.starts_with(".s_ref_"));
        handle.label = Some(name);
        assert_eq!(
            handle_marker_class(&handle),
            marker.replacen("s_", "s_ref_", 1)
        );
    }
}
