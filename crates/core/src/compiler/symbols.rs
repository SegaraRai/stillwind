use super::{
    GeneratedSymbol, HandleProjection, SourceFile, Span, Spanned, handle_marker_class,
    normalized_module_path, scope_marker_class,
};

pub(super) fn handle_projection_name(projection: &HandleProjection) -> &'static str {
    match projection {
        HandleProjection::Anchor => "anchor",
        HandleProjection::Position => "position",
        HandleProjection::Scope => "scope",
        HandleProjection::Container(_) => "container",
    }
}

pub(super) fn runtime_projection_class(
    handle: &stillwind_ir::HandleDefinition,
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

pub(super) fn ref_symbol_name(definition: &stillwind_ir::HandleDefinition) -> String {
    let identity = asset_identity(&definition.module_path, "ref", &definition.name);
    format!(
        "--sw-r-{}",
        &blake3::hash(identity.as_bytes()).to_hex()[..8]
    )
}

pub(super) fn symbol_name(source: &SourceFile, kind: &str, definition: &Spanned<String>) -> String {
    asset_symbol_name(source.path(), kind, definition)
}

pub(super) fn asset_symbol_name(
    module_path: &str,
    kind: &str,
    definition: &Spanned<String>,
) -> String {
    let prefix = match kind {
        "ref" => "--sw-r-",
        "scope" => "--sw-s-",
        "animation" => "--sw-a-",
        "view-transition" => "--sw-v-",
        _ => unreachable!("only named Stillwind asset kinds are allocated"),
    };
    let identity = asset_identity(module_path, kind, definition);
    let hash = blake3::hash(identity.as_bytes());
    format!("{prefix}{}", &hash.to_hex()[..8])
}

pub(super) fn generated_symbol(
    source: &SourceFile,
    kind: &str,
    definition: &Spanned<String>,
) -> GeneratedSymbol {
    GeneratedSymbol {
        name: symbol_name(source, kind, definition),
        kind: kind.to_owned(),
        identity: symbol_identity(source, kind, definition),
        source_path: normalized_module_path(source.path()),
        start: definition.span.start.0,
        end: definition.span.end.0,
    }
}

pub(super) fn generated_ref_symbol(definition: &stillwind_ir::HandleDefinition) -> GeneratedSymbol {
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

pub(super) fn generated_scope_symbol(
    definition: &stillwind_ir::ScopeDefinition,
) -> GeneratedSymbol {
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

pub(super) fn generated_ref_marker_symbol(
    definition: &stillwind_ir::HandleDefinition,
) -> GeneratedSymbol {
    GeneratedSymbol {
        name: handle_marker_class(definition),
        kind: "ref-marker".to_owned(),
        identity: asset_identity(&definition.module_path, "ref-marker", &definition.name),
        source_path: definition.module_path.clone(),
        start: definition.name.span.start.0,
        end: definition.name.span.end.0,
    }
}

pub(super) fn generated_scope_marker_symbol(
    definition: &stillwind_ir::ScopeDefinition,
) -> GeneratedSymbol {
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
