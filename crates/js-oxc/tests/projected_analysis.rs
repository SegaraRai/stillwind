//! Projected-input tests: framework scaffolding removal and span identity.
//!
//! Each case builds a converter-shaped input (a shadow module plus a map) and
//! checks what the analyzer produces: which scaffolding was dropped, which spans
//! belong to the original file, and how converter configuration participates in
//! the revision.

use std::sync::Arc;

use stillwind_diagnostics::{CompileErrorKind, LintKind};
use stillwind_ir::{
    AdapterKind, ClassExpression, ConverterIdentity, FrameworkProjection, GroupEntry,
    ScriptLanguage, SourceAnalyzer, SourceInput, StyleInput, StyleVariant,
};
use stillwind_js_oxc::{OxcSourceAnalyzer, normalize_projection, parse};
use stillwind_semantic::AssetKind;
use stillwind_source::{FileId, ProjectionMap, SourceFile, VolarMapping};

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
            "Component.svelte",
            format!("<script>{script}</script><div />"),
        )
        .unwrap(),
    );
    SourceInput {
        code: Arc::new(SourceFile::new(FileId(2), "Component.svelte", script).unwrap()),
        language: ScriptLanguage::Ts,
        framework: Some(AdapterKind::Svelte),
        upstream_map: None,
        stylesheet_imports: Vec::new(),
        module_css_import: None,
        projection: Some(FrameworkProjection {
            original,
            map: ProjectionMap::Volar {
                mappings: vec![VolarMapping {
                    source_offsets: vec![8],
                    generated_offsets: vec![0],
                    lengths: vec![u32::try_from(script.encode_utf16().count()).unwrap()],
                    generated_lengths: None,
                }],
            },
            converter: ConverterIdentity {
                name: "svelte2tsx".into(),
                version: "0.7.61".into(),
                options_fingerprint: "test".into(),
            },
            analysis_id: FileId(3),
            realms: Vec::new(),
            regions: Vec::new(),
        }),
    }
}

#[test]
fn static_spreads_preserve_projected_raw_and_decoded_spans() {
    let input = projected_script(
        r"import {style} from 'stillwind/svelte'; const 前置 = '😀'; const props = style(
          [...[/*😀*/ 'fl\u0065x']],
          {...{[`data-項目`]: /* retained value */ 前置}});",
    );
    let original = input.projection.as_ref().unwrap().original.clone();
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
    let ClassExpression::Utilities(value) = leaf else {
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
    let original = input.projection.as_ref().unwrap().original.clone();
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
    let original = input.projection.as_ref().unwrap().original.clone();
    let analysis = OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan_input(input), None);
    let error = analysis
        .lint_findings
        .iter()
        .find(|error| error.kind == LintKind::MacroArgumentMutation)
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
            "Component.vue",
            format!("<script setup>{original_text}</script>"),
        )
        .unwrap();
        let generated =
            SourceFile::new(FileId(2), "Component.ts", generated_text.as_str()).unwrap();
        let map = ProjectionMap::Volar {
            mappings: vec![VolarMapping {
                source_offsets: vec![14],
                generated_offsets: vec![0],
                lengths: vec![u32::try_from(original_text.len()).unwrap()],
                generated_lengths: None,
            }],
        };
        let normalized =
            normalize_projection(&generated, &original, &map, AdapterKind::Vue, FileId(3)).unwrap();
        let parsed = parse(&normalized.source);
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
    let mut input = projected();
    input.code = Arc::new(
        SourceFile::new(
            input.code.id(),
            input.code.path(),
            format!("{}\ntype Exposed = typeof style;", input.code.text()),
        )
        .unwrap(),
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
/// identities point at declarations in it, and checker probes are empty because their
/// spans would describe the generated module.
#[test]
fn projected_analysis_preserves_original_source_and_semantic_identity() {
    let input = projected();
    let original = Arc::clone(input.edit_source());
    let scan = OxcSourceAnalyzer.scan_input(input);
    assert!(scan.diagnostics.is_empty(), "{:?}", scan.diagnostics);
    assert!(Arc::ptr_eq(&scan.source, &original));
    assert!(!scan.provenance.is_empty());
    assert!(scan.type_probes.is_empty());
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
    assert_eq!(asset.identity.module_path, "Component.svelte");
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
    let mut changed = input.clone();
    changed
        .projection
        .as_mut()
        .unwrap()
        .converter
        .options_fingerprint = "different".into();
    assert_ne!(revision, changed.revision());
    let mut changed = input.clone();
    changed.framework = Some(AdapterKind::Vue);
    assert_ne!(revision, changed.revision());
    assert!(
        OxcSourceAnalyzer
            .scan_input(changed)
            .diagnostics
            .iter()
            .any(|error| error.kind == CompileErrorKind::FrameworkConflict)
    );
    let mut changed = input;
    changed.projection.as_mut().unwrap().map = ProjectionMap::Volar {
        mappings: Vec::new(),
    };
    let scan = OxcSourceAnalyzer.scan_input(changed);
    assert!(matches!(
        scan.diagnostics[0].kind,
        CompileErrorKind::ProjectionUnmapped { .. }
    ));
    assert!(scan.local_ir.style_calls.is_empty());
    assert!(scan.local_ir.ref_definitions.is_empty());
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
            "opaque.extension",
            "import {style} from 'stillwind/react'; const props: object = style('flex');",
        )
        .unwrap(),
    );
    let mut input = SourceInput::normal(Arc::clone(&source));
    input.language = ScriptLanguage::Ts;
    input.upstream_map = Some("{\"version\":3}".into());
    let scan = OxcSourceAnalyzer.scan_input(input);
    assert!(scan.diagnostics.is_empty());
    assert!(Arc::ptr_eq(&scan.source, &source));
    assert_eq!(scan.local_ir.style_calls.len(), 1);
}
