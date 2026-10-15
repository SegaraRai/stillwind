use std::sync::Arc;

use lean_string::LeanStr;

use stillwind_source::{FileId, ProjectionMap, SourceFile, Span};

use super::*;
use crate::AdapterKind;

fn file(id: u32, text: &str) -> Arc<SourceFile> {
    Arc::new(SourceFile::new(FileId(id), None, None, text).unwrap())
}

fn syntax() -> ScriptSyntax {
    ScriptSyntax::new(ScriptLanguage::Ts, ScriptModuleKind::Module)
}

fn converter() -> ConverterIdentity {
    ConverterIdentity {
        name: "converter".into(),
        version: "1".into(),
        options_fingerprint: "options".into(),
    }
}

fn realm(id: u32, start: u32, end: u32, parent: Option<u32>) -> ImportRealm {
    ImportRealm {
        id,
        source: Span::new(FileId(1), start, end),
        parent,
    }
}

fn region(id: u32, start: u32, end: u32) -> ProjectionRegion {
    ProjectionRegion {
        source: Span::new(FileId(1), start, end),
        import_realm: id,
        attribute_quote: None,
    }
}

fn projection(
    realms: Vec<ImportRealm>,
    regions: Vec<ProjectionRegion>,
) -> Result<SourceProjection, SourceInputError> {
    SourceProjection::try_new(
        file(2, "const x = 1;"),
        syntax(),
        ProjectionMap::Volar {
            mappings: Vec::new(),
        },
        converter(),
        FileId(3),
        realms,
        regions,
    )
}

fn projected(
    document: Arc<SourceFile>,
    realms: Vec<ImportRealm>,
    regions: Vec<ProjectionRegion>,
) -> Result<ProjectedSource, SourceInputError> {
    ProjectedSource::try_new(
        SourceDocument::new(document, SourceFormat::Svelte, None),
        projection(realms, regions)?,
    )
}

struct DocumentFixture {
    file: Arc<SourceFile>,
    format: SourceFormat,
    upstream_map: Option<LeanStr>,
}

struct ProjectionFixture {
    code: Arc<SourceFile>,
    syntax: ScriptSyntax,
    map: ProjectionMap,
    converter: ConverterIdentity,
    analysis_id: FileId,
    import_realms: Vec<ImportRealm>,
    editable_regions: Vec<ProjectionRegion>,
}

fn assert_revision_change(
    input: &SourceInput,
    change: impl FnOnce(&mut DocumentFixture, &mut ProjectionFixture),
) {
    let original = input.document();
    let prepared = input.projection().unwrap();
    let mut document = DocumentFixture {
        file: original.file().clone(),
        format: original.format(),
        upstream_map: original.upstream_map().map(LeanStr::from),
    };
    let mut projection = ProjectionFixture {
        code: prepared.code().clone(),
        syntax: prepared.syntax(),
        map: prepared.map().clone(),
        converter: prepared.converter().clone(),
        analysis_id: prepared.analysis_id(),
        import_realms: prepared.import_realms().to_vec(),
        editable_regions: prepared.editable_regions().to_vec(),
    };
    change(&mut document, &mut projection);
    let document = SourceDocument::new(document.file, document.format, document.upstream_map);
    let projection = SourceProjection::try_new(
        projection.code,
        projection.syntax,
        projection.map,
        projection.converter,
        projection.analysis_id,
        projection.import_realms,
        projection.editable_regions,
    )
    .unwrap();
    let changed = SourceInput::new(
        input.module_id.clone(),
        PreparedSource::Projected(ProjectedSource::try_new(document, projection).unwrap()),
    );
    assert_ne!(input.revision(), changed.revision());
}

#[test]
fn revision_includes_every_source_and_projection_semantic_field() {
    let input = SourceInput::new(
        "module".into(),
        PreparedSource::Projected(
            projected(
                file(1, "module instance template"),
                vec![realm(1, 0, 6, None), realm(2, 7, 15, Some(1))],
                vec![region(2, 16, 24)],
            )
            .unwrap(),
        ),
    );
    assert_eq!(input.revision(), input.clone().revision());
    assert_revision_change(&input, |doc, _| {
        doc.file = file(1, "MODULE instance template");
    });
    assert_revision_change(&input, |doc, _| doc.format = SourceFormat::Vue);
    assert_revision_change(&input, |doc, _| doc.upstream_map = Some("map".into()));
    assert_revision_change(&input, |_, projection| {
        projection.code = file(2, "other code");
    });
    assert_revision_change(&input, |_, projection| {
        projection.code = file(4, "const x = 1;");
    });
    assert_revision_change(&input, |_, projection| projection.analysis_id = FileId(4));
    assert_revision_change(&input, |_, projection| {
        projection.syntax = ScriptSyntax::new(ScriptLanguage::Tsx, ScriptModuleKind::Module);
    });
    assert_revision_change(&input, |_, projection| {
        projection.syntax = ScriptSyntax::new(ScriptLanguage::Ts, ScriptModuleKind::Unambiguous);
    });
    assert_revision_change(&input, |_, projection| {
        projection.map = ProjectionMap::V3 {
            json: "map".into(),
            source: "original".into(),
        }
    });
    assert_revision_change(&input, |_, projection| {
        projection.converter.name = "other".into();
    });
    assert_revision_change(&input, |_, projection| {
        projection.converter.version = "2".into();
    });
    assert_revision_change(&input, |_, projection| {
        projection.converter.options_fingerprint = "other".into();
    });
    assert_revision_change(&input, |_, projection| {
        projection.import_realms[0].source.end.0 = 5;
    });
    assert_revision_change(&input, |_, projection| {
        projection.import_realms[1].parent = None;
    });
    assert_revision_change(&input, |_, projection| projection.import_realms.reverse());
    assert_revision_change(&input, |_, projection| {
        projection.editable_regions[0].source.start.0 = 17;
    });
    assert_revision_change(&input, |_, projection| {
        projection.editable_regions[0].import_realm = 1;
    });
    assert_revision_change(&input, |_, projection| {
        projection.editable_regions[0].attribute_quote = Some(AttributeQuote::Single);
    });
    assert_revision_change(&input, |_, projection| projection.editable_regions.clear());
    assert_revision_change(&input, |_, projection| {
        projection.import_realms[0].id = 9;
        projection.import_realms[1].parent = Some(9);
    });
    for change in [
        |input: &mut SourceInput| input.module_id = "other module".into(),
        |input: &mut SourceInput| input.framework = Some(AdapterKind::Vue),
        |input: &mut SourceInput| input.stylesheet_imports.push("module.css".into()),
        |input: &mut SourceInput| input.module_css_import = Some("module-css.js".into()),
    ] {
        let mut changed = input.clone();
        change(&mut changed);
        assert_ne!(input.revision(), changed.revision());
    }
}
