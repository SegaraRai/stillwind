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

use stillwind_ir::{HandleDefinition, HandleProjection, ScopeDefinition};
use stillwind_semantic::{AssetKind, IdentityId};
use stillwind_source::{SourceFile, Span, Spanned, normalized_module_path};

use super::handles::{handle_marker_class, scope_marker_class};

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
    handle: &HandleDefinition,
    projection: &HandleProjection,
) -> String {
    let suffix = match projection {
        HandleProjection::Anchor => "anchor",
        HandleProjection::Position => "position",
        HandleProjection::Scope => "scope",
        HandleProjection::Container(kind) => kind.value.as_str(),
    };
    format!("_swp_{}_{suffix}", handle_marker_class(handle))
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
pub(super) fn ref_symbol_name(definition: &HandleDefinition) -> String {
    let identity = asset_identity(&definition.module_path, "ref", &definition.name);
    format!(
        "--sw-r-{}",
        &blake3::hash(identity.as_bytes()).to_hex()[..8]
    )
}

fn kind_name(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Style => "style",
        AssetKind::Ref => "ref",
        AssetKind::Scope => "scope",
        AssetKind::Animation => "animation",
        AssetKind::ViewTransition => "view-transition",
    }
}

/// Generated name of a class or motion asset, prefix selected by kind.
pub(super) fn symbol_name(
    source: &SourceFile,
    kind: AssetKind,
    definition: &Spanned<String>,
) -> String {
    asset_symbol_name(source.path(), kind, definition)
}

/// Generated name of a class or motion asset, prefixed by kind so declarations of
/// different kinds at the same span cannot collide.
pub(super) fn asset_symbol_name(
    module_path: &str,
    kind: AssetKind,
    definition: &Spanned<String>,
) -> String {
    let prefix = match kind {
        AssetKind::Style => "_s_",
        AssetKind::Ref => "--sw-r-",
        AssetKind::Scope => "--sw-s-",
        AssetKind::Animation => "--sw-a-",
        AssetKind::ViewTransition => "--sw-v-",
    };
    let identity = asset_identity(module_path, kind_name(kind), definition);
    let hash = blake3::hash(identity.as_bytes());
    format!("{prefix}{}", &hash.to_hex()[..8])
}

/// Builds the reportable symbol for a named class or motion declaration.
pub(super) fn generated_symbol(
    source: &SourceFile,
    kind: AssetKind,
    definition: &Spanned<String>,
) -> GeneratedSymbol {
    GeneratedSymbol {
        name: symbol_name(source, kind, definition),
        kind: kind_name(kind).to_owned(),
        identity: symbol_identity(source, kind_name(kind), definition),
        source_path: normalized_module_path(source.path()),
        start: definition.span.start.0,
        end: definition.span.end.0,
    }
}

/// Builds the reportable symbol for a ref declaration's projection name.
pub(super) fn generated_ref_symbol(definition: &HandleDefinition) -> GeneratedSymbol {
    let identity = asset_identity(&definition.module_path, "ref", &definition.name);
    GeneratedSymbol {
        name: ref_symbol_name(definition),
        kind: "ref".to_owned(),
        identity,
        source_path: definition.module_path.clone(),
        start: definition.name.span.start.0,
        end: definition.name.span.end.0,
    }
}

/// Builds the reportable symbol for a scope declaration's projection name.
pub(super) fn generated_scope_symbol(definition: &ScopeDefinition) -> GeneratedSymbol {
    let identity = asset_identity(&definition.module_path, "scope", &definition.name);
    GeneratedSymbol {
        name: format!(
            "--sw-s-{}",
            &blake3::hash(identity.as_bytes()).to_hex()[..8]
        ),
        kind: "scope".to_owned(),
        identity,
        source_path: definition.module_path.clone(),
        start: definition.name.span.start.0,
        end: definition.name.span.end.0,
    }
}

