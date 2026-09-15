//! Deterministic generated identifiers.
//!
//! Every name is the first eight hex digits of a hash over an identity string of the
//! form `stillwind/v1\0<module>\0<kind>\0<start>:<end>:<text>`, with normalized module
//! paths so the same declaration yields the same name on Windows and on POSIX. Class
//! names additionally mix in the callsite offset and a per-call branch index, which is
//! what keeps two callsites with identical utility sets in different class units. The
//! truncation makes a collision conceivable, so [`GeneratedSymbol`] reports the full
//! identity next to the name and a consumer that must be certain compares identities
//! instead of names.

use serde::Serialize;
use stillwind_diagnostics::CompileErrorKind;

use stillwind_ir::{HandleDefinition, HandleProjection, ScopeDefinition};
use stillwind_semantic::{AssetKind, IdentityId};
use stillwind_source::{SourceFile, Span, Spanned, normalized_module_path};

use crate::identifier::{IdentifierContext, IdentifierPool};

/// A generated CSS identifier with the canonical identity used to allocate it.
///
/// Names are deterministic but use truncated hashes and can collide. Consumers that
/// need to distinguish declarations across producers compare `identity`, not only
/// `name`. This record reports both values; constructing it does not check collisions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeneratedSymbol {
    /// The generated name, without a leading dot.
    pub name: String,
    /// Symbol family: `ref`, `scope`, `ref-marker`, `scope-marker`, `animation`,
    /// `view-transition`, or `class`.
    pub kind: String,
    /// CSS namespace the name is written into. Names in different pools never interact,
    /// so only equal spellings from different identities within one pool collide.
    pub pool: IdentifierPool,
    /// Canonical identity the name hashes.
    pub identity: String,
    /// Normalized source path of the module that declared the symbol.
    pub source_path: String,
    /// Byte offset of the declaration's first character.
    pub start: u32,
    /// Byte offset just past the declaration.
    pub end: u32,
}

/// Diagnostic-facing name of a handle projection.
pub(super) fn handle_projection_name(projection: &HandleProjection) -> &'static str {
    match projection {
        HandleProjection::Anchor => "anchor",
        HandleProjection::Position => "position",
        HandleProjection::Scope => "scope",
        HandleProjection::Container(_) => "container",
    }
}

/// Class name used by the runtime projection lookup table.
///
/// The table key is the marker class of the handle, so the runtime picks the projected
/// class of whichever handle it was given.
pub(super) fn runtime_projection_class(
    context: &IdentifierContext,
    handle: &HandleDefinition,
    projection: &HandleProjection,
) -> Result<String, CompileErrorKind> {
    let suffix = match projection {
        HandleProjection::Anchor => "anchor",
        HandleProjection::Position => "position",
        HandleProjection::Scope => "scope",
        HandleProjection::Container(kind) => kind.value.as_str(),
    };
    Ok(format!(
        "_swp_{}_{suffix}",
        ref_marker_name(context, handle)?
    ))
}

/// Canonical identity input of a ref declaration's generated names.
pub(super) fn ref_name_input(definition: &HandleDefinition) -> String {
    asset_identity(&definition.module_path, "ref", &definition.name)
}

/// Generated ref marker class token for one declaration.
///
/// # Errors
///
/// Returns the host generator's failure for the ref marker family.
pub(crate) fn ref_marker_name(
    context: &IdentifierContext,
    definition: &HandleDefinition,
) -> Result<String, CompileErrorKind> {
    context.ref_marker_name(
        &ref_name_input(definition),
        definition.label.as_ref().map(|label| label.value.as_str()),
    )
}

/// Canonical identity input of a scope declaration's generated names.
pub(super) fn scope_name_input(definition: &ScopeDefinition) -> String {
    asset_identity(&definition.module_path, "scope", &definition.name)
}

