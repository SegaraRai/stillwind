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
use stillwind_ir::{BoundaryRef, RefDefinition, RefProjection};
use stillwind_semantic::{AssetKind, IdentityId};
use stillwind_source::{SourceFile, Span, Spanned, normalized_module_path};

use crate::identifiers::{
    AnchorName, ClassContext, ContainerName, DeclarationContext, DetachedName, IdentifierContext,
    IdentifierPool, Label, LocatorContext, RefMarker,
};

/// A generated CSS identifier with the canonical identity used to allocate it.
///
/// Names are deterministic but use truncated hashes and can collide. Consumers that
/// need to distinguish declarations across producers compare `identity`, not only
/// `name`. This record reports both values; constructing it does not check collisions.
///
/// A conditional helper is named for the CSS it serves and an anonymous layer for its
/// template, so neither has a declaration of its own: those records carry no position.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeneratedSymbol {
    /// The generated name, without a leading dot.
    pub name: String,
    /// Symbol family: `ref`, `ref-marker`, `keyframes`, `view-transition`, `class`,
    /// `custom-property`, or `layer`.
    pub kind: String,
    /// CSS namespace the name is written into. Names in different pools never interact,
    /// so only equal spellings from different identities within one pool collide.
    pub pool: IdentifierPool,
    /// Canonical identity the name hashes.
    pub identity: String,
    /// Normalized source path of the module that declared the symbol, or of the stylesheet
    /// that named it when the name has no declaration.
    pub source_path: String,
    /// Byte offset of the declaration's first character, when it has one.
    pub start: Option<u32>,
    /// Byte offset just past the declaration, when it has one.
    pub end: Option<u32>,
}

/// Builds the report record for one name that has no declaration position.
pub(crate) fn detached_symbol(name: &DetachedName, source_path: &str) -> GeneratedSymbol {
    GeneratedSymbol {
        name: name.name.clone(),
        kind: name.kind.as_str().to_owned(),
        pool: name.kind.pool(),
        identity: name.identity.clone(),
        source_path: normalized_module_path(source_path),
        start: None,
        end: None,
    }
}

/// Diagnostic-facing name of a ref projection.
pub(super) fn ref_projection_name(projection: &RefProjection) -> &'static str {
    match projection {
        RefProjection::Anchor => "anchor",
        RefProjection::PositionAnchor => "positionAnchor",
        RefProjection::AnchorScope => "anchorScope",
        RefProjection::Container(_) => "container",
    }
}

/// The declaration fields a ref's generated names are built from.
pub(super) fn ref_context(definition: &RefDefinition, projection: Option<&str>) -> LocatorContext {
    LocatorContext {
        module: normalized_module_path(&definition.module_path),
        start: definition.binding.span.start.0,
        end: definition.binding.span.end.0,
        binding: definition.binding.value.clone(),
        projection: projection.map(str::to_owned),
    }
}

/// The declaration fields a ref marker is built from, including its label.
pub(super) fn ref_declaration_context(definition: &RefDefinition) -> DeclarationContext {
    DeclarationContext {
        module: normalized_module_path(&definition.module_path),
        start: definition.binding.span.start.0,
        end: definition.binding.span.end.0,
        binding: definition.binding.value.clone(),
        // An empty label is no label; the declaration is named without one.
        label: definition
            .label
            .as_ref()
            .map(|label| Label::new(&label.value))
            .transpose()
            .ok()
            .flatten(),
    }
}

/// The declaration fields a boundary ref's marker is built from, including its label.
pub(super) fn boundary_declaration_context(boundary: &BoundaryRef) -> DeclarationContext {
    DeclarationContext {
        module: normalized_module_path(&boundary.module_path),
        start: boundary.binding.span.start.0,
        end: boundary.binding.span.end.0,
        binding: boundary.binding.value.clone(),
        // An empty label is no label; the declaration is named without one.
        label: boundary
            .label
            .as_ref()
            .map(|label| Label::new(&label.value))
            .transpose()
            .ok()
            .flatten(),
    }
}

/// Generated ref marker class token for one declaration.
///
/// # Errors
///
/// Returns the host generator's failure for the ref marker family.
pub(crate) fn ref_marker_name(
    context: &IdentifierContext,
    definition: &RefDefinition,
) -> Result<RefMarker, CompileErrorKind> {
    context.ref_marker_name(&ref_declaration_context(definition))
}

/// Generated marker class token for a boundary ref, without a leading dot.
///
/// # Errors
///
/// Returns the host generator's failure for the ref marker family.
pub(crate) fn boundary_marker_name(
    context: &IdentifierContext,
    boundary: &BoundaryRef,
) -> Result<RefMarker, CompileErrorKind> {
    context.ref_marker_name(&boundary_declaration_context(boundary))
}

/// Generated `anchor-name` for a ref declaration's identity.
///
/// # Errors
///
/// Returns the host generator's failure for the anchor family.
pub(crate) fn anchor_name(
    context: &IdentifierContext,
    definition: &RefDefinition,
    projection: Option<&str>,
) -> Result<AnchorName, CompileErrorKind> {
    context.anchor_name(&ref_context(definition, projection))
}

