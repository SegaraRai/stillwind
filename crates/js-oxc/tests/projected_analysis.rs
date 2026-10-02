//! Projected-input tests: framework scaffolding removal and span identity.
//!
//! Each case builds a converter-shaped input (a shadow module plus a map) and
//! checks what the analyzer produces: which scaffolding was dropped, which spans
//! belong to the original file, and how converter configuration participates in
//! the revision.

use std::collections::BTreeMap;
use std::sync::Arc;

use stillwind_diagnostics::{CompileDiagnostic, LintDiagnostic};
use stillwind_ir::{
    AdapterKind, ClassExpression, ConverterIdentity, DirectSource, GroupEntry, ImportRealm,
    PreparedSource, ProjectedSource, RuleExpression, ScriptLanguage, ScriptModuleKind,
    ScriptSyntax, SourceAnalyzer, SourceDocument, SourceFormat, SourceInput, SourceInputError,
    SourceProjection, StyleInput, StyleVariant,
};
use stillwind_js_oxc::{OxcSourceAnalyzer, normalize_projection, parse};
use stillwind_semantic::{AssetKind, ExportedValue, SymbolicValue, TypeFact, TypeFacts};
use stillwind_source::{
    FileId, Fingerprint, ModuleId, ProjectionMap, SourceFile, Span, VolarMapping,
};

#[test]
fn checker_spans_restore_each_generated_receiver_for_properties_and_calls() {
    let original = SourceFile::new(FileId(1), None, None, "foo(); bar;").unwrap();
    let generated =
        SourceFile::new(FileId(2), None, None, "__VLS_ctx.foo(); __VLS_ctx.bar;").unwrap();
    let map = ProjectionMap::Volar {
        mappings: vec![VolarMapping {
            source_offsets: vec![0, 7],
            generated_offsets: vec![10, 27],
            lengths: vec![7, 4],
            generated_lengths: None,
        }],
    };
    let normalized = normalize_projection(
        &generated,
        &original,
        &map,
        AdapterKind::Vue,
        FileId(3),
        ScriptLanguage::Ts,
        ScriptModuleKind::Unambiguous,
    )
    .unwrap();
    assert_eq!(normalized.source.text(), original.text());
    for (start, end, expected) in [
        (0, 3, "__VLS_ctx.foo"),
        (0, 5, "__VLS_ctx.foo()"),
        (7, 10, "__VLS_ctx.bar"),
        (3, 5, "()"),
    ] {
        let mapped = normalized
            .original_span(Span::new(FileId(3), start, end))
            .unwrap();
        assert_eq!(mapped.original, Span::new(original.id(), start, end));
        assert_eq!(generated.slice(mapped.projected).unwrap(), expected);
    }
}

/// A Svelte-style projection of one script block into an original file.
///
/// The map is deliberately simple (one interval covering the script), so a failure
/// points at the analyzer rather than at the fixture's arithmetic.
fn projected() -> SourceInput {
    let script = "import {style} from 'stillwind/svelte'; export const target = style.ref('target'); const active: boolean = true; const props = style(target, active ? 'flex' : 'grid');";
    projected_script(script)
}

fn projected_script(script: &str) -> SourceInput {
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

fn rebuild_projection(
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
        "framework-import-mismatch"
    );
    assert_eq!(
        scan.source
            .slice(scan.lint_findings[0].labels[0].span)
            .unwrap(),
        "'stillwind/react'"
    );
}

#[test]
fn static_spreads_preserve_projected_raw_and_decoded_spans() {
    let input = projected_script(
        r"import {style} from 'stillwind/svelte'; const 前置 = '😀'; const props = style(
          [...[/*😀*/ 'fl\u0065x']],
          {...{[`data-項目`]: /* retained value */ 前置}});",
    );
    let original = Arc::clone(input.document().file());
    let analysis = OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan_input(input), None);
    assert!(
        analysis.diagnostics.is_empty(),
        "{:?}",
        analysis.diagnostics
    );
    let inputs = &analysis.ir.style_calls[0].inputs;
    let StyleInput::Class(ClassExpression::Array(values)) = &inputs[0] else {
        panic!("array");
    };
    let mut leaf = &values[0];
    while let ClassExpression::Array(nested) = leaf {
        assert_eq!(nested.len(), 1);
        leaf = &nested[0];
    }
    let ClassExpression::Rule(rule) = leaf else {
        panic!("rule");
    };
    let RuleExpression::Utilities(value) = rule.as_ref() else {
        panic!("utility");
    };
    assert_eq!(value.value, "flex");
    assert_eq!(value.span.file, original.id());
    assert_eq!(original.slice(value.span).unwrap(), r"fl\u0065x");
    let StyleInput::Class(ClassExpression::Group { entries, .. }) = &inputs[1] else {
        panic!("group");
    };
    let GroupEntry::Attribute(attribute) = &entries[0] else {
        panic!("attribute");
    };
    assert_eq!(attribute.name.value, "data-項目");
    assert_eq!(original.slice(attribute.value).unwrap(), "前置");
}

