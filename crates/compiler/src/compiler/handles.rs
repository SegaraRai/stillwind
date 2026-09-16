//! Derived names and identities for ref and scope assets.
//!
//! A declaration's identity is independent of its projections: `anchor`, `position`,
//! `scope`, and `container` all reference the same generated name, which is what makes
//! anchor positioning resolve to the element the ref was attached to.
//!
//! A label stays readable in the marker class, so the DOM token keeps the authored text;
//! escaping for a CSS selector or a generated JavaScript string literal happens where
//! those are written.
//!
//! [`summary_runtime_assets`] seeds runtime reachability from module exports; call
//! emission adds uses. Unreferenced producer calls are replaced by `undefined`, but
//! static motion uses retain CSS without requiring a runtime producer value.

use std::collections::BTreeSet;

use stillwind_ir::{HandleDefinition, HandleProjection, ScopeDefinition};
use stillwind_semantic::{AssetKind, Candidates, ExportedValue, IdentityId, ModuleSummary};
use stillwind_source::{SourceFile, Spanned};

use super::symbols::anchor_name;
use super::symbols::{GeneratedSymbol, projection_symbol};
use super::symbols::{identity_input_parts, ref_context};
use crate::identifier::IdentifierPool;
use crate::identifier::{IdentifierContext, custom_ident};

/// The declaration that makes a projection usable on the element itself, and the
/// reportable symbol for the name it writes.
///
/// An anchor projection writes the declaration's `anchor-name`, and a container
/// projection writes its `container-name`; both names are written for the ref's
/// identity, so one declaration can legitimately appear in two CSS namespaces under
/// the same spelling.
///
/// # Errors
///
/// Returns the host generator's failure for the anchor or container family.
pub(super) fn render_handle_projection(
    context: &IdentifierContext,
    handle: &HandleDefinition,
    projection: &HandleProjection,
    source_path: &str,
) -> Result<(String, Option<GeneratedSymbol>), stillwind_diagnostics::CompileErrorKind> {
    let name = anchor_name(context, handle, Some(projection_name(projection)))?;
    let symbol = |name: &str, pool: IdentifierPool, kind: &str| {
        Some(projection_symbol(
            name,
            identity_input_parts(&ref_context(handle, None)),
            pool,
            kind,
            source_path,
            handle.call_span,
        ))
    };
    let name = custom_ident(&name);
    Ok(match projection {
        HandleProjection::Anchor => (
            format!("anchor-name:{name};"),
            symbol(&name, IdentifierPool::AnchorName, "anchor"),
        ),
        HandleProjection::Position => (format!("position-anchor:{name};"), None),
        HandleProjection::Scope => (format!("anchor-scope:{name};"), None),
        HandleProjection::Container(kind) => {
            let container = context.container_name(&ref_context(handle, Some("container")))?;
            (
                format!(
                    "container-name:{};container-type:{};",
                    custom_ident(&container),
                    kind.value
                ),
                Some(projection_symbol(
                    &container,
                    identity_input_parts(&ref_context(handle, None)),
                    IdentifierPool::ContainerName,
                    "container",
                    source_path,
                    handle.call_span,
                )),
            )
        }
    })
}

/// The projection name one written anchor or container name belongs to.
///
/// A ref can be projected into several roles, and the projection is part of the context
/// a generator may use; it is not part of the identity.
fn projection_name(projection: &HandleProjection) -> &'static str {
    match projection {
        HandleProjection::Anchor => "anchor",
        HandleProjection::Position => "position",
        HandleProjection::Scope => "scope",
        HandleProjection::Container(_) => "container",
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

/// Identity of a scope declaration, spanning its name token.
pub(super) fn scope_identity(scope: &ScopeDefinition) -> IdentityId {
    IdentityId::new(
        AssetKind::Scope,
        &scope.module_path,
        scope.name.span.start.0,
        scope.name.span.end.0,
        scope.name.value.clone(),
    )
}

/// Identity of a named motion asset, spanning its name token.
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

/// Every asset identity the module's exports can reach.
///
/// Finite candidates are exact roots. An exported universe conservatively retains all
/// owned declarations of that kind, because the exporter cannot prove them unreachable.
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
            ExportedValue::Styles(value)
            | ExportedValue::Scopes(value)
            | ExportedValue::Animations(value)
            | ExportedValue::ViewTransitions(value) => {
                if let Candidates::Finite(candidates) = &value.candidates {
                    identities.extend(candidates.iter().cloned());
                } else {
                    identities.extend(
                        summary
                            .assets
                            .iter()
                            .filter(|asset| asset.identity.kind == value.kind)
                            .map(|asset| asset.identity.clone()),
                    );
                }
            }
        }
    }
    identities
}

