//! Lowering of invalid input to empty or false meaning, empty assets, and omission
//! independent of lint reporting.

use std::collections::BTreeMap;
use std::sync::Arc;

use stillwind_compiler::{CompileResult, Compiler, CompilerOptions};
use stillwind_diagnostics::{
    CompileDiagnostic, LintDiagnostic, LintKind, LintOverrides, LintSeverity,
};
use stillwind_ir::{
    ModuleAnalysis, ScriptLanguage, ScriptModuleKind, SourceAnalyzer, StyleInput,
    composition_dependencies,
};
use stillwind_js_oxc::{OxcSourceAnalyzer, parse};
use stillwind_semantic::{AssetKind, IdentityId};
use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile};

fn compile(code: &str, css: &str, options: &CompilerOptions) -> (SourceFile, CompileResult) {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        format!("import {{style}} from 'stillwind/react'; {code}"),
    )
    .unwrap();
    let theme = SourceFile::new(
        FileId(2),
        None,
        None,
        format!("{css} @stillwind-slot utilities;"),
    )
    .unwrap();
    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &source,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("root.css"),
        options,
    );
    (source, result)
}

fn options(severity: LintSeverity) -> CompilerOptions {
    CompilerOptions {
        lint: LintOverrides {
            invalid_style_input: Some(severity),
            invalid_variant_input: Some(severity),
            invalid_ref_input: Some(severity),
            invalid_keyframes_definition: Some(severity),
            invalid_keyframes_use: Some(severity),
            invalid_keyframes_variant: Some(severity),
            invalid_view_transition_definition: Some(severity),
            invalid_view_transition_use: Some(severity),
            invalid_view_transition_variant: Some(severity),
            invalid_animation_option: Some(severity),
            invalid_cva_config: Some(severity),
            invalid_cva_input: Some(severity),
            invalid_style_composition: Some(severity),
            invalid_utility: Some(severity),
            unknown_utility: Some(severity),
            ..LintOverrides::default()
        },
        ..CompilerOptions::default()
    }
}

const CSS: &str =
    "@utility a {color:red;} @utility b {color:blue;} @variant hover {&:hover {@slot;}}";

