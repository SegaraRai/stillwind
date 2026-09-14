//! Literal structure normalization through the public compiler.

use stillwind_compiler::{CompileResult, Compiler};
use stillwind_diagnostics::CompileErrorKind;
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{FileId, SourceFile};

fn compile(code: &str, css: &str) -> (SourceFile, CompileResult) {
    let source = SourceFile::new(
        FileId(1),
        "entry.tsx",
        format!("import {{ style }} from 'stillwind/react'; {code}"),
    )
    .unwrap();
    let theme = SourceFile::new(
        FileId(2),
        "theme.css",
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
    let result = Compiler::compile_source(&OxcSourceAnalyzer, &source, &theme);
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
