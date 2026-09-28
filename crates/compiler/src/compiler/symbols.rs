//! Deterministic generated identifiers.
//!
//! Generated names use hashes of requests. [`GeneratedSymbol`] carries a separate owner
//! identity alongside each written name so document-wide collision checks can
//! distinguish name reuse from a collision.

use serde::Serialize;

use stillwind_diagnostics::CompileErrorKind;
use stillwind_ir::{AssetReference, BoundaryRef, RefDefinition, RefProjection};
use stillwind_semantic::{AssetKind, IdentityId};
use stillwind_source::{CssSourceName, ModuleId, Span, Spanned};

use crate::identifiers::{
    AnchorName, ClassContext, ContainerName, DeclarationContext, DetachedName, IdentifierContext,
    IdentifierPool, Label, LocatorContext, RefMarker, declaration_fields,
};

/// A generated CSS identifier with the identity of its owner.
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
    /// `condition-helper`, `view-transition-slot`, or `layer`.
    pub kind: String,
    /// CSS namespace the name is written into. Names in different pools never interact,
    /// so only equal spellings from different identities within one pool collide.
    pub pool: IdentifierPool,
    /// Owner identity used to distinguish name reuse from a collision in the same pool.
    pub identity: String,
    /// Source that owns the declaration or detached name.
    pub origin: SymbolOrigin,
    /// Byte offset of the declaration's first character, when it has one.
    pub start: Option<u32>,
    /// Byte offset just past the declaration, when it has one.
    pub end: Option<u32>,
}

/// Source identity used to attribute a generated name.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "kebab-case")]
pub enum SymbolOrigin {
    /// A script module.
    Module(ModuleId),
    /// A stylesheet that named a detached helper or layer.
    Stylesheet(CssSourceName),
}

impl SymbolOrigin {
    /// Returns the declaring module when the name comes from a script.
    pub fn module_id(&self) -> Option<&ModuleId> {
        match self {
            Self::Module(id) => Some(id),
            Self::Stylesheet(_) => None,
        }
    }
}

/// Builds the report record for one name that has no declaration position.
pub(crate) fn detached_symbol(name: &DetachedName, origin: SymbolOrigin) -> GeneratedSymbol {
    GeneratedSymbol {
        name: name.name.clone(),
        kind: name.kind.as_str().to_owned(),
        pool: name.kind.pool(),
        identity: name.identity.clone(),
        origin,
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
        module: definition.module_id.clone(),
        start: definition.binding.span.start.0,
        end: definition.binding.span.end.0,
        binding: definition.binding.value.clone(),
        label: declared_label(definition.label.as_ref()),
        projection: projection.map(str::to_owned),
    }
}

/// The declaration fields a ref marker is built from, including its label.
pub(super) fn ref_declaration_context(definition: &RefDefinition) -> DeclarationContext {
    DeclarationContext {
        module: definition.module_id.clone(),
        start: definition.binding.span.start.0,
        end: definition.binding.span.end.0,
        binding: definition.binding.value.clone(),
        label: declared_label(definition.label.as_ref()),
    }
}

/// The declaration fields a boundary ref's marker is built from, including its label.
pub(super) fn boundary_declaration_context(boundary: &BoundaryRef) -> DeclarationContext {
    DeclarationContext {
        module: boundary.module_id.clone(),
        start: boundary.binding.span.start.0,
        end: boundary.binding.span.end.0,
        binding: boundary.binding.value.clone(),
        label: declared_label(boundary.label.as_ref()),
    }
}