/// Generated scope marker class token for one declaration, without a leading dot.
///
/// # Errors
///
/// Returns the host generator's failure for the scope marker family.
pub(crate) fn scope_marker_name(
    context: &IdentifierContext,
    definition: &ScopeDefinition,
) -> Result<String, CompileErrorKind> {
    context.scope_marker_name(
        &scope_name_input(definition),
        definition.label.as_ref().map(|label| label.value.as_str()),
    )
}

/// Generated `anchor-name` for a ref declaration's identity.
///
/// # Errors
///
/// Returns the host generator's failure for the anchor family.
pub(crate) fn anchor_name(
    context: &IdentifierContext,
    definition: &HandleDefinition,
) -> Result<String, CompileErrorKind> {
    context.anchor_name(&ref_name_input(definition))
}

/// Generated `container-name` for a scope declaration's identity.
///
/// # Errors
///
/// Returns the host generator's failure for the container family.
pub(super) fn container_name(
    context: &IdentifierContext,
    definition: &ScopeDefinition,
) -> Result<String, CompileErrorKind> {
    context.container_name(&scope_name_input(definition))
}

/// Identity string for a declaration inside one source file.
pub(super) fn symbol_identity(
    source: &SourceFile,
    kind: &str,
    definition: &Spanned<String>,
) -> String {
    format!(
        "stillwind/v1\0{}\0{kind}\0{}:{}:{}",
        normalized_module_path(source.path()),
        definition.span.start.0,
        definition.span.end.0,
        definition.value,
    )
}

/// Identity string for a declaration addressed by module path.
///
/// Used where the source buffer is not available, for example when a use site refers to
/// an asset declared in another module.
pub(super) fn asset_identity(
    module_path: &str,
    kind: &str,
    definition: &Spanned<String>,
) -> String {
    format!(
        "stillwind/v1\0{}\0{kind}\0{}:{}:{}",
        normalized_module_path(module_path),
        definition.span.start.0,
        definition.span.end.0,
        definition.value,
    )
}

/// Generated custom-property name of a ref's anchor identity.
fn kind_name(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Style => "style",
        AssetKind::Ref => "ref",
        AssetKind::Scope => "scope",
        AssetKind::Animation => "animation",
        AssetKind::ViewTransition => "view-transition",
    }
}

/// The CSS namespace a named asset's generated name is written into.
fn kind_pool(kind: AssetKind) -> IdentifierPool {
    match kind {
        AssetKind::Style => IdentifierPool::ClassToken,
        AssetKind::Ref => IdentifierPool::AnchorName,
        AssetKind::Scope => IdentifierPool::ContainerName,
        AssetKind::Animation => IdentifierPool::KeyframesName,
        AssetKind::ViewTransition => IdentifierPool::ViewTransitionName,
    }
}

/// Generated name of a named motion declaration, prefix selected by kind.
///
/// # Errors
///
/// Returns the host generator's failure for the declaration's family.
pub(super) fn motion_name(
    context: &IdentifierContext,
    module_path: &str,
    kind: AssetKind,
    definition: &Spanned<String>,
) -> Result<String, CompileErrorKind> {
    let identity = asset_identity(module_path, kind_name(kind), definition);
    match kind {
        AssetKind::Animation => context.animation_name(&identity),
        AssetKind::ViewTransition => context.view_transition_name(&identity),
        AssetKind::Style => context.class_name(&identity),
        AssetKind::Ref => context.anchor_name(&identity),
        AssetKind::Scope => context.container_name(&identity),
    }
}

/// Builds the reportable symbol for a named class or motion declaration.
///
/// The name is determined once and reused, so a report and the code that uses the
/// name cannot disagree.
///
/// # Errors
///
/// Returns the host generator's failure for the declaration's family.
pub(super) fn generated_symbol(
    context: &IdentifierContext,
    source: &SourceFile,
    kind: AssetKind,
    definition: &Spanned<String>,
) -> Result<GeneratedSymbol, CompileErrorKind> {
    Ok(GeneratedSymbol {
        name: motion_name(context, source.path(), kind, definition)?,
        kind: kind_name(kind).to_owned(),
        pool: kind_pool(kind),
        identity: symbol_identity(source, kind_name(kind), definition),
        source_path: normalized_module_path(source.path()),
        start: definition.span.start.0,
        end: definition.span.end.0,
    })
}

