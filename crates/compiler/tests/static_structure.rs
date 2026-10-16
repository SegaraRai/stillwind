//! Literal structure normalization through the public compiler.

use stillwind_compiler::{CompileResult, Compiler, CompilerOptions, SourceTransform};
use stillwind_diagnostics::{
    CompileDiagnostic, LintDiagnostic, LintKind, LintOverrides, LintSeverity,
    RuleGroupInputErrorKind,
};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile};

fn compile(code: &str, css: &str) -> (SourceFile, CompileResult) {
    compile_with_options(code, css, &CompilerOptions::default())
}

fn compile_with_options(
    code: &str,
    css: &str,
    options: &CompilerOptions,
) -> (SourceFile, CompileResult) {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        format!("import {{ style }} from 'stillwind/react'; {code}"),
    )
    .unwrap();
    let theme = SourceFile::new(
        FileId(2),
        None,
        None,
        format!(
            "{css} @stillwind-slot utilities; {}",
            if css.contains("@stillwind-slot keyframes;") {
                ""
            } else {
                "@stillwind-slot keyframes;"
            }
        ),
    )
    .unwrap();
    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &source,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        options,
    );
    (source, result)
}

#[test]
fn empty_rule_group_keys_retain_enclosing_variants_and_entry_order() {
    let css = "@utility a { color:red; } @utility b { margin:2px; } \
               @variant hover { &:hover { @slot; } }";
    for (plain, grouped) in [
        ("'a'", "{'': 'a'}"),
        ("'a'", "{':': 'a'}"),
        ("'hover:a'", "{'hover:': {'': 'a'}}"),
        ("'hover:a'", "{'hover:': {':': 'a'}}"),
        ("'hover:a'", "{'': {'hover:': 'a'}}"),
        ("'a b'", "{'': 'a', ':': 'b'}"),
        ("'b a'", "{':': 'b', '': 'a'}"),
        ("'b'", "{...{'': 'a'}, '': 'b'}"),
    ] {
        let (_, expected) = compile(&format!("const x = style({plain});"), css);
        let (_, result) = compile(&format!("const x = style({grouped});"), css);
        assert!(result.diagnostics.is_empty(), "{grouped}: {result:#?}");
        assert!(result.lint_findings.is_empty(), "{grouped}: {result:#?}");
        assert_eq!(result.css, expected.css, "{grouped}");
    }
}