#[test]
fn unsupported_operands_preserve_supported_siblings_at_every_reporting_level() {
    for severity in [LintSeverity::Deny, LintSeverity::Warn, LintSeverity::Off] {
        for input in [
            "42",
            "{title:'b'}",
            "{[key]:'b'}",
            "[...values]",
            "...values",
            "{...values}",
            "style('b')",
            "`${value}:b`",
            "{'unknown:':'b'}",
            "frames(42)",
            "frames()",
            "ref.asContainer('bad')",
            "ref.positionAnchor(42)",
            "ref.asAnchor('bad')",
            "{[ref.in()]: 'b'}",
            "{title:style('b')}",
            "[...style('b')]",
            "...style('b')",
            "{...style('b')}",
            "[1 + style('b')]",
            "{'hover:': {'data-ignored':style('b')} }",
        ] {
            let (_, result) = compile(
                &format!(
                    "const ref=style.ref(); const frames=style.keyframes({{}}); const x=style('a',{input});"
                ),
                CSS,
                &options(severity),
            );
            assert!(result.diagnostics.is_empty(), "{input}: {result:#?}");
            assert!(result.css.contains("color:red"), "{input}: {}", result.css);
            assert!(
                !result.css.contains("color:blue"),
                "{input}: {}",
                result.css
            );
            assert!(
                result
                    .lint_findings
                    .iter()
                    .filter(|finding| finding.kind() != LintKind::RequireRefAttachment)
                    .all(
                        |finding| result.lint_settings.reporting_severity(finding.kind())
                            == severity
                    ),
                "{input}: {result:#?}"
            );
        }
    }
    let (_, result) = compile(
        "// swlint-disable-next-line invalid-style-input\nconst x=style('a',{title:'b'});",
        CSS,
        &CompilerOptions::default(),
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    assert!(result.lint_findings.is_empty(), "{result:#?}");
    assert!(result.css.contains("color:red"), "{}", result.css);
    assert!(!result.css.contains("color:blue"), "{}", result.css);
}

#[test]
fn empty_keyframes_keep_the_binding_without_requiring_a_keyframes_slot() {
    for definition in [
        "{}",
        "{label:'empty'}",
        "{from:'missing'}",
        "42",
        "{from:42}",
    ] {
        let (source, result) = compile(
            &format!(
                "export const frames=style.keyframes({definition}); export const x=style(frames(''));"
            ),
            "",
            &CompilerOptions::default(),
        );
        assert!(result.diagnostics.is_empty(), "{definition}: {result:#?}");
        assert!(
            !result.css.contains("@keyframes"),
            "{definition}: {}",
            result.css
        );
        let code = result.apply_to_source(&source).unwrap();
        assert!(code.contains("export const frames="), "{code}");
        assert!(!code.contains("style.keyframes"), "{code}");
        assert!(
            result.css.contains("animation-name"),
            "{definition}: {}",
            result.css
        );
        if matches!(definition, "{}" | "{label:'empty'}") {
            assert!(result.lint_findings.is_empty(), "{result:#?}");
        } else {
            assert_eq!(result.lint_findings.len(), 1, "{result:#?}");
        }
    }
}

#[test]
fn empty_transitions_keep_keyed_use_semantics_without_definition_css() {
    for definition in ["{}", "{label:'empty'}", "{old:'missing'}", "42", "{old:42}"] {
        let (source, result) = compile(
            &format!(
                "export const transition=style.viewTransition({definition}); export const x=style(transition.for('pair'));"
            ),
            "",
            &CompilerOptions::default(),
        );
        assert!(result.diagnostics.is_empty(), "{definition}: {result:#?}");
        assert!(
            !result.css.contains("::view-transition"),
            "{definition}: {}",
            result.css
        );
        assert!(
            result.css.contains("view-transition-name"),
            "{definition}: {}",
            result.css
        );
        assert!(
            result.css.contains("view-transition-class"),
            "{definition}: {}",
            result.css
        );
        let code = result.apply_to_source(&source).unwrap();
        assert!(code.contains("export const transition="), "{code}");
        if matches!(definition, "{}" | "{label:'empty'}") {
            assert!(result.lint_findings.is_empty(), "{result:#?}");
        } else {
            assert_eq!(result.lint_findings.len(), 1, "{result:#?}");
        }
    }
}

#[test]
fn cva_removes_invalid_conditions_as_whole_compounds() {
    for compound in [
        "{unknown:'on',class:'b'}",
        "{[key]:'on',class:'b'}",
        "{tone:'missing',class:'b'}",
        "{...value,class:'b'}",
        "",
        ",",
        "...value",
    ] {
        let (source, result) = compile(
            &format!(
                "export const button=style.cva('a',{{variants:{{tone:{{on:42,off:'a'}}}},defaultVariants:{{unknown:'on'}},compoundVariants:[{compound},{{tone:'off',class:'a'}}]}});"
            ),
            CSS,
            &CompilerOptions::default(),
        );
        assert!(result.diagnostics.is_empty(), "{compound}: {result:#?}");
        assert!(result.css.contains("color:red"), "{}", result.css);
        assert!(!result.css.contains("color:blue"), "{}", result.css);
        let code = result.apply_to_source(&source).unwrap();
        assert!(!code.contains("class:'b'"), "{code}");
        assert!(!code.contains("on:42"), "{code}");
        let checked = SourceFile::new(FileId(3), None, None, code).unwrap();
        let parsed = parse(
            &checked,
            &ModuleId::from("output.tsx"),
            &[],
            &BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        assert!(parsed.diagnostics.is_empty(), "{parsed:#?}");
    }
}

#[test]
fn incompatible_composition_keeps_independent_rules() {
    let (_, result) = compile(
        "const base=style({'data-value':1}); const x=style(base,'a');",
        CSS,
        &CompilerOptions::default(),
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    assert!(
        result
            .lint_findings
            .iter()
            .any(|finding| finding.diagnostic == LintDiagnostic::StyleBaseWithProps),
        "{result:#?}"
    );
    assert!(result.css.contains("color:red"), "{}", result.css);
    assert!(result.composition.is_empty(), "{result:#?}");
}

#[test]
fn composition_cycles_remain_fatal_at_every_reporting_level() {
    let body =
        "const first=style(second,'a'); const second=style(first,'b'); const other=style(first);";
    for severity in [LintSeverity::Deny, LintSeverity::Warn, LintSeverity::Off] {
        for source in [
            body.to_owned(),
            format!("// swlint-disable invalid-style-composition\n{body}"),
        ] {
            let (_, result) = compile(&source, CSS, &options(severity));
            assert!(
                result
                    .diagnostics
                    .iter()
                    .any(|error| error.diagnostic == CompileDiagnostic::CompositionCycle),
                "{result:#?}"
            );
            assert!(result.transforms.is_empty(), "{result:#?}");
            assert!(result.css.is_empty(), "{result:#?}");
        }
    }
}

fn analyzed(module: &str, framework: &str) -> ModuleAnalysis {
    let source = Arc::new(SourceFile::new(FileId(1), None, None, format!(
        "import {{style}} from 'stillwind/{framework}'; const base=style('a'); const derived=style('b'); const local=style(base);"
    )).unwrap());
    OxcSourceAnalyzer.finalize(
        OxcSourceAnalyzer.scan(
            source,
            ModuleId::from(module),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Module,
        ),
        None,
    )
}

fn base_identity(module: &ModuleAnalysis) -> IdentityId {
    let call = &module.ir.style_calls[0];
    IdentityId::new(
        AssetKind::Style,
        module.module_id.clone(),
        call.span.start.0,
        call.span.end.0,
        "",
    )
}

fn compose_external(module: &mut ModuleAnalysis, base: IdentityId) {
    let call = &mut module.ir.style_calls[1];
    call.inputs.insert(
        0,
        StyleInput::Composition {
            value: call.span,
            candidates: vec![base],
        },
    );
}

#[test]
fn graph_cycles_remain_fatal_and_incompatible_framework_operands_are_omitted() {
    for framework in ["react", "vue"] {
        let mut first = analyzed("first.ts", "react");
        let mut second = analyzed("second.ts", framework);
        compose_external(&mut first, base_identity(&second));
        compose_external(&mut second, base_identity(&first));
        let lowered = ModuleAnalysis::attach_asset_support_batch(&[first, second]);
        for module in &lowered {
            if framework == "react" {
                assert!(
                    module
                        .diagnostics
                        .iter()
                        .any(|error| error.diagnostic == CompileDiagnostic::CompositionModuleCycle),
                    "{module:#?}"
                );
                assert_eq!(composition_dependencies(module).len(), 2);
            } else {
                assert!(module.diagnostics.is_empty(), "{module:#?}");
                assert_eq!(composition_dependencies(module).len(), 1);
                assert!(
                    module
                        .lint_findings
                        .iter()
                        .any(|finding| finding.diagnostic
                            == LintDiagnostic::StyleBaseFrameworkMismatch),
                    "{module:#?}"
                );
            }
            assert_eq!(module.ir.style_calls.len(), 3);
        }
    }
}

#[test]
fn known_unsupported_identities_are_omitted_but_missing_facts_remain_fatal() {
    for input in ["frames", "frames.asAnchor()", "ref.asAnchor([frames])"] {
        let (_, result) = compile(
            &format!(
                "const frames=style.keyframes({{}}); const ref=style.ref(); const x=style('a',{input});"
            ),
            CSS,
            &CompilerOptions::default(),
        );
        assert!(result.diagnostics.is_empty(), "{input}: {result:#?}");
        assert!(result.css.contains("color:red"), "{}", result.css);
        assert!(!result.lint_findings.is_empty(), "{input}");
    }
    for input in [
        "remote.asAnchor()",
        "remote('')",
        "ref.asAnchor([remote])",
        "{[remote('hover')]: 'b'}",
    ] {
        let (_, result) = compile(
            &format!(
                "import {{remote}} from './refs'; const ref=style.ref(); const x=style('a',{input});"
            ),
            CSS,
            &CompilerOptions::default(),
        );
        assert!(!result.diagnostics.is_empty(), "{input}");
        assert!(result.transforms.is_empty(), "{input}: {result:#?}");
    }
}

#[test]
fn composition_operands_with_mixed_bases_require_a_conditional_expression() {
    let mut first = analyzed("first.ts", "react");
    let second = analyzed("second.ts", "vue");
    let candidates = vec![base_identity(&first), base_identity(&second)];
    let call = &mut first.ir.style_calls[1];
    call.inputs.insert(
        0,
        StyleInput::Composition {
            value: call.span,
            candidates,
        },
    );
    let lowered = ModuleAnalysis::attach_asset_support_batch(&[first, second]);
    assert!(
        lowered[0]
            .diagnostics
            .iter()
            .any(|error| error.diagnostic == CompileDiagnostic::MixedStyleBaseCandidates),
        "{:#?}",
        lowered[0]
    );
    assert!(lowered[1].diagnostics.is_empty(), "{:#?}", lowered[1]);
}

#[test]
fn receivers_with_unsupported_branches_lower_one_use_per_branch() {
    for (input, lint) in [
        (
            "(enabled ? frames : 1)('1s')",
            LintDiagnostic::InvalidKeyframesReceiver,
        ),
        (
            "(enabled ? ref : 1).asAnchor()",
            LintDiagnostic::InvalidRefProjection,
        ),
        (
            "(enabled ? transition : 1).for('a')",
            LintDiagnostic::InvalidViewTransitionReceiver,
        ),
    ] {
        let (_, result) = compile(
            &format!(
                "declare const enabled: boolean; const frames=style.keyframes({{}}); const ref=style.ref(); const transition=style.viewTransition({{}}); const x=style('a',{input});"
            ),
            CSS,
            &CompilerOptions::default(),
        );
        assert!(result.diagnostics.is_empty(), "{input}: {result:#?}");
        assert!(
            result
                .lint_findings
                .iter()
                .any(|finding| finding.diagnostic == lint),
            "{input}: {result:#?}"
        );
        let replacement = result
            .transforms
            .iter()
            .map(|transform| transform.replacement.as_str())
            .collect::<String>();
        assert!(
            replacement.contains("enabled") && !replacement.contains(": 1"),
            "{input}: the runtime must test the condition and must not read the unsupported receiver: {replacement}"
        );
    }
    for input in [
        "(enabled && frames)('1s')",
        "(enabled ? ref : null).asAnchor()",
        "{[(enabled ? ref : 1)('hover:')]: 'b'}",
    ] {
        let (_, result) = compile(
            &format!(
                "declare const enabled: boolean; const frames=style.keyframes({{}}); const ref=style.ref(); const x=style('a',{input});"
            ),
            CSS,
            &CompilerOptions::default(),
        );
        assert!(result.diagnostics.is_empty(), "{input}: {result:#?}");
        assert!(result.css.contains("color:red"), "{input}: {}", result.css);
    }
}

#[test]
fn invalid_options_lower_as_omitted_arguments() {
    for (input, expected) in [
        ("ref.asContainer('bad')", "container-name"),
        ("ref.asAnchor('bad')", "anchor-name"),
        ("ref.positionAnchor(42)", "position-anchor"),
    ] {
        let (_, result) = compile(
            &format!("const ref=style.ref(); const x=style('a',{input});"),
            CSS,
            &CompilerOptions::default(),
        );
        assert!(result.diagnostics.is_empty(), "{input}: {result:#?}");
        let input_lints = result
            .lint_findings
            .iter()
            .filter(|finding| finding.kind() != LintKind::RequireRefAttachment)
            .count();
        assert_eq!(input_lints, 1, "{input}: {:#?}", result.lint_findings);
        assert!(result.css.contains(expected), "{input}: {}", result.css);
    }
}

#[test]
fn unknown_variants_share_the_utility_rules_wherever_they_appear() {
    for input in ["'unknown:a'", "{'unknown:': 'a'}"] {
        let (_, result) = compile(
            &format!("const x=style('b',{input});"),
            CSS,
            &CompilerOptions::default(),
        );
        assert!(result.diagnostics.is_empty(), "{input}: {result:#?}");
        assert!(
            result
                .lint_findings
                .iter()
                .any(|finding| finding.kind() == LintKind::UnknownUtility),
            "{input}: {result:#?}"
        );
        assert!(!result.css.contains("color:red"), "{input}: {}", result.css);
    }
}