#[cfg(test)]
mod tests {
    use stillwind_ir::{HandleDefinition, ScopeDefinition};
    use stillwind_semantic::{AssetMetadata, AssetRecord, SymbolicValue};
    use stillwind_source::{FileId, Span};

    use super::*;
    use crate::compiler::symbols::{ref_marker_name, scope_marker_name};
    use crate::identifier::{IdentifierContext, IdentifierPool, marker_label, selector};

    #[test]
    fn labels_preserve_punctuation_and_replace_whitespace() {
        assert_eq!(marker_label("項目-🧭"), "項目-🧭");
        assert_eq!(marker_label("a b:c/d_"), "a_b:c/d_");
        assert_eq!(marker_label("a\t\nb"), "a__b");
        assert_eq!(selector("a:b/c"), r".a\:b\/c");
    }

    #[test]
    fn exported_universe_retains_owned_assets_of_its_kind() {
        let mut summary = ModuleSummary::new("x");
        let animation = IdentityId::new(AssetKind::Animation, "x", 0, 1, "a");
        summary.assets.push(AssetRecord {
            identity: animation.clone(),
            metadata: AssetMetadata { label: None },
            call_start: 2,
            call_end: 8,
        });
        summary.exports.insert(
            "chosen".into(),
            ExportedValue::Animations(SymbolicValue::universe(AssetKind::Animation)),
        );
        assert_eq!(
            summary_runtime_assets(&summary),
            BTreeSet::from([animation])
        );
    }

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
        let identifiers = IdentifierContext::builtin();
        let marker = ref_marker_name(&identifiers, &handle).unwrap();
        let (anchor, anchor_symbol) =
            render_handle_projection(&identifiers, &handle, &HandleProjection::Anchor, "x")
                .unwrap();
        let reference = anchor.strip_prefix("anchor-name:").unwrap();
        let anchor_symbol = anchor_symbol.expect("anchor name symbol");
        assert_eq!(anchor_symbol.pool, IdentifierPool::AnchorName);
        assert_eq!(
            anchor_symbol.identity,
            identity_input_parts(&ref_context(&handle, None))
        );
        let (position, _) =
            render_handle_projection(&identifiers, &handle, &HandleProjection::Position, "x")
                .unwrap();
        assert_eq!(position, format!("position-anchor:{reference}"));
        let (scope_projection, _) =
            render_handle_projection(&identifiers, &handle, &HandleProjection::Scope, "x").unwrap();
        assert_eq!(scope_projection, format!("anchor-scope:{reference}"));
        let (container, container_symbol) = render_handle_projection(
            &identifiers,
            &handle,
            &HandleProjection::Container(Spanned::new("size".into(), name.span)),
            "x",
        )
        .unwrap();
        // A container name is its own family, so it is not the anchor's spelling.
        let container_name = container
            .strip_prefix("container-name:")
            .and_then(|rest| rest.strip_suffix("container-type:size;"))
            .expect("container declaration");
        assert!(container_name.starts_with("--sw-s-"), "{container_name}");
        assert_ne!(container_name, reference);
        let container_symbol = container_symbol.expect("container name symbol");
        assert_eq!(container_symbol.pool, IdentifierPool::ContainerName);
        assert_eq!(
            container_symbol.identity,
            identity_input_parts(&ref_context(&handle, None))
        );
        handle.scope = Some(ScopeDefinition {
            module_path: "x".into(),
            name: name.clone(),
            label: Some(name.clone()),
            call_span: name.span,
        });
        let scope = scope_marker_name(&identifiers, handle.scope.as_ref().unwrap()).unwrap();
        assert!(scope.starts_with("s_"), "{scope}");
        handle.label = Some(name);
        // The built-in name hashes the declaration alone, so a label cannot move it:
        // every position the marker is written to resolves the same declaration.
        assert_eq!(ref_marker_name(&identifiers, &handle).unwrap(), marker);
    }
}
