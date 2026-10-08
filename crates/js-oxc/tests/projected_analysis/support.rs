//! Shared source builders and result helpers for these contracts.

use super::*;

/// A Svelte-style projection of one script block into an original file.
///
/// The map is deliberately simple (one interval covering the script), so a failure
/// points at the analyzer rather than at the fixture's arithmetic.
pub(super) fn projected() -> SourceInput {
    let script = "import {style} from 'stillwind/svelte'; export const target = style.ref('target'); const active: boolean = true; const props = style(target, active ? 'flex' : 'grid');";
    projected_script(script)
}

pub(super) fn projected_script(script: &str) -> SourceInput {
    let original = Arc::new(
        SourceFile::new(
            FileId(1),
            None,
            None,
            format!("<script>{script}</script><div />"),
        )
        .unwrap(),
    );
    let projection = SourceProjection::try_new(
        Arc::new(SourceFile::new(FileId(2), None, None, script).unwrap()),
        ScriptSyntax::new(ScriptLanguage::Ts, ScriptModuleKind::Unambiguous),
        ProjectionMap::Volar {
            mappings: vec![VolarMapping {
                source_offsets: vec![8],
                generated_offsets: vec![0],
                lengths: vec![u32::try_from(script.encode_utf16().count()).unwrap()],
                generated_lengths: None,
            }],
        },
        ConverterIdentity {
            name: "svelte2tsx".into(),
            version: "0.7.61".into(),
            options_fingerprint: "test".into(),
        },
        FileId(3),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let prepared = ProjectedSource::try_new(
        SourceDocument::new(original, SourceFormat::Svelte, None),
        projection,
    )
    .unwrap();
    let mut input = SourceInput::new(
        ModuleId::from("Component.svelte"),
        PreparedSource::Projected(prepared),
    );
    input.framework = Some(AdapterKind::Svelte);
    input
}

pub(super) fn rebuild_projection(
    input: &SourceInput,
    code: Arc<SourceFile>,
    syntax: ScriptSyntax,
    map: ProjectionMap,
    converter: ConverterIdentity,
) -> SourceInput {
    let projection = input.projection().unwrap();
    let projection = SourceProjection::try_new(
        code,
        syntax,
        map,
        converter,
        projection.analysis_id(),
        projection.import_realms().to_vec(),
        projection.editable_regions().to_vec(),
    )
    .unwrap();
    let projected = ProjectedSource::try_new(input.document().clone(), projection).unwrap();
    let mut rebuilt = SourceInput::new(
        input.module_id.clone(),
        PreparedSource::Projected(projected),
    );
    rebuilt.framework = input.framework;
    rebuilt
        .stylesheet_imports
        .clone_from(&input.stylesheet_imports);
    rebuilt
        .module_css_import
        .clone_from(&input.module_css_import);
    rebuilt
}