/// Generated `container-name` for a ref declaration's identity.
///
/// Both the `container()` projection that writes the name and the `at()` projection
/// that queries it resolve it through this one request, so a container query and the
/// container name it selects always agree.
///
/// # Errors
///
/// Returns the host generator's failure for the container family.
pub(crate) fn ref_container_name(
    context: &IdentifierContext,
    definition: &RefDefinition,
) -> Result<ContainerName, CompileErrorKind> {
    context.container_name(&ref_context(definition, Some("container")))
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

/// Identity and report spelling of one asset kind.
fn kind_name(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Style => "style",
        AssetKind::Ref => "ref",
        AssetKind::Keyframes => "keyframes",
        AssetKind::ViewTransition => "view-transition",
    }
}

/// The CSS namespace a named asset's generated name is written into.
fn kind_pool(kind: AssetKind) -> IdentifierPool {
    match kind {
        AssetKind::Style => IdentifierPool::ClassToken,
        AssetKind::Ref => IdentifierPool::AnchorName,
        AssetKind::Keyframes => IdentifierPool::KeyframesName,
        AssetKind::ViewTransition => IdentifierPool::ViewTransitionClass,
    }
}

/// The declaration a named asset's generated name is built from.
pub(super) fn named_declaration(
    module_path: &str,
    definition: &Spanned<String>,
) -> DeclarationContext {
    DeclarationContext {
        module: normalized_module_path(module_path),
        start: definition.span.start.0,
        end: definition.span.end.0,
        binding: definition.value.clone(),
        label: None,
    }
}

/// The declaration an identity reported by another module describes.
///
/// A use site in another module has the identity rather than the source buffer, so the
/// declaration is rebuilt from the identity's own fields.
pub(super) fn identity_declaration(identity: &IdentityId) -> DeclarationContext {
    DeclarationContext {
        module: normalized_module_path(&identity.module_path),
        start: identity.start,
        end: identity.end,
        binding: identity.binding_name.clone(),
        label: None,
    }
}

/// Generated name of one named asset, in the family its kind writes.
///
/// # Errors
///
/// Returns the host generator's failure for the declaration's family.
fn declared_name(
    context: &IdentifierContext,
    kind: AssetKind,
    declaration: &DeclarationContext,
) -> Result<String, CompileErrorKind> {
    let name = match kind {
        AssetKind::Keyframes => context.keyframes_name(declaration)?.as_str().to_owned(),
        AssetKind::ViewTransition => context
            .view_transition_class(declaration)?
            .as_str()
            .to_owned(),
        AssetKind::Style => context
            .class_name(&ClassContext {
                module: declaration.module.clone(),
                namespace: String::new(),
                call: declaration.start,
                branch: declaration.binding.clone(),
            })?
            .as_str()
            .to_owned(),
        AssetKind::Ref => context
            .anchor_name(&locator(declaration))?
            .as_str()
            .to_owned(),
    };
    Ok(name)
}

/// The locator position one declaration names, without a projection.
fn locator(declaration: &DeclarationContext) -> LocatorContext {
    LocatorContext {
        module: declaration.module.clone(),
        start: declaration.start,
        end: declaration.end,
        binding: declaration.binding.clone(),
        projection: None,
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
        name: declared_name(context, kind, &named_declaration(source.path(), definition))?,
        kind: kind_name(kind).to_owned(),
        pool: kind_pool(kind),
        identity: symbol_identity(source, kind_name(kind), definition),
        source_path: normalized_module_path(source.path()),
        start: Some(definition.span.start.0),
        end: Some(definition.span.end.0),
    })
}

/// Builds the reportable symbol for a ref declaration's projection name.
///
/// # Errors
///
/// Returns the host generator's failure for the anchor family.
pub(super) fn generated_ref_symbol(
    context: &IdentifierContext,
    definition: &RefDefinition,
) -> Result<GeneratedSymbol, CompileErrorKind> {
    let identity = identity_input(&ref_identity(definition));
    Ok(GeneratedSymbol {
        name: anchor_name(context, definition, None)?.as_str().to_owned(),
        kind: "ref".to_owned(),
        pool: IdentifierPool::AnchorName,
        identity,
        source_path: definition.module_path.clone(),
        start: Some(definition.binding.span.start.0),
        end: Some(definition.binding.span.end.0),
    })
}

