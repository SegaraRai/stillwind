//! Checks normalization and original correspondence for generated expressions.

use super::*;

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
        let span = Span::new(FileId(3), start, end);
        let mapped = if expected == "()" {
            normalized.original_span(span)
        } else {
            normalized
                .checker_span(span, CheckerRole::Expression)
                .map(|checked| checked.provenance)
        }
        .unwrap();
        assert_eq!(mapped.original, Span::new(original.id(), start, end));
        assert_eq!(generated.slice(mapped.projected).unwrap(), expected);
    }
}

#[test]
fn nested_unwrap_and_inserted_value_retain_the_authored_expression() {
    let original_text = "style(active ? 'flex' : 'grid', {class: external})";
    let generated_text = "__VLS_unwrap(style, {} as import('vue').Ref<unknown>)(active.value ? 'flex' : 'grid', {class: __VLS_unwrap(external, {} as import('vue').Ref<unknown>)} )".replace("} )", "})");
    let original = SourceFile::new(FileId(1), None, None, original_text).unwrap();
    let generated = SourceFile::new(FileId(2), None, None, generated_text.as_str()).unwrap();
    let mut mappings = Vec::new();
    for piece in [
        "style",
        "(active",
        " ? 'flex' : 'grid', {class: ",
        "external",
        "})",
    ] {
        mappings.push(VolarMapping {
            source_offsets: vec![u32::try_from(original_text.find(piece).unwrap()).unwrap()],
            generated_offsets: vec![u32::try_from(generated_text.find(piece).unwrap()).unwrap()],
            lengths: vec![u32::try_from(piece.len()).unwrap()],
            generated_lengths: None,
        });
    }
    let normalized = normalize_projection(
        &generated,
        &original,
        &ProjectionMap::Volar { mappings },
        AdapterKind::Vue,
        FileId(3),
        ScriptLanguage::Ts,
        ScriptModuleKind::Unambiguous,
    )
    .unwrap();
    assert_eq!(normalized.source.text(), original_text);
    let checked = normalized
        .checker_span(normalized.source.full_span(), CheckerRole::Producer)
        .unwrap();
    assert_eq!(checked.provenance.projected, generated.full_span());
}

#[test]
fn qualified_names_and_authored_value_accesses_share_mapping_validation() {
    for authored in [false, true] {
        let original_text = if authored {
            "typeof state.value"
        } else {
            "typeof state"
        };
        let generated_text = if authored {
            "typeof state.value.value"
        } else {
            "typeof state.value"
        };
        let mut mappings = vec![VolarMapping {
            source_offsets: vec![0],
            generated_offsets: vec![0],
            lengths: vec![12],
            generated_lengths: None,
        }];
        if authored {
            mappings.push(VolarMapping {
                source_offsets: vec![12],
                generated_offsets: vec![18],
                lengths: vec![6],
                generated_lengths: None,
            });
        }
        let original =
            SourceFile::new(FileId(1), None, None, format!("type T = {original_text};")).unwrap();
        let generated =
            SourceFile::new(FileId(2), None, None, format!("type T = {generated_text};")).unwrap();
        for mapping in &mut mappings {
            mapping.source_offsets[0] += 9;
            mapping.generated_offsets[0] += 9;
        }
        let normalized = normalize_projection(
            &generated,
            &original,
            &ProjectionMap::Volar { mappings },
            AdapterKind::Vue,
            FileId(3),
            ScriptLanguage::Ts,
            ScriptModuleKind::Unambiguous,
        )
        .unwrap();
        assert_eq!(normalized.source.text(), original.text());
    }
}

