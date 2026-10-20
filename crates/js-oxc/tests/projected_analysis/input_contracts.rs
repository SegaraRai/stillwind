//! Checks projection metadata, source revisions, and preparation contracts.

use stillwind_diagnostics::LintDiagnostic;

use super::*;

#[test]
fn original_component_format_reports_import_mismatch_without_adapter_conflict() {
    let mut input =
        projected_script("import { style } from 'stillwind/react'; const props = style('flex');");
    input.framework = None;
    input = rebuild_projection(
        &input,
        Arc::clone(input.parser_source()),
        ScriptSyntax::new(ScriptLanguage::Tsx, input.syntax().module_kind()),
        input.projection().unwrap().map().clone(),
        input.projection().unwrap().converter().clone(),
    );
    let scan = OxcSourceAnalyzer.scan_input(input);
    assert_eq!(scan.diagnostics, []);
    assert_eq!(scan.local_ir.adapter, AdapterKind::React);
    assert_eq!(scan.lint_findings.len(), 1);
    assert_eq!(
        scan.lint_findings[0].diagnostic.code(),
        "mismatched-framework-import"
    );
    assert_eq!(
        scan.source
            .slice(scan.lint_findings[0].labels[0].span)
            .unwrap(),
        "'stillwind/react'"
    );
}

#[test]
fn contract_findings_map_to_original_framework_expression_spans() {
    let input = projected_script(
        "import {style} from 'stillwind/svelte'; const 状態 = 1; const props = style({'--x': 状態++});",
    );
    let original = Arc::clone(input.document().file());
    let analysis = OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan_input(input), None);
    let error = analysis
        .lint_findings
        .iter()
        .find(|error| error.diagnostic == LintDiagnostic::MutatingMacroArgument)
        .unwrap();
    assert_eq!(error.labels[0].span.file, original.id());
    assert_eq!(original.slice(error.labels[0].span).unwrap(), "状態++");
}

/// Converter configuration is part of the input revision, and failures are atomic.
///
/// Changing the options fingerprint or the framework changes the revision, a
/// conflicting framework is reported, and an unusable map yields a diagnostic scan with
/// no partial IR.
#[test]
fn projection_failures_are_atomic_and_configuration_is_part_of_the_revision() {
    let input = projected();
    let revision = input.revision();
    let mut converter = input.projection().unwrap().converter().clone();
    converter.options_fingerprint = "different".into();
    let changed = rebuild_projection(
        &input,
        Arc::clone(input.parser_source()),
        input.syntax(),
        input.projection().unwrap().map().clone(),
        converter,
    );
    assert_ne!(revision, changed.revision());
    let mut changed = input.clone();
    changed.framework = Some(AdapterKind::Vue);
    assert_ne!(revision, changed.revision());
    assert!(
        OxcSourceAnalyzer
            .scan_input(changed)
            .diagnostics
            .iter()
            .any(|error| error.diagnostic == CompileDiagnostic::FrameworkConstraintConflict)
    );
    let changed = rebuild_projection(
        &input,
        Arc::clone(input.parser_source()),
        input.syntax(),
        ProjectionMap::Volar {
            mappings: Vec::new(),
        },
        input.projection().unwrap().converter().clone(),
    );
    let scan = OxcSourceAnalyzer.scan_input(changed);
    assert!(matches!(
        scan.diagnostics[0].diagnostic,
        CompileDiagnostic::ProjectionUnmapped { .. }
    ));
    assert_eq!(scan.local_ir.style_calls, [] as [StyleCall; 0]);
    assert_eq!(scan.local_ir.ref_definitions, [] as [RefDefinition; 0]);
}

#[test]
fn incomplete_converter_identities_fail_during_projection_creation() {
    let input = projected();
    let projection = input.projection().unwrap();
    for field in ["name", "version", "options_fingerprint"] {
        for blank in ["", " \t"] {
            let mut converter = projection.converter().clone();
            match field {
                "name" => converter.name = blank.into(),
                "version" => converter.version = blank.into(),
                _ => converter.options_fingerprint = blank.into(),
            }
            let error = SourceProjection::try_new(
                Arc::clone(projection.code()),
                projection.syntax(),
                projection.map().clone(),
                converter,
                projection.analysis_id(),
                projection.import_realms().to_vec(),
                projection.editable_regions().to_vec(),
            )
            .unwrap_err();
            assert!(
                matches!(error, SourceInputError::IncompleteConverterIdentity { field: rejected } if rejected == field)
            );
        }
    }
}