/// Builds the reportable symbol for a ref declaration's projection name.
///
/// # Errors
///
/// Returns the host generator's failure for the anchor family.
pub(super) fn generated_ref_symbol(
    context: &IdentifierContext,
    definition: &HandleDefinition,
) -> Result<GeneratedSymbol, CompileErrorKind> {
    let identity = ref_name_input(definition);
    Ok(GeneratedSymbol {
        name: anchor_name(context, definition)?,
        kind: "ref".to_owned(),
        pool: IdentifierPool::AnchorName,
        identity,
        source_path: definition.module_path.clone(),
        start: definition.name.span.start.0,
        end: definition.name.span.end.0,
    })
}

/// Builds the reportable symbol for a scope declaration's projection name.
///
/// # Errors
///
/// Returns the host generator's failure for the container family.
pub(super) fn generated_scope_symbol(
    context: &IdentifierContext,
    definition: &ScopeDefinition,
) -> Result<GeneratedSymbol, CompileErrorKind> {
    let identity = scope_name_input(definition);
    Ok(GeneratedSymbol {
        name: container_name(context, definition)?,
        kind: "scope".to_owned(),
        pool: IdentifierPool::ContainerName,
        identity,
        source_path: definition.module_path.clone(),
        start: definition.name.span.start.0,
        end: definition.name.span.end.0,
    })
}

/// Builds the reportable symbol for a ref's marker class.
///
/// # Errors
///
/// Returns the host generator's failure for the ref marker family.
pub(super) fn generated_ref_marker_symbol(
    context: &IdentifierContext,
    definition: &HandleDefinition,
) -> Result<GeneratedSymbol, CompileErrorKind> {
    Ok(GeneratedSymbol {
        name: ref_marker_name(context, definition)?,
        kind: "ref-marker".to_owned(),
        pool: IdentifierPool::ClassToken,
        identity: asset_identity(&definition.module_path, "ref-marker", &definition.name),
        source_path: definition.module_path.clone(),
        start: definition.name.span.start.0,
        end: definition.name.span.end.0,
    })
}

/// Builds the reportable symbol for a scope's marker class.
///
/// # Errors
///
/// Returns the host generator's failure for the scope marker family.
pub(super) fn generated_scope_marker_symbol(
    context: &IdentifierContext,
    definition: &ScopeDefinition,
) -> Result<GeneratedSymbol, CompileErrorKind> {
    Ok(GeneratedSymbol {
        name: scope_marker_name(context, definition)?,
        kind: "scope-marker".to_owned(),
        pool: IdentifierPool::ClassToken,
        identity: asset_identity(&definition.module_path, "scope-marker", &definition.name),
        source_path: definition.module_path.clone(),
        start: definition.name.span.start.0,
        end: definition.name.span.end.0,
    })
}

/// Builds the reportable symbol for a generated utility class.
///
/// The identity keeps the whole canonical input - namespace, module, callsite, branch,
/// and rule spelling - rather than the truncated name, so two producers can prove they
/// generated the same class or detect that they generated different ones with equal
/// names.
pub(super) fn generated_class_symbol(
    source: &SourceFile,
    name: &str,
    canonical_input: &str,
    span: Span,
) -> GeneratedSymbol {
    GeneratedSymbol {
        name: name.to_owned(),
        kind: "class".to_owned(),
        pool: IdentifierPool::ClassToken,
        identity: format!("stillwind/v1\0class\0{canonical_input}"),
        source_path: normalized_module_path(source.path()),
        start: span.start.0,
        end: span.end.0,
    }
}

