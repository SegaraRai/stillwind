//! CSS parsing contracts across definition loading, evaluation, and emission.

use stillwind_compiler::{
    CandidateContext, CompileResult, Compiler, CompilerOptions, DesignSystemQuery,
};
use stillwind_css::{CssNode, CssNodeKind, parse_structure, validate_css_syntax};
use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile};

fn compile(css: &str, candidate: &str) -> CompileResult {
    let code =
        format!("import {{style}} from 'stillwind/react'; export const x = style({candidate:?});");
    Compiler::compile_source(
        &OxcSourceAnalyzer,
        &SourceFile::new(FileId(1), None, None, code).unwrap(),
        &ModuleId::from("parsing.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Module,
        &SourceFile::new(
            FileId(2),
            None,
            None,
            format!("{css} @stillwind-slot utilities;"),
        )
        .unwrap(),
        &CssSourceName::from("parsing.css"),
        &CompilerOptions::default(),
    )
}

fn values<'a>(css: &'a str, property_name: &str) -> Vec<&'a str> {
    fn collect<'a>(css: &'a str, nodes: &[CssNode], name: &str, values: &mut Vec<&'a str>) {
        for node in nodes {
            if let CssNodeKind::Declaration { property, value } = &node.kind
                && &css[property.clone()] == name
            {
                values.push(&css[value.clone()]);
            }
            collect(css, node.children(), name, values);
        }
    }
    let mut values = Vec::new();
    collect(css, &parse_structure(css), property_name, &mut values);
    values
}

fn successful(css: &str, candidate: &str) -> CompileResult {
    let result = compile(css, candidate);
    assert!(
        result.diagnostics.is_empty(),
        "{css}: {:?}",
        result.diagnostics
    );
    assert!(
        result.lint_findings.is_empty(),
        "{css}: {:?}",
        result.lint_findings
    );
    assert_eq!(validate_css_syntax(&result.css), Ok(()), "{}", result.css);
    result
}

#[test]
fn value_maps_keep_quoted_commas_parentheses_and_escaped_delimiters() {
    for value in [r#""a,b""#, r#""(""#, r#"")""#, r#""}""#, r"foo\,bar"] {
        let css = format!("@utility probe-* {{ --answer:--value-map(x: {value}, y: ok); }}");
        let first = successful(&css, "probe-x");
        assert_eq!(values(&first.css, "--answer"), [value], "{}", first.css);
        let second = successful(&css, "probe-y");
        assert_eq!(values(&second.css, "--answer"), ["ok"]);
    }
}

#[test]
fn value_map_preview_keeps_the_whole_quoted_value() {
    let source = SourceFile::new(FileId(1), None, None, r#"@utility probe-* { content:--value-map(x: "a,b", y: "ok"); } @stillwind-slot utilities;"#).unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &source,
        "parsing.css".into(),
        DesignSystemOptions::default(),
    )
    .unwrap();
    let query = DesignSystemQuery::new(snapshot, CompilerOptions::default());
    let preview = query
        .preview_candidate("probe-x", CandidateContext::Utility)
        .unwrap()
        .unwrap();
    assert_eq!(values(&preview, "content"), [r#""a,b""#]);
    assert_eq!(validate_css_syntax(&preview), Ok(()));
}

#[test]
fn definition_headers_keep_whitespace_consumed_by_css_escapes() {
    let result = successful(r"@utility probe-\31 x { color:red; }", "probe-1x");
    assert_eq!(values(&result.css, "color"), ["red"]);
}

#[test]
fn utility_and_variant_bodies_keep_delimiters_inside_strings() {
    for value in ["(", ")", "{", "}", ";", "&"] {
        let css = format!(r#"@utility probe {{ &[data-x="{value}"] {{ --answer:"{value}"; }} }}"#);
        let result = successful(&css, "probe");
        assert!(
            result.css.contains(&format!(r#"[data-x="{value}"]"#)),
            "{}",
            result.css
        );
        assert_eq!(values(&result.css, "--answer"), [format!("\"{value}\"")]);

        let css = format!(
            r#"@custom-variant probe {{ & {{ --answer:"{value}"; @slot; }} }} @utility red {{ color:red; }}"#
        );
        let result = successful(&css, "probe:red");
        assert_eq!(values(&result.css, "--answer"), [format!("\"{value}\"")]);
    }
    let result = successful(r#"@utility probe { content:"&"; color:red; }"#, "probe");
    assert_eq!(values(&result.css, "content"), [r#""&""#]);
}

#[test]
fn keyframes_keep_quoted_and_commented_braces_and_following_rules() {
    for frame in [
        r#"--answer:"}";"#,
        r#"--answer:"{";"#,
        "/* } { */ --answer:ok;",
    ] {
        let css = format!(
            "@stillwind-theme-namespace --animate-* [animation]; @theme {{ --animate-probe:probe 1s linear; @keyframes probe {{ from {{ {frame} }} to {{ --answer:done; }} }} }} @utility probe {{ animation:var(--animate-probe); }} :root {{ @stillwind-slot theme; }} @stillwind-slot keyframes; .after {{ --after:yes; }}"
        );
        let result = successful(&css, "probe");
        assert!(result.css.contains("@keyframes probe"), "{}", result.css);
        assert!(
            values(&result.css, "--answer").contains(&"done"),
            "{}",
            result.css
        );
        assert_eq!(values(&result.css, "--after"), ["yes"]);
    }
}

#[test]
fn theme_declarations_and_function_fallbacks_use_css_token_boundaries() {
    let result = successful(
        r#"@theme { --foo:"a;b"; } :root { @stillwind-slot theme; } @utility probe { content:var(--foo); }"#,
        "probe",
    );
    assert_eq!(values(&result.css, "--foo"), [r#""a;b""#]);
    for arguments in ["--missing/* , */ , red", r"--missing\,part, red"] {
        let css = format!(".a {{ --answer:--theme({arguments}); }} @utility probe {{color:red;}}");
        let result = successful(&css, "probe");
        assert_eq!(values(&result.css, "--answer"), ["red"]);
    }
}

#[test]
fn malformed_definition_values_report_errors_before_evaluation() {
    for value in ["[)]", "]", "([)]", "(foo", "[foo"] {
        for definition in [
            format!("@utility probe {{ --answer:{value}; color:red; }}"),
            format!(
                "@custom-variant bad {{ & {{ --answer:{value}; @slot; }} }} @utility probe {{color:red;}}"
            ),
            format!("@theme {{ --answer:{value}; }} @utility probe {{color:red;}}"),
        ] {
            let result = compile(&definition, "probe");
            assert!(
                !result.diagnostics.is_empty(),
                "{definition}: {}",
                result.css
            );
        }
    }
}

#[test]
fn ordinary_stylesheets_retain_css_recovery_at_eof() {
    let result = compile(
        "@utility probe { color:red; } .a {color:blue;} @stillwind-slot utilities; /* unfinished",
        "probe",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(values(&result.css, "color"), ["blue", "red"]);
}