/// The authored label of one declaration, normalized for name use.
///
/// An empty label is no label: a declaration whose label cleans to nothing is named
/// without one rather than failing.
fn declared_label(label: Option<&Spanned<String>>) -> Option<Label> {
    label.and_then(|label| Label::new(&label.value).ok())
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

/// Owner identity for a named declaration.
fn owner_declaration_identity(
    module_id: &ModuleId,
    kind: &str,
    start: u32,
    end: u32,
    binding: &str,
) -> String {
    format!(
        "stillwind-identity/v1\0{}",
        declaration_fields(module_id, kind, start, end, binding),
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
///
/// `label` is the declaration's authored label, which the built-in generator spells
/// into the name.
pub(super) fn named_declaration(
    module_id: &ModuleId,
    definition: &Spanned<String>,
    label: Option<&Spanned<String>>,
) -> DeclarationContext {
    DeclarationContext {
        module: module_id.clone(),
        start: definition.span.start.0,
        end: definition.span.end.0,
        binding: definition.value.clone(),
        label: declared_label(label),
    }
}

/// The declaration an identity reported by another module describes.
///
/// A use site in another module has the identity rather than the source buffer, so the
/// declaration is rebuilt from the identity's own fields and the label the reference
/// carries.
pub(super) fn identity_declaration(reference: &AssetReference) -> DeclarationContext {
    DeclarationContext {
        module: reference.identity.module_id.clone(),
        start: reference.identity.start,
        end: reference.identity.end,
        binding: reference.identity.binding_name.clone(),
        label: declared_label(reference.label.as_ref()),
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
        label: declaration.label.clone(),
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
    module_id: &ModuleId,
    kind: AssetKind,
    definition: &Spanned<String>,
    label: Option<&Spanned<String>>,
) -> Result<GeneratedSymbol, CompileErrorKind> {
    Ok(GeneratedSymbol {
        name: declared_name(
            context,
            kind,
            &named_declaration(module_id, definition, label),
        )?,
        kind: kind_name(kind).to_owned(),
        pool: kind_pool(kind),
        identity: owner_declaration_identity(
            module_id,
            kind_name(kind),
            definition.span.start.0,
            definition.span.end.0,
            &definition.value,
        ),
        origin: SymbolOrigin::Module(module_id.clone()),
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
        origin: SymbolOrigin::Module(definition.module_id.clone()),
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
        identity: format!(
            "stillwind-identity/v1\0ref-marker\0{}\0{}:{}\0{}\0",
            definition.module_id,
            definition.binding.span.start.0,
            definition.binding.span.end.0,
            definition.binding.value,
        ),
        origin: SymbolOrigin::Module(definition.module_id.clone()),
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
    module_id: &ModuleId,
    name: &str,
    canonical_input: &str,
    span: Span,
) -> GeneratedSymbol {
    GeneratedSymbol {
        name: name.to_owned(),
        kind: "class".to_owned(),
        pool: IdentifierPool::ClassToken,
        identity: format!("stillwind-identity/v1\0class\0{canonical_input}"),
        origin: SymbolOrigin::Module(module_id.clone()),
        start: Some(span.start.0),
        end: Some(span.end.0),
    }
}

/// Generates one declared definition's name and reports it as a symbol.
///
/// `kind` selects the family the name belongs to, which is the kind the report carries.
/// Reported UTF-8 bounds address the original declaration in its identity's module.
///
/// # Errors
///
/// Returns the host generator's failure for the asset's family.
pub(super) fn definition_name_symbol(
    context: &IdentifierContext,
    reference: &AssetReference,
    kind: AssetKind,
) -> Result<GeneratedSymbol, CompileErrorKind> {
    let name = declared_name(context, kind, &identity_declaration(reference))?;
    Ok(GeneratedSymbol {
        name,
        kind: kind_name(kind).to_owned(),
        pool: kind_pool(kind),
        identity: identity_input(&reference.identity),
        origin: SymbolOrigin::Module(reference.identity.module_id.clone()),
        start: Some(reference.identity.start),
        end: Some(reference.identity.end),
    })
}

/// Owner identity of a declaration referenced through its semantic identity.
pub(super) fn identity_input(identity: &IdentityId) -> String {
    owner_declaration_identity(
        &identity.module_id,
        kind_name(identity.kind),
        identity.start,
        identity.end,
        &identity.binding_name,
    )
}

/// Owner identity of a ref declaration referenced through a locator.
///
/// The identity is what a symbol report compares, so it must not depend on the family
/// it was written for.
pub(super) fn identity_input_parts(context: &LocatorContext) -> String {
    owner_declaration_identity(
        &context.module,
        "ref",
        context.start,
        context.end,
        &context.binding,
    )
}

pub(super) fn ref_identity(definition: &RefDefinition) -> IdentityId {
    IdentityId::new(
        AssetKind::Ref,
        definition.module_id.clone(),
        definition.binding.span.start.0,
        definition.binding.span.end.0,
        definition.binding.value.clone(),
    )
}

/// Identity of a named motion asset, spanning its name token.
pub(super) fn named_identity(
    module_id: &ModuleId,
    kind: AssetKind,
    name: &Spanned<String>,
) -> IdentityId {
    IdentityId::new(
        kind,
        module_id.clone(),
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
    module_id: &ModuleId,
    span: Span,
) -> GeneratedSymbol {
    GeneratedSymbol {
        name: name.to_owned(),
        kind: kind.to_owned(),
        pool,
        identity,
        origin: SymbolOrigin::Module(module_id.clone()),
        start: Some(span.start.0),
        end: Some(span.end.0),
    }
}
#[cfg(test)]
mod tests {
    use stillwind_source::FileId;

    use super::*;
    use crate::identifiers::{
        DEFAULT_HASH_LENGTH, GeneratorSet, IdentifierContext, IdentifierMode, KeyframesNameRequest,
        Label, NameRequest, RefMarker,
    };

    fn identifiers() -> IdentifierContext {
        IdentifierContext::new(
            GeneratorSet::default(),
            IdentifierMode::Release,
            DEFAULT_HASH_LENGTH,
        )
    }

    #[test]
    fn identities_preserve_module_ids_and_class_names_are_content_stable() {
        let identifiers = identifiers();
        let class = |branch: &str| ClassContext {
            module: ModuleId::from("src/x.tsx"),
            namespace: String::new(),
            call: 0,
            branch: branch.to_owned(),
        };
        let name = Spanned::new("ref".to_owned(), Span::new(FileId(1), 2, 5));
        assert_ne!(
            owner_declaration_identity(&ModuleId::from(r"src\x.tsx"), "ref", 2, 5, &name.value),
            owner_declaration_identity(&ModuleId::from("src/x.tsx"), "ref", 2, 5, &name.value)
        );
        assert_ne!(
            owner_declaration_identity(&ModuleId::from("src/x.tsx"), "ref", 2, 5, &name.value),
            owner_declaration_identity(&ModuleId::from("src/y.tsx"), "ref", 2, 5, &name.value)
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
                .starts_with("sw_")
        );
        let namespaced = |namespace: &str| ClassContext {
            module: ModuleId::from("src/x.tsx"),
            namespace: namespace.to_owned(),
            call: 0,
            branch: "flex".to_owned(),
        };
        assert_eq!(
            identifiers.class_name(&namespaced("app")),
            identifiers.class_name(&namespaced("app"))
        );
        assert_ne!(
            identifiers.class_name(&namespaced("app")),
            identifiers.class_name(&namespaced("other"))
        );
    }

    #[test]
    fn owner_identities_share_one_declaration_spelling() {
        let module = ModuleId::from("src/x.tsx");
        let name = Spanned::new("item".to_owned(), Span::new(FileId(1), 2, 6));
        let identity = IdentityId::new(AssetKind::Ref, module.clone(), 2, 6, "item");
        let context = LocatorContext {
            module: module.clone(),
            start: 2,
            end: 6,
            binding: "item".to_owned(),
            label: None,
            projection: Some("container".to_owned()),
        };
        let expected = "stillwind-identity/v1\0src/x.tsx\0ref\x002:6\0item\0";
        assert_eq!(
            owner_declaration_identity(&module, "ref", 2, 6, &name.value),
            expected
        );
        assert_eq!(identity_input(&identity), expected);
        assert_eq!(identity_input_parts(&context), expected);
        let keyframes = IdentityId::new(AssetKind::Keyframes, module.clone(), 2, 6, "item");
        let request = KeyframesNameRequest(named_declaration(&module, &name, None));
        assert_eq!(
            request.identity().strip_prefix("stillwind/v1\0"),
            identity_input(&keyframes).strip_prefix("stillwind-identity/v1\0"),
        );
        assert_eq!(
            generated_class_symbol(&module, "sw_class", "class-input", name.span).identity,
            "stillwind-identity/v1\0class\0class-input"
        );
    }

    #[test]
    fn asset_names_distinguish_kinds_and_spans_and_keep_metadata() {
        let identifiers = identifiers();
        let name = Spanned::new("ref".into(), Span::new(FileId(1), 0, 3));
        for (kind, prefix) in [
            (AssetKind::Ref, "sw-a-"),
            (AssetKind::Keyframes, "sw-kf-"),
            (AssetKind::ViewTransition, "sw-vt-"),
        ] {
            let symbol = generated_symbol(
                &identifiers,
                &ModuleId::from("src/x.tsx"),
                kind,
                &name,
                None,
            )
            .unwrap();
            assert!(symbol.name.starts_with(prefix));
            assert_eq!(
                symbol.origin,
                SymbolOrigin::Module(ModuleId::from("src/x.tsx"))
            );
            assert_eq!((symbol.start, symbol.end), (Some(0), Some(3)));
            assert_eq!(
                symbol.identity,
                owner_declaration_identity(
                    &ModuleId::from("src/x.tsx"),
                    kind_name(kind),
                    0,
                    3,
                    &name.value
                )
            );
            let shifted = Spanned::new("ref".into(), Span::new(FileId(1), 1, 3));
            let shifted_name = declared_name(
                &identifiers,
                kind,
                &named_declaration(&ModuleId::from("src/x.tsx"), &shifted, None),
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