/// Class unit name for a canonical input.
/// Uses declaration identity directly, without fabricating a source-buffer span.
/// Uses declaration identity directly, without fabricating a source-buffer span.
///
/// # Errors
///
/// Returns the host generator's failure for the asset's family.
pub(super) fn motion_symbol_name(
    context: &IdentifierContext,
    identity: &IdentityId,
) -> Result<String, CompileErrorKind> {
    let identity_input = format!(
        "stillwind/v1\0{}\0{}\0{}:{}:{}",
        normalized_module_path(&identity.module_path),
        kind_name(identity.kind),
        identity.start,
        identity.end,
        identity.binding_name
    );
    match identity.kind {
        AssetKind::Animation => context.animation_name(&identity_input),
        AssetKind::ViewTransition => context.view_transition_name(&identity_input),
        AssetKind::Style => context.class_name(&identity_input),
        AssetKind::Ref => context.anchor_name(&identity_input),
        AssetKind::Scope => context.container_name(&identity_input),
    }
}

/// Builds the reportable symbol for a written anchor or container name.
///
/// Both `anchor_name` and `container_name` are written for one declaration's identity,
/// and the same declaration may be written many times, so the pipeline drops repeated
/// records before reporting. The identity is the declaration's, which is what makes a
/// shared name across pools a reuse rather than a collision.
pub(crate) fn projection_symbol(
    name: &str,
    identity: String,
    pool: IdentifierPool,
    kind: &str,
    source_path: &str,
    span: Span,
) -> GeneratedSymbol {
    GeneratedSymbol {
        name: name.to_owned(),
        kind: kind.to_owned(),
        pool,
        identity,
        source_path: normalized_module_path(source_path),
        start: span.start.0,
        end: span.end.0,
    }
}
#[cfg(test)]
mod tests {
    use stillwind_source::FileId;

    use super::*;
    use crate::identifier::IdentifierContext;

    #[test]
    fn identities_normalize_paths_and_class_names_are_content_stable() {
        let identifiers = IdentifierContext::builtin();
        let name = Spanned::new("ref".into(), Span::new(FileId(1), 2, 5));
        assert_eq!(
            asset_identity(r"src\x.tsx", "ref", &name),
            asset_identity("src/x.tsx", "ref", &name)
        );
        assert_ne!(
            asset_identity("src/x.tsx", "ref", &name),
            asset_identity("src/y.tsx", "ref", &name)
        );
        assert_eq!(
            identifiers.class_name("flex"),
            identifiers.class_name("flex")
        );
        assert_ne!(
            identifiers.class_name("flex"),
            identifiers.class_name("grid")
        );
        assert!(identifiers.class_name("flex").unwrap().starts_with("_s_"));
    }

    #[test]
    fn asset_names_distinguish_kinds_and_spans_and_keep_metadata() {
        let identifiers = IdentifierContext::builtin();
        let source = SourceFile::new(FileId(1), r"src\x.tsx", "ref").unwrap();
        let name = Spanned::new("ref".into(), Span::new(FileId(1), 0, 3));
        for (kind, prefix) in [
            (AssetKind::Ref, "--sw-r-"),
            (AssetKind::Scope, "--sw-s-"),
            (AssetKind::Animation, "--sw-a-"),
            (AssetKind::ViewTransition, "--sw-v-"),
        ] {
            let symbol = generated_symbol(&identifiers, &source, kind, &name).unwrap();
            assert!(symbol.name.starts_with(prefix));
            assert_eq!(symbol.source_path, "src/x.tsx");
            assert_eq!((symbol.start, symbol.end), (0, 3));
            assert_eq!(
                symbol.identity,
                asset_identity(source.path(), kind_name(kind), &name)
            );
            let shifted = Spanned::new("ref".into(), Span::new(FileId(1), 1, 3));
            assert_ne!(
                symbol.name,
                motion_name(&identifiers, source.path(), kind, &shifted).unwrap()
            );
        }
    }
}
