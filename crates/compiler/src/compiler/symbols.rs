use serde::Serialize;
use stillwind_ir::{HandleDefinition, HandleProjection, ScopeDefinition};
use stillwind_semantic::AssetKind;
use stillwind_source::{SourceFile, Span, Spanned, normalized_module_path};

use super::handles::{handle_marker_class, scope_marker_class};

/// A generated CSS identifier with the canonical identity used to allocate it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeneratedSymbol {
    pub name: String,
    pub kind: String,
    pub identity: String,
    pub source_path: String,
    pub start: u32,
    pub end: u32,
}

pub(super) fn handle_projection_name(projection: &HandleProjection) -> &'static str {
    match projection {
        HandleProjection::Anchor => "anchor",
        HandleProjection::Position => "position",
        HandleProjection::Scope => "scope",
        HandleProjection::Container(_) => "container",
    }
}

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

pub(super) fn ref_symbol_name(definition: &HandleDefinition) -> String {
    let identity = asset_identity(&definition.module_path, "ref", &definition.name);
    format!(
        "--sw-r-{}",
        &blake3::hash(identity.as_bytes()).to_hex()[..8]
    )
}

fn kind_name(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Ref => "ref",
        AssetKind::Scope => "scope",
        AssetKind::Animation => "animation",
        AssetKind::ViewTransition => "view-transition",
    }
}

pub(super) fn symbol_name(
    source: &SourceFile,
    kind: AssetKind,
    definition: &Spanned<String>,
) -> String {
    asset_symbol_name(source.path(), kind, definition)
}

pub(super) fn asset_symbol_name(
    module_path: &str,
    kind: AssetKind,
    definition: &Spanned<String>,
) -> String {
    let prefix = match kind {
        AssetKind::Ref => "--sw-r-",
        AssetKind::Scope => "--sw-s-",
        AssetKind::Animation => "--sw-a-",
        AssetKind::ViewTransition => "--sw-v-",
    };
    let identity = asset_identity(module_path, kind_name(kind), definition);
    let hash = blake3::hash(identity.as_bytes());
    format!("{prefix}{}", &hash.to_hex()[..8])
}

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

pub(crate) fn class_name(canonical_input: &str) -> String {
    let hash = blake3::hash(canonical_input.as_bytes());
    format!("_s_{}", &hash.to_hex()[..8])
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