/// Builds the reportable symbol for a ref's marker class.
///
/// # Errors
///
/// Returns the host generator's failure for the ref marker family.
pub(super) fn generated_ref_marker_symbol(
    context: &IdentifierContext,
    definition: &RefDefinition,
) -> Result<GeneratedSymbol, CompileErrorKind> {
    Ok(GeneratedSymbol {
        name: ref_marker_name(context, definition)?.as_str().to_owned(),
        kind: "ref-marker".to_owned(),
        pool: IdentifierPool::ClassToken,
        identity: asset_identity(&definition.module_path, "ref-marker", &definition.binding),
        source_path: definition.module_path.clone(),
        start: Some(definition.binding.span.start.0),
        end: Some(definition.binding.span.end.0),
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
        start: Some(span.start.0),
        end: Some(span.end.0),
    }
}

/// Generates one declared definition's name and reports it as a symbol.
///
/// `kind` selects the family the name belongs to, which is the kind the report carries.
/// The span addresses the declaration; the writer's span is the caller's business.
///
/// # Errors
///
/// Returns the host generator's failure for the asset's family.
pub(super) fn definition_name_symbol(
    context: &IdentifierContext,
    identity: &IdentityId,
    kind: AssetKind,
    span: Span,
) -> Result<GeneratedSymbol, CompileErrorKind> {
    let name = declared_name(context, kind, &identity_declaration(identity))?;
    Ok(GeneratedSymbol {
        name,
        kind: kind_name(kind).to_owned(),
        pool: kind_pool(kind),
        identity: identity_input(identity),
        source_path: normalized_module_path(&identity.module_path),
        start: Some(span.start.0),
        end: Some(span.end.0),
    })
}

/// Canonical identity input for a declaration's generated names.
pub(super) fn identity_input(identity: &IdentityId) -> String {
    format!(
        "stillwind/v1\0{}\0{}\0{}:{}:{}",
        normalized_module_path(&identity.module_path),
        kind_name(identity.kind),
        identity.start,
        identity.end,
        identity.binding_name
    )
}

/// Canonical identity text for one declaration's fields, in the versioned spelling.
///
/// The identity is what a symbol report compares, so it must not depend on the family
/// it was written for.
pub(super) fn identity_input_parts(context: &LocatorContext) -> String {
    format!(
        "stillwind/v1\0{}\0ref\0{}:{}:{}",
        context.module, context.start, context.end, context.binding,
    )
}

pub(super) fn ref_identity(definition: &RefDefinition) -> IdentityId {
    IdentityId::new(
        AssetKind::Ref,
        &definition.module_path,
        definition.binding.span.start.0,
        definition.binding.span.end.0,
        definition.binding.value.clone(),
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

/// Builds one reportable symbol for a name a declaration writes into a pool.
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
        start: Some(span.start.0),
        end: Some(span.end.0),
    }
}
#[cfg(test)]
mod tests {
    use stillwind_source::FileId;

    use super::*;
    use crate::identifiers::{IdentifierContext, Label, RefMarker};

    #[test]
    fn identities_normalize_paths_and_class_names_are_content_stable() {
        let identifiers = IdentifierContext::builtin();
        let class = |branch: &str| ClassContext {
            module: "src/x.tsx".to_owned(),
            namespace: String::new(),
            call: 0,
            branch: branch.to_owned(),
        };
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
            identifiers.class_name(&class("flex")),
            identifiers.class_name(&class("flex"))
        );
        assert_ne!(
            identifiers.class_name(&class("flex")),
            identifiers.class_name(&class("grid"))
        );
        assert!(
            identifiers
                .class_name(&class("flex"))
                .unwrap()
                .as_str()
                .starts_with("_s_")
        );
    }

    #[test]
    fn asset_names_distinguish_kinds_and_spans_and_keep_metadata() {
        let identifiers = IdentifierContext::builtin();
        let source = SourceFile::new(FileId(1), r"src\x.tsx", "ref").unwrap();
        let name = Spanned::new("ref".into(), Span::new(FileId(1), 0, 3));
        for (kind, prefix) in [
            (AssetKind::Ref, "--sw-r-"),
            (AssetKind::Keyframes, "--sw-a-"),
            (AssetKind::ViewTransition, "--sw-v-"),
        ] {
            let symbol = generated_symbol(&identifiers, &source, kind, &name).unwrap();
            assert!(symbol.name.starts_with(prefix));
            assert_eq!(symbol.source_path, "src/x.tsx");
            assert_eq!((symbol.start, symbol.end), (Some(0), Some(3)));
            assert_eq!(
                symbol.identity,
                asset_identity(source.path(), kind_name(kind), &name)
            );
            let shifted = Spanned::new("ref".into(), Span::new(FileId(1), 1, 3));
            let shifted_name = declared_name(
                &identifiers,
                kind,
                &named_declaration(source.path(), &shifted),
            )
            .unwrap();
            assert_ne!(symbol.name, shifted_name);
        }
    }
    #[test]
    fn labels_preserve_punctuation_and_replace_whitespace() {
        assert_eq!(Label::new("項目-🧭").unwrap().as_str(), "項目-🧭");
        assert_eq!(Label::new("a b:c/d_").unwrap().as_str(), "a_b:c/d_");
        assert_eq!(Label::new("a\t\nb").unwrap().as_str(), "a__b");
        assert_eq!(RefMarker::new("a:b/c").unwrap().selector(), r".a\:b\/c");
    }
}