/// Builds the reportable symbol for a ref's marker class.
pub(super) fn generated_ref_marker_symbol(definition: &HandleDefinition) -> GeneratedSymbol {
    GeneratedSymbol {
        name: handle_marker_class(definition),
        kind: "ref-marker".to_owned(),
        identity: asset_identity(&definition.module_path, "ref-marker", &definition.name),
        source_path: definition.module_path.clone(),
        start: definition.name.span.start.0,
        end: definition.name.span.end.0,
    }
}

/// Builds the reportable symbol for a scope's marker class.
pub(super) fn generated_scope_marker_symbol(definition: &ScopeDefinition) -> GeneratedSymbol {
    GeneratedSymbol {
        name: scope_marker_class(definition)
            .trim_start_matches('.')
            .to_owned(),
        kind: "scope-marker".to_owned(),
        identity: asset_identity(&definition.module_path, "scope-marker", &definition.name),
        source_path: definition.module_path.clone(),
        start: definition.name.span.start.0,
        end: definition.name.span.end.0,
    }
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
        identity: format!("stillwind/v1\0class\0{canonical_input}"),
        source_path: normalized_module_path(source.path()),
        start: span.start.0,
        end: span.end.0,
    }
}

/// Class unit name for a canonical input.
pub(crate) fn class_name(canonical_input: &str) -> String {
    let hash = blake3::hash(canonical_input.as_bytes());
    format!("_s_{}", &hash.to_hex()[..8])
}

/// Uses declaration identity directly, without fabricating a source-buffer span.
pub(super) fn motion_symbol_name(identity: &IdentityId) -> String {
    let prefix = match identity.kind {
        AssetKind::Animation => "--sw-a-",
        AssetKind::ViewTransition => "--sw-v-",
        AssetKind::Style => "_s_",
        AssetKind::Ref => "--sw-r-",
        AssetKind::Scope => "--sw-s-",
    };
    let content = format!(
        "stillwind/v1\0{}\0{}\0{}:{}:{}",
        normalized_module_path(&identity.module_path),
        kind_name(identity.kind),
        identity.start,
        identity.end,
        identity.binding_name
    );
    format!(
        "{prefix}{}",
        &blake3::hash(content.as_bytes()).to_hex()[..8]
    )
}

#[cfg(test)]
mod tests {
    use stillwind_source::FileId;

    use super::*;

    #[test]
    fn identities_normalize_paths_and_class_names_are_content_stable() {
        let name = Spanned::new("ref".into(), Span::new(FileId(1), 2, 5));
        assert_eq!(
            asset_identity(r"src\x.tsx", "ref", &name),
            asset_identity("src/x.tsx", "ref", &name)
        );
        assert_ne!(
            asset_identity("src/x.tsx", "ref", &name),
            asset_identity("src/y.tsx", "ref", &name)
        );
        assert_eq!(class_name("flex"), class_name("flex"));
        assert_ne!(class_name("flex"), class_name("grid"));
        assert!(class_name("flex").starts_with("_s_"));
    }

    #[test]
    fn asset_names_distinguish_kinds_and_spans_and_keep_metadata() {
        let source = SourceFile::new(FileId(1), r"src\x.tsx", "ref").unwrap();
        let name = Spanned::new("ref".into(), Span::new(FileId(1), 0, 3));
        for (kind, prefix) in [
            (AssetKind::Ref, "--sw-r-"),
            (AssetKind::Scope, "--sw-s-"),
            (AssetKind::Animation, "--sw-a-"),
            (AssetKind::ViewTransition, "--sw-v-"),
        ] {
            let symbol = generated_symbol(&source, kind, &name);
            assert!(symbol.name.starts_with(prefix));
            assert_eq!(symbol.source_path, "src/x.tsx");
            assert_eq!((symbol.start, symbol.end), (0, 3));
            assert_eq!(
                symbol.identity,
                asset_identity(source.path(), kind_name(kind), &name)
            );
            assert_ne!(
                symbol.name,
                asset_symbol_name(
                    source.path(),
                    kind,
                    &Spanned::new("ref".into(), Span::new(FileId(1), 1, 3))
                )
            );
        }
    }
}
