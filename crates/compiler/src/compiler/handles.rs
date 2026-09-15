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
use crate::identifier::{IdentifierContext, custom_ident};

/// The declaration that makes a projection usable on the element itself.
pub(super) fn render_handle_projection(
    context: &IdentifierContext,
    handle: &HandleDefinition,
    projection: &HandleProjection,
) -> Result<String, stillwind_diagnostics::CompileErrorKind> {
    let name = anchor_name(context, handle)?;
    let name = custom_ident(&name);
    Ok(match projection {
        HandleProjection::Anchor => format!("anchor-name:{name};"),
        HandleProjection::Position => format!("position-anchor:{name};"),
        HandleProjection::Scope => format!("anchor-scope:{name};"),
        HandleProjection::Container(kind) => {
            format!("container-name:{name};container-type:{};", kind.value)
        }
    })
}

/// Identity of a ref declaration, spanning its name token.
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
    use crate::identifier::{IdentifierContext, marker_label, selector};

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
        let anchor =
            render_handle_projection(&identifiers, &handle, &HandleProjection::Anchor).unwrap();
        let reference = anchor.strip_prefix("anchor-name:").unwrap();
        assert_eq!(
            render_handle_projection(&identifiers, &handle, &HandleProjection::Position).unwrap(),
            format!("position-anchor:{reference}")
        );
        assert_eq!(
            render_handle_projection(&identifiers, &handle, &HandleProjection::Scope).unwrap(),
            format!("anchor-scope:{reference}")
        );
        assert_eq!(
            render_handle_projection(
                &identifiers,
                &handle,
                &HandleProjection::Container(Spanned::new("size".into(), name.span))
            )
            .unwrap(),
            format!("container-name:{reference}container-type:size;")
        );
        handle.scope = Some(ScopeDefinition {
            module_path: "x".into(),
            name: name.clone(),
            label: Some(name.clone()),
            call_span: name.span,
        });
        let scope = scope_marker_name(&identifiers, handle.scope.as_ref().unwrap()).unwrap();
        assert!(scope.starts_with("s_ref_"));
        handle.label = Some(name);
        assert_eq!(
            ref_marker_name(&identifiers, &handle).unwrap(),
            marker.replacen("s_", "s_ref_", 1)
        );
    }
}