#[test]
fn expanded_shorthand_queries_the_value_expression() {
    let original = SourceFile::new(FileId(1), None, None, "({value})").unwrap();
    let generated = SourceFile::new(
        FileId(2),
        None,
        None,
        "({value: __VLS_unwrap(value, {} as import('vue').Ref<unknown>)})",
    )
    .unwrap();
    let value = u32::try_from(generated.text().find("(value,").unwrap() + 1).unwrap();
    let end = u32::try_from(generated.text().len()).unwrap();
    let map = ProjectionMap::Volar {
        mappings: vec![VolarMapping {
            source_offsets: vec![0, 2, 2, 7],
            generated_offsets: vec![0, 2, value, end - 2],
            lengths: vec![2, 5, 5, 2],
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
    let checked = normalized
        .checker_span(Span::new(FileId(3), 2, 7), CheckerRole::Expression)
        .unwrap();
    assert_eq!(
        generated.slice(checked.provenance.projected).unwrap(),
        "__VLS_unwrap(value, {} as import('vue').Ref<unknown>)"
    );
}

#[test]
fn constructor_parentheses_follow_the_map_even_when_generated_text_is_identical() {
    let generated_text = "new (__VLS_unwrap(Factory, {} as import('vue').Ref<unknown>))()";
    let generated = SourceFile::new(FileId(2), None, None, generated_text).unwrap();
    let factory = u32::try_from(generated_text.find("Factory").unwrap()).unwrap();
    let close = u32::try_from(generated_text.len() - 3).unwrap();
    for authored in [false, true] {
        let original_text = if authored {
            "new (Factory)()"
        } else {
            "new Factory()"
        };
        let original = SourceFile::new(FileId(1), None, None, original_text).unwrap();
        let map = ProjectionMap::Volar {
            mappings: vec![VolarMapping {
                source_offsets: vec![
                    0,
                    if authored { 5 } else { 4 },
                    if authored { 12 } else { 11 },
                ],
                generated_offsets: vec![0, factory, if authored { close } else { close + 1 }],
                lengths: vec![
                    if authored { 5 } else { 4 },
                    7,
                    if authored { 3 } else { 2 },
                ],
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
        assert_eq!(normalized.source.text(), original_text);
    }
}

#[test]
fn conflicting_wrapper_points_do_not_authorize_a_rewrite() {
    let original = SourceFile::new(FileId(1), None, None, "value; foreign;").unwrap();
    let generated = SourceFile::new(FileId(2), None, None, "__VLS_ctx.value;").unwrap();
    let map = ProjectionMap::Volar {
        mappings: vec![VolarMapping {
            source_offsets: vec![0, 7, 0],
            generated_offsets: vec![0, 0, 10],
            lengths: vec![0, 0, 6],
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
    assert_eq!(normalized.source.text(), generated.text());
    assert!(
        normalized
            .original_span(normalized.source.full_span())
            .is_err()
    );
}

#[test]
fn static_spreads_preserve_projected_raw_and_decoded_spans() {
    let input = projected_script(
        r"import {style} from 'stillwind/svelte'; const 前置 = '😀'; const props = style(
          [...[/*😀*/ 'fl\u0065x']],
          {...{[`--項目`]: /* retained value */ 前置}});",
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
    let GroupEntry::CustomProperty(property) = &entries[0] else {
        panic!("custom property");
    };
    assert_eq!(property.name.value, "--項目");
    assert_eq!(original.slice(property.value).unwrap(), "前置");
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

/// Only references proven to come from converter scaffolding are ignored.
///
/// The same generated text is analysed twice, once where the original also contains the
/// scaffolding and once where it does not; authored scaffolding must survive, and the
/// reference count must reflect that difference.
#[test]
fn only_verified_synthetic_references_are_ignored() {
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
        assert_eq!(parsed.diagnostics, [] as [CompileError; 0]);
        let (ir, _) = normalized.original_ir(&parsed.module).unwrap();
        let binding = &ir.imports.macros[0].specifiers[0];
        assert!(!binding.has_unmapped_references);
        assert_eq!(binding.references.len(), if authored { 3 } else { 1 });
    }
}

#[test]
fn generated_exposure_reads_and_nested_completion_reads_keep_import_guards_precise() {
    let script = "import {style} from 'stillwind/vue'; const props = style('flex');";
    let scaffolds = [
        (
            "type __VLS_Bindings = __VLS_ProxyRefs<{style:typeof style}>;",
            false,
        ),
        (
            "type __VLS_SetupExposed = __VLS_ProxyRefs<{style:typeof style}>;",
            false,
        ),
        (
            "const __VLS_self = {setup(){return {style:style as typeof style};}};",
            false,
        ),
        (
            "const __VLS_self = {setup:()=>({style:style as typeof style})};",
            false,
        ),
        ("function completion(){\n// @ts-ignore\n[style,];\n}", false),
        (
            "__VLS_withDotValue(style, {} as import('vue').Ref<unknown>);",
            false,
        ),
        ("const __VLS_self = {setup(){return {leak:style};}};", true),
    ];
    for (scaffold, guarded) in scaffolds {
        let original = SourceFile::new(FileId(1), None, None, script).unwrap();
        let generated =
            SourceFile::new(FileId(2), None, None, format!("{script}\n{scaffold}")).unwrap();
        let map = ProjectionMap::Volar {
            mappings: vec![VolarMapping {
                source_offsets: vec![0],
                generated_offsets: vec![0],
                lengths: vec![u32::try_from(script.len()).unwrap()],
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
            &ModuleId::from("exposure.vue"),
            &[],
            &BTreeMap::new(),
            ScriptLanguage::Ts,
            ScriptModuleKind::Unambiguous,
        );
        let (ir, _) = normalized.original_ir(&parsed.module).unwrap();
        let binding = &ir.imports.macros[0].specifiers[0];
        assert_eq!(binding.has_unmapped_references, guarded, "{scaffold}");
        assert_eq!(binding.references.len(), 1, "{scaffold}");
    }
}

#[test]
fn converter_only_references_reject_the_scan_before_checker_queries() {
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
    let error = &scan.diagnostics[0];
    assert!(matches!(
        error.diagnostic,
        CompileDiagnostic::UnmappedMacroReference { .. }
    ));
    assert_eq!(error.labels[0].scope, LabelScope::Document);
    assert_eq!(scan.source.slice(error.labels[1].span).unwrap(), "style");
    assert_eq!(scan.local_ir.style_calls, [] as [StyleCall; 0]);
    assert!(scan.type_probes.is_empty());
}

#[test]
fn generated_syntax_reports_a_verified_original_range() {
    let scan = OxcSourceAnalyzer.scan_input(projected_script("const = ;"));
    let error = &scan.diagnostics[0];
    assert!(matches!(
        error.diagnostic,
        CompileDiagnostic::ProjectionSyntax { .. }
    ));
    assert_eq!(error.labels[0].scope, LabelScope::Range);
    assert_eq!(scan.source.slice(error.labels[0].span).unwrap(), "=");
    assert_eq!(scan.local_ir.style_calls, [] as [StyleCall; 0]);
    assert!(scan.type_probes.is_empty());
}