#[test]
fn literal_spreads_and_fixed_keys_preserve_object_overwrites() {
    let css = "@utility a { color:red; } @utility b { color:blue; }";
    for input in ["[,...['a',, ...['b']]]", "...['a',, ...['b']]"] {
        let (_, result) = compile(&format!("export const x = style({input});"), css);
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        assert!(
            result.css.ends_with("{color:red;color:blue;}"),
            "{}",
            result.css
        );
    }
    let (_, result) = compile(
        "export const x = style({ ...{class: 'discarded', 'data-項目': 'old'}, \
         [`data-項目`]: 'new', ['class']: 'kept', \
         ...{ 'hover:': 'a', ...{['hover:']: 'b'} } });",
        &format!("{css} @variant hover {{ &:hover {{ @slot; }} }}"),
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    assert!(result.lint_findings.is_empty(), "{result:#?}");
    assert!(result.css.contains("color:blue;"), "{}", result.css);
    assert!(!result.css.contains("color:red;"), "{}", result.css);
    let code = &result.transforms[0].replacement;
    assert!(!code.contains("discarded"), "{code}");
    assert!(!code.contains("'old'"), "{code}");
    assert!(code.contains("'new'") && code.contains("'kept'"), "{code}");
}

#[test]
fn inline_custom_properties_combine_with_variant_utilities() {
    let css = "@utility a { color:red; } @utility b { color:blue; } \
               @variant hover { &:hover { @slot; } } \
               @variant focus { &:focus { @slot; } }";
    for value in ["'red'", "0", "dynamicValue"] {
        let (_, result) = compile(
            &format!(
                "const x = style({{'hover:': ['a', {{'--x': {value}}}], \
                     'focus:': 'b', 'data-kept': 1}});"
            ),
            css,
        );
        assert!(
            result.lint_findings.iter().any(|finding| finding.diagnostic
                == LintDiagnostic::InvalidRuleGroupInput {
                    reason: RuleGroupInputErrorKind::CustomProperty,
                }),
            "{result:#?}"
        );
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        assert!(result.css.contains(":focus{color:blue;}"), "{}", result.css);
        assert!(!result.css.contains("color:red;"), "{}", result.css);
        let output = &result.transforms[0].replacement;
        assert!(output.contains("data-kept"), "{output}");
        assert!(!output.contains("--x"), "{output}");
    }
    let (_, result) = compile(
        "const x = style('hover:[--x:var(--y)]', {'--y': dynamicValue});",
        css,
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    assert!(result.css.contains("--x:var(--y)"), "{}", result.css);
    assert!(!result.css.contains("dynamicValue"));
    assert!(result.transforms[0].replacement.contains("dynamicValue"));
}

#[test]
fn malformed_utility_requirement_keeps_its_definition_cause() {
    let (_, result) = compile(
        "const x = style('faulty');",
        "@utility faulty { @stillwind-require-properties red; color:red; }",
    );
    assert!(
        result.diagnostics.iter().any(|error| matches!(
            &error.diagnostic,
            CompileDiagnostic::MalformedUtilityRequirement { utility, offset: 0 }
                if utility == "faulty"
        )),
        "{result:#?}"
    );
    assert_eq!(result.transforms, [] as [SourceTransform; 0]);
}

#[test]
fn nested_variant_failure_labels_the_first_incompatible_leaf() {
    for (input, reason, label) in [
        (
            "const x = style({'hover:': [flag && {'aria-busy': true}]});",
            RuleGroupInputErrorKind::Attribute,
            "aria-busy",
        ),
        (
            "const x = style({'hover:': [{class: external}]});",
            RuleGroupInputErrorKind::ExternalClass,
            "external",
        ),
    ] {
        let (source, result) = compile(input, "@variant hover { &:hover { @slot; } }");
        let failure = result
            .lint_findings
            .iter()
            .find(|finding| finding.diagnostic == LintDiagnostic::InvalidRuleGroupInput { reason })
            .unwrap();
        assert_eq!(source.slice(failure.labels[0].span).unwrap(), label);
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        assert_ne!(result.transforms, [] as [SourceTransform; 0]);
    }
}

#[test]
fn invalid_rule_groups_are_omitted_independently_of_lint_reporting() {
    let css = "@utility a { color:red; } @utility b { color:blue; } \
               @variant hover { &:hover { @slot; } }";
    for key in ["hover:", "", ":"] {
        let code = format!(
            "const x = style({{'{key}': ['a', {{'data-dropped': value}}], \
                           'data-kept': 1}}, 'b');"
        );
        for severity in [LintSeverity::Deny, LintSeverity::Warn, LintSeverity::Off] {
            let options = CompilerOptions {
                lint: LintOverrides {
                    invalid_rule_group_input: Some(severity),
                    ..LintOverrides::default()
                },
                ..CompilerOptions::default()
            };
            let (_, result) = compile_with_options(&code, css, &options);
            assert!(result.diagnostics.is_empty(), "{result:#?}");
            assert_eq!(result.lint_findings.len(), 1);
            assert_eq!(
                result
                    .lint_settings
                    .reporting_severity(LintKind::InvalidRuleGroupInput),
                severity
            );
            assert!(result.css.contains("color:blue;"), "{}", result.css);
            assert!(!result.css.contains("color:red;"), "{}", result.css);
            let output = &result.transforms[0].replacement;
            assert!(output.contains("data-kept"), "{output}");
            assert!(
                !output.contains("data-dropped") && !output.contains("value"),
                "{output}"
            );
        }
        let (_, suppressed) = compile(
            &format!("\n// swlint-disable-next-line invalid-rule-group-input\n{code}"),
            css,
        );
        assert!(suppressed.diagnostics.is_empty(), "{suppressed:#?}");
        assert!(suppressed.lint_findings.is_empty(), "{suppressed:#?}");
        assert!(
            !suppressed.transforms[0]
                .replacement
                .contains("data-dropped")
        );
    }
}

#[test]
fn opaque_spreads_and_dynamic_keys_remain_diagnostics() {
    for (input, expected) in [
        ("[...values]", LintDiagnostic::StyleArraySpread),
        ("...values", LintDiagnostic::StyleArgumentSpread),
        ("{...values}", LintDiagnostic::AttributeSpread),
        ("{[key]: 'a'}", LintDiagnostic::InvalidComputedAttribute),
    ] {
        let (_, result) = compile(&format!("const x = style({input});"), "");
        assert!(
            result
                .lint_findings
                .iter()
                .any(|error| error.diagnostic == expected),
            "{result:#?}"
        );
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        assert_eq!(result.transforms[0].replacement, "{}");
    }
}

#[test]
fn spread_values_keep_original_source_mappings() {
    let (source, result) = compile(
        "const x = style({...{['data-項目']: label, [`--値`]: amount}}, [...['a']]);",
        "@utility a { color:red; }",
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    for value in ["label", "amount"] {
        assert!(
            result.transforms.iter().any(|edit| edit
                .expressions
                .iter()
                .any(|mapping| { source.slice(mapping.original).unwrap() == value })),
            "{result:#?}"
        );
    }
}
