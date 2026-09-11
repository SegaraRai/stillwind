use std::sync::Arc;

use stillwind_ir::{
    AdapterKind, ConverterIdentity, FrameworkProjection, ScriptLanguage, SourceAnalyzer,
    SourceInput,
};
use stillwind_js_oxc::{OxcSourceAnalyzer, normalize_projection, parse};
use stillwind_semantic::AssetKind;
use stillwind_source::{FileId, ProjectionMap, SourceFile, VolarMapping};

fn projected() -> SourceInput {
    let script = "import {style} from 'stillwind/svelte'; export const target = style.ref('target'); const active: boolean = true; const props = style(target, active ? 'flex' : 'grid');";
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
                    lengths: vec![u32::try_from(script.len()).unwrap()],
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
        scan.local_ir.handle_definitions[0].name.span.start.0
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
            .any(|error| error.code == "SHJS008")
    );
    let mut changed = input;
    changed.projection.as_mut().unwrap().map = ProjectionMap::Volar {
        mappings: Vec::new(),
    };
    let scan = OxcSourceAnalyzer.scan_input(changed);
    assert_eq!(scan.diagnostics[0].code, "SHPRJ001");
    assert!(scan.local_ir.style_calls.is_empty());
    assert!(scan.local_ir.handle_definitions.is_empty());
}

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