#[test]
fn conflicting_analysis_file_ids_fail_during_source_creation() {
    let input = projected();
    let projection = input.projection().unwrap();
    let error = SourceProjection::try_new(
        Arc::clone(projection.code()),
        projection.syntax(),
        projection.map().clone(),
        projection.converter().clone(),
        projection.code().id(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap_err();
    assert!(
        matches!(error, SourceInputError::DuplicateFileId { id } if id == projection.code().id())
    );
    let projection = SourceProjection::try_new(
        Arc::clone(projection.code()),
        projection.syntax(),
        projection.map().clone(),
        projection.converter().clone(),
        input.document().file().id(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let error = ProjectedSource::try_new(input.document().clone(), projection).unwrap_err();
    assert!(
        matches!(error, SourceInputError::DuplicateFileId { id } if id == input.document().file().id())
    );
}

#[test]
fn source_constructors_reject_component_direct_and_script_projected_inputs() {
    let input = projected();
    assert!(matches!(
        DirectSource::try_new(input.document().clone(), input.syntax()),
        Err(SourceInputError::InvalidDirectFormat {
            format: SourceFormat::Svelte
        })
    ));
    let document = SourceDocument::new(
        Arc::clone(input.document().file()),
        SourceFormat::Script,
        None,
    );
    assert!(matches!(
        ProjectedSource::try_new(document, input.projection().unwrap().clone()),
        Err(SourceInputError::InvalidProjectedFormat {
            format: SourceFormat::Script
        })
    ));
}

#[test]
fn parent_import_realms_can_occupy_separate_source_ranges() {
    let input = projected();
    let projection = input.projection().unwrap();
    let file_id = input.document().file().id();
    let projection = SourceProjection::try_new(
        Arc::clone(projection.code()),
        projection.syntax(),
        projection.map().clone(),
        projection.converter().clone(),
        projection.analysis_id(),
        vec![
            ImportRealm {
                id: 1,
                source: Span::new(file_id, 8, 14),
                parent: None,
            },
            ImportRealm {
                id: 2,
                source: Span::new(file_id, 15, 20),
                parent: Some(1),
            },
        ],
        Vec::new(),
    )
    .unwrap();
    let projected = ProjectedSource::try_new(input.document().clone(), projection).unwrap();
    let scan = OxcSourceAnalyzer.scan_input(SourceInput::new(
        input.module_id,
        PreparedSource::Projected(projected),
    ));
    assert!(scan.diagnostics.is_empty(), "{:?}", scan.diagnostics);
    assert_eq!(scan.local_ir.style_calls.len(), 1);
}

/// An upstream map without a projection does not change the analysed source.
///
/// `upstream_map` is host metadata for other tools; parsing and the edit target must
/// stay tied to the input `SourceFile`.
#[test]
fn normal_input_map_does_not_change_the_edit_target_or_parser_language() {
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            None,
            None,
            "import {style} from 'stillwind/react'; const props: object = style('flex');",
        )
        .unwrap(),
    );
    let direct = DirectSource::try_new(
        SourceDocument::new(
            Arc::clone(&source),
            SourceFormat::Script,
            Some("{\"version\":3}".into()),
        ),
        ScriptSyntax::new(ScriptLanguage::Ts, ScriptModuleKind::Unambiguous),
    )
    .unwrap();
    let input = SourceInput::new(ModuleId::from("test.tsx"), PreparedSource::Direct(direct));
    let scan = OxcSourceAnalyzer.scan_input(input);
    assert_eq!(scan.diagnostics, [] as [CompileError; 0]);
    assert!(Arc::ptr_eq(&scan.source, &source));
    assert_eq!(scan.local_ir.style_calls.len(), 1);
}

#[test]
fn lint_directives_come_from_projected_script_comments_at_original_offsets() {
    let script = "import {style} from 'svelte';
// swlint-disable-next-line no-important
const props = style('[display:block]!');";
    let input = projected_script_after("<p>Don't</p>\n", script);
    let text = input.document().file().text().to_owned();
    let scan = OxcSourceAnalyzer.scan_input(input);
    assert_eq!(scan.lint_directives.len(), 1, "{:#?}", scan.lint_directives);
    let directive = &scan.lint_directives[0];
    assert_eq!(directive.line, 2);
    assert_eq!(
        &text[directive.offset..directive.end],
        "swlint-disable-next-line"
    );
    let (name, offset) = &directive.names[0];
    assert_eq!(name, "no-important");
    assert_eq!(&text[*offset..*offset + name.len()], "no-important");
}