#[test]
fn animation_branch_spans_map_back_to_the_original_script() {
    let input = projected_script(
        "import {style} from 'stillwind/svelte'; const fade = style.keyframes({'motion-reduce:': {from:'opacity-0'}});",
    );
    let original = Arc::clone(input.document().file());
    let analysis = OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan_input(input), None);
    assert!(
        analysis.diagnostics.is_empty(),
        "{:?}",
        analysis.diagnostics
    );
    let branch = &analysis.ir.keyframes_declarations[0].body.branches[0];
    let StyleVariant::Prefix(prefix) = &branch.variant else {
        panic!("prefix");
    };
    assert_eq!(original.slice(prefix.span).unwrap(), "motion-reduce:");
    assert_eq!(
        original
            .slice(branch.body.frames[0].utilities.span)
            .unwrap(),
        "opacity-0"
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
        .find(|error| error.diagnostic == LintDiagnostic::MacroArgumentMutation)
        .unwrap();
    assert_eq!(error.labels[0].span.file, original.id());
    assert_eq!(original.slice(error.labels[0].span).unwrap(), "状態++");
}

/// Only scaffolding the map proves was inserted is discarded.
///
/// The same generated text is analysed twice, once where the original also contains the
/// scaffolding and once where it does not; authored scaffolding must survive, and the
/// reference count must reflect that difference.
#[test]
fn only_verified_unmapped_framework_scaffolding_is_discarded() {
    let script = "import {style} from 'stillwind/vue'; const props = style('flex');";
    let scaffold = "\ntype __VLS_SetupExposed = import('vue').ShallowUnwrapRef<{ style: typeof style; }>;\n// @ts-ignore\n[style,];\nconst __VLS_export = {};";
    let generated_text = format!("{script}{scaffold}");
    for authored in [false, true] {
        let original_text = if authored {
            generated_text.as_str()
        } else {
            script
        };
        let original = SourceFile::new(
            FileId(1),
            None,
            None,
            format!("<script setup>{original_text}</script>"),
        )
        .unwrap();
        let generated = SourceFile::new(FileId(2), None, None, generated_text.as_str()).unwrap();
        let map = ProjectionMap::Volar {
            mappings: vec![VolarMapping {
                source_offsets: vec![14],
                generated_offsets: vec![0],
                lengths: vec![u32::try_from(original_text.len()).unwrap()],
                generated_lengths: None,
            }],
        };
        let normalized = normalize_projection(
            &generated,
            &original,
            &map,
            AdapterKind::Vue,
            FileId(3),
            ScriptLanguage::Ts,
            ScriptModuleKind::Unambiguous,
        )
        .unwrap();
        let parsed = parse(
            &normalized.source,
            &ModuleId::from("test.tsx"),
            &[],
            &BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        assert!(parsed.diagnostics.is_empty());
        let (ir, _) = normalized.original_ir(&parsed.module).unwrap();
        let binding = &ir.imports.macros[0].specifiers[0];
        assert!(!binding.has_unmapped_references);
        assert_eq!(binding.references.len(), if authored { 3 } else { 1 });
        assert_eq!(normalized.source.text().contains("typeof style"), authored);
        assert_eq!(normalized.source.text().contains("[style,]"), authored);
    }
}

/// Converter-only reads are flagged instead of breaking macro extraction.
///
/// A reference that exists only in generated text cannot be mapped, so it is recorded
/// as unmapped on the specifier while the mapped references are kept.
#[test]
fn converter_only_references_prevent_unsafe_import_deletion_without_blocking_macros() {
    let input = projected();
    let code = Arc::new(
        SourceFile::new(
            input.parser_source().id(),
            None,
            None,
            format!(
                "{}\ntype Exposed = typeof style;",
                input.parser_source().text()
            ),
        )
        .unwrap(),
    );
    let input = rebuild_projection(
        &input,
        code,
        input.syntax(),
        input.projection().unwrap().map().clone(),
        input.projection().unwrap().converter().clone(),
    );
    let scan = OxcSourceAnalyzer.scan_input(input);
    assert!(scan.diagnostics.is_empty(), "{:?}", scan.diagnostics);
    assert_eq!(scan.local_ir.style_calls.len(), 1);
    let specifier = &scan.local_ir.imports.macros[0].specifiers[0];
    assert!(specifier.has_unmapped_references);
    assert_eq!(specifier.references.len(), 2);
    for reference in &specifier.references {
        assert_eq!(scan.source.slice(*reference).unwrap(), "style");
    }
}

/// Finalized projected analyses speak in original coordinates.
///
/// The source buffer is the original file, every summary span slices it, asset
/// identities and checker probes point into the original file.
#[test]
fn projected_analysis_preserves_original_source_and_semantic_identity() {
    let input = projected();
    let original = Arc::clone(input.document().file());
    let scan = OxcSourceAnalyzer.scan_input(input);
    assert!(scan.diagnostics.is_empty(), "{:?}", scan.diagnostics);
    assert!(Arc::ptr_eq(&scan.source, &original));
    assert!(!scan.provenance.is_empty());
    assert!(!scan.type_probes.is_empty());
    for probe in &scan.type_probes {
        assert_eq!(probe.span.file, original.id());
        assert!(original.slice(probe.span).is_ok());
    }
    for specifier in &scan.summary.module_specifiers {
        assert_eq!(
            &original.text()[specifier.start as usize..specifier.end as usize],
            "'stillwind/svelte'"
        );
    }
    let asset = scan
        .summary
        .assets
        .iter()
        .find(|asset| asset.identity.kind == AssetKind::Ref)
        .unwrap();
    assert_eq!(asset.identity.module_id, "Component.svelte".into());
    assert_eq!(
        &original.text()[asset.identity.start as usize..asset.identity.end as usize],
        "target"
    );
    assert_eq!(
        asset.identity.start,
        scan.local_ir.ref_definitions[0].binding.span.start.0
    );
    let analysis = OxcSourceAnalyzer.finalize(scan, None);
    assert_eq!(analysis.ir.adapter, AdapterKind::Svelte);
    assert!(analysis.source.text().starts_with("<script>"));
    assert_eq!(analysis.ir.style_calls.len(), 1);
    assert!(
        analysis
            .ir
            .style_calls
            .iter()
            .all(|call| call.span.file == original.id())
    );
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
            .any(|error| error.diagnostic == CompileDiagnostic::FrameworkConflict)
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
    assert!(scan.local_ir.style_calls.is_empty());
    assert!(scan.local_ir.ref_definitions.is_empty());
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
    assert!(scan.diagnostics.is_empty());
    assert!(Arc::ptr_eq(&scan.source, &source));
    assert_eq!(scan.local_ir.style_calls.len(), 1);
}

#[test]
fn projected_facts_resolve_composition_at_original_unicode_offsets() {
    let input = projected_script(
        "import {style} from 'stillwind/svelte'; const 前置='🧭'; const base=style('flex'); const chosen=base; const props=style(chosen);",
    );
    let scan = OxcSourceAnalyzer.scan_input(input);
    let identity = scan
        .summary
        .assets
        .iter()
        .find(|asset| asset.identity.kind == AssetKind::Style)
        .unwrap()
        .identity
        .clone();
    let probe = scan
        .type_probes
        .iter()
        .find(|probe| scan.source.slice(probe.span).unwrap() == "chosen")
        .unwrap()
        .span;
    let facts = TypeFacts {
        schema: TypeFacts::SCHEMA,
        module_id: scan.module_id.clone(),
        source_revision: scan.source_revision.clone(),
        semantic_revision: Fingerprint::of("checker"),
        assets: BTreeMap::new(),
        facts: vec![TypeFact {
            start: probe.start.0,
            end: probe.end.0,
            value: ExportedValue::Styles(SymbolicValue::exact(identity.clone())),
        }],
    };
    let analysis = OxcSourceAnalyzer.finalize(scan, Some(&facts));
    assert!(
        analysis.diagnostics.is_empty(),
        "{:?}",
        analysis.diagnostics
    );
    assert!(analysis.integration_diagnostics.is_empty());
    let StyleInput::Composition { value, candidates } = &analysis.ir.style_calls[1].inputs[0]
    else {
        panic!("composition")
    };
    assert_eq!(*value, probe);
    assert_eq!(candidates, std::slice::from_ref(&identity));
    assert_eq!(
        &analysis.source.text()[identity.start as usize..identity.end as usize],
        "style('flex')"
    );
    assert!(
        analysis
            .provenance
            .iter()
            .any(|mapping| mapping.original == probe)
    );
}

#[test]
fn projected_forward_ref_scopes_match_later_attachments_in_original_coordinates() {
    let input = projected_script(
        "// 日本語 🧭\n\
         import {style} from 'stillwind/svelte';\n\
         function props() { return style({[r('hover').in(s)]: 'flex'}); }\n\
         const r = style.ref('reference');\n\
         const s = style.ref('scope');\n\
         export const view = <section {...style(s)}><button {...style(r)} />\
         <span {...props()} /></section>;",
    );
    let input = rebuild_projection(
        &input,
        Arc::clone(input.parser_source()),
        ScriptSyntax::new(ScriptLanguage::Tsx, input.syntax().module_kind()),
        input.projection().unwrap().map().clone(),
        input.projection().unwrap().converter().clone(),
    );
    let scan = OxcSourceAnalyzer.scan_input(input);
    let refs = scan
        .summary
        .assets
        .iter()
        .filter(|asset| asset.identity.kind == AssetKind::Ref)
        .map(|asset| (asset.identity.binding_name.as_str(), asset))
        .collect::<BTreeMap<_, _>>();
    let facts = TypeFacts {
        schema: TypeFacts::SCHEMA,
        module_id: scan.module_id.clone(),
        source_revision: scan.source_revision.clone(),
        semantic_revision: Fingerprint::of("forward refs"),
        assets: refs
            .values()
            .map(|asset| (asset.identity.clone(), asset.metadata.clone()))
            .collect(),
        facts: scan
            .type_probes
            .iter()
            .filter_map(|probe| {
                let name = scan.source.slice(probe.span).unwrap();
                let asset = refs.get(name)?;
                Some(TypeFact {
                    start: probe.span.start.0,
                    end: probe.span.end.0,
                    value: ExportedValue::Refs(SymbolicValue::exact(asset.identity.clone())),
                })
            })
            .collect(),
    };
    let analysis = OxcSourceAnalyzer.finalize(scan, Some(&facts));
    assert!(analysis.integration_diagnostics.is_empty());
    assert!(
        analysis.diagnostics.is_empty(),
        "{:?}",
        analysis.diagnostics
    );
    assert!(
        analysis.lint_findings.is_empty(),
        "{:?}",
        analysis.lint_findings
    );
    let StyleInput::Class(ClassExpression::Group { entries, .. }) =
        &analysis.ir.style_calls[0].inputs[0]
    else {
        panic!("selector group");
    };
    let GroupEntry::Class(expression) = &entries[0] else {
        panic!("selector rule");
    };
    let ClassExpression::Rule(rule) = expression.as_ref() else {
        panic!("selector variant");
    };
    let RuleExpression::Variant {
        variant: StyleVariant::Ref { condition, scopes },
        ..
    } = rule.as_ref()
    else {
        panic!("ref variant");
    };
    for (definition, call_index) in [(&scopes[0].refs[0], 1), (&condition.refs[0], 2)] {
        let StyleInput::RefAttachment {
            definition: attached,
            ..
        } = &analysis.ir.style_calls[call_index].inputs[0]
        else {
            panic!("ref attachment");
        };
        assert_eq!(definition.binding, attached.binding);
        assert_eq!(definition.module_id, attached.module_id);
        assert_eq!(definition.binding.span.file, analysis.source.id());
        assert_eq!(
            analysis.source.slice(definition.binding.span).unwrap(),
            definition.binding.value
        );
    }
}
