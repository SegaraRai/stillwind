//! Literal structure normalization through the public compiler.

use stillwind_compiler::{CompileResult, Compiler, CompilerOptions};
use stillwind_diagnostics::{CompileErrorKind, VariantInputErrorKind};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{FileId, SourceFile};

fn compile(code: &str, css: &str) -> (SourceFile, CompileResult) {
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
        "test.tsx",
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        "stillwind.css",
        &CompilerOptions::default(),
    );
    (source, result)
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
    let css = "@variant hover { &:hover { @slot; } }";
    for value in ["'red'", "0", "dynamicValue"] {
        let (_, result) = compile(
            &format!("const x = style({{'hover:': {{'--x': {value}}}}});"),
            css,
        );
        assert!(
            result.diagnostics.iter().any(|error| error.kind
                == CompileErrorKind::InvalidVariantInput {
                    reason: VariantInputErrorKind::CustomProperty,
                }),
            "{result:#?}"
        );
        assert!(result.transforms.is_empty());
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
            &error.kind,
            CompileErrorKind::MalformedUtilityRequirement { utility, offset: 0 }
                if utility == "faulty"
        )),
        "{result:#?}"
    );
    assert!(result.transforms.is_empty());
}

#[test]
fn nested_variant_failure_labels_the_first_incompatible_leaf() {
    for (input, reason, label) in [
        (
            "const x = style({'hover:': [flag && {'aria-busy': true}]});",
            VariantInputErrorKind::Attribute,
            "aria-busy",
        ),
        (
            "const x = style({'hover:': [{class: external}]});",
            VariantInputErrorKind::ExternalClass,
            "external",
        ),
    ] {
        let (source, result) = compile(input, "@variant hover { &:hover { @slot; } }");
        let failure = result
            .diagnostics
            .iter()
            .find(|error| error.kind == CompileErrorKind::InvalidVariantInput { reason })
            .unwrap();
        assert_eq!(source.slice(failure.labels[0].span).unwrap(), label);
    }
}

#[test]
fn opaque_spreads_and_dynamic_keys_remain_diagnostics() {
    for (input, expected) in [
        ("[...values]", CompileErrorKind::StyleArraySpread),
        ("...values", CompileErrorKind::StyleArgumentSpread),
        ("{...values}", CompileErrorKind::AttributeSpread),
        ("{[key]: 'a'}", CompileErrorKind::InvalidComputedAttribute),
    ] {
        let (_, result) = compile(&format!("const x = style({input});"), "");
        assert!(
            result
                .diagnostics
                .iter()
                .any(|error| error.kind == expected),
            "{result:#?}"
        );
        assert!(result.transforms.is_empty());
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
