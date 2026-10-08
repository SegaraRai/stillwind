//! Checks semantic identities and facts at original document coordinates.

use super::*;

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
    assert_ne!(scan.provenance, [] as [SourceProvenance; 0]);
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
    assert_eq!(
        analysis.integration_diagnostics,
        [] as [IntegrationDiagnostic; 0]
    );
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
    assert_eq!(
        analysis.integration_diagnostics,
        [] as [IntegrationDiagnostic; 0]
    );
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
