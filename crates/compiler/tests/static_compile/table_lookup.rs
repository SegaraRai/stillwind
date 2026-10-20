//! Lowering of lookups in array and object literals inside `style()`.
//!
//! Each table entry becomes its own conditional class unit, and the generated code
//! keeps one runtime lookup of the original key. Literal keys select their entry
//! statically.

use stillwind_compiler::{CompileResult, CompilerOptions};
use stillwind_diagnostics::LintDiagnostic;
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, ModuleId};
use stillwind_test_support::css_source;

use super::*;

fn compile(body: &str) -> CompileResult {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        format!(
            "import {{ style }} from \"stillwind/react\";\n\
             declare const kind: \"file\" | \"folder\";\n\
             declare const index: number;\n\
             declare const other: string;\n\
             {body}"
        ),
    )
    .unwrap();
    let theme = css_source(
        FileId(2),
        "@theme {} @utility p-4 { padding: 1rem; } @utility p-2 { padding: 0.5rem; } \
         @utility m-2 { margin: 0.5rem; } @utility m-4 { margin: 1rem; }",
    )
    .unwrap();
    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &CompilerOptions::default(),
    );
    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    result
}

fn emitted(result: &CompileResult) -> &str {
    &result.transforms[0].replacement
}

fn lints(result: &CompileResult) -> Vec<&LintDiagnostic> {
    result
        .lint_findings
        .iter()
        .map(|finding| &finding.diagnostic)
        .collect()
}

/// Returns the table key that selects each generated class, or `None` for a class
/// without a table step.
fn selecting_keys(result: &CompileResult) -> Vec<Option<&str>> {
    result
        .classes
        .iter()
        .map(|class| class.guard_path.iter().find_map(|step| step.key.as_deref()))
        .collect()
}

#[test]
fn object_tables_select_one_unit_per_entry_through_one_lookup() {
    let result = compile("const props = style(\"p-4\", { file: \"p-2\", folder: \"m-2\" }[kind]);");
    let replacement = emitted(&result);
    assert!(
        replacement.contains("({__proto__:null,\"file\":\"") && replacement.contains("})[kind]"),
        "{replacement}"
    );
    assert_eq!(replacement.matches("kind").count(), 1, "{replacement}");
    assert_eq!(
        selecting_keys(&result),
        [None, Some("file"), Some("folder")]
    );
    let order = ["padding: 1rem", "padding: 0.5rem", "margin: 0.5rem"].map(|rule| {
        result
            .css
            .find(rule)
            .unwrap_or_else(|| panic!("missing {rule} in {}", result.css))
    });
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{order:?}");
}

#[test]
fn array_tables_use_decimal_index_keys() {
    let result = compile("const props = style([\"p-2\", , \"m-2\"][index % 2]);");
    let replacement = emitted(&result);
    assert!(replacement.contains("})[index % 2]"), "{replacement}");
    assert_eq!(selecting_keys(&result), [Some("0"), Some("2")]);
}

#[test]
fn entries_of_one_table_do_not_conflict() {
    let result = compile("const props = style({ file: \"p-2\", folder: \"p-4\" }[kind]);");
    assert_eq!(lints(&result), [] as [&LintDiagnostic; 0]);
}

#[test]
fn tables_keep_enclosing_variants() {
    let result = compile("const props = style({ \"hover:\": { file: \"p-2\" }[kind] });");
    assert!(
        normalize_css(&result.css).contains(":hover{padding:0.5rem;}"),
        "{}",
        result.css
    );
    assert_eq!(selecting_keys(&result), [Some("file")]);
}

#[test]
fn literal_keys_select_their_entry_statically() {
    for (body, padding) in [
        (
            "const props = style({ file: \"p-2\", folder: \"m-2\" }[\"file\"]);",
            true,
        ),
        (
            "const props = style({ file: \"p-2\", folder: \"m-2\" }.folder);",
            false,
        ),
        ("const props = style([\"m-2\", \"p-2\"][1]);", true),
        (
            "const props = style({ ...{ file: \"m-2\" }, file: \"p-2\" }[`file`]);",
            true,
        ),
    ] {
        let result = compile(body);
        let replacement = emitted(&result);
        assert!(!replacement.contains("__proto__"), "{body}: {replacement}");
        assert_eq!(selecting_keys(&result), [None], "{body}");
        assert_eq!(result.css.contains("padding: 0.5rem"), padding, "{body}");
        assert_eq!(result.css.contains("margin: 0.5rem"), !padding, "{body}");
    }
}

#[test]
fn tables_without_selectable_values_contribute_nothing() {
    for body in [
        "const props = style({ file: null, folder: false }[kind]);",
        "const props = style([undefined][index]);",
        "const props = style({ file: \"p-2\" }[\"folder\"]);",
    ] {
        let result = compile(body);
        assert_eq!(emitted(&result), "{}", "{body}");
        assert_eq!(lints(&result), [] as [&LintDiagnostic; 0], "{body}");
    }
}

#[test]
fn tables_without_static_entries_are_omitted() {
    for body in [
        "const props = style({ file: \"p-2\", [other]: \"m-2\" }[kind]);",
        "const props = style({ file: \"p-2\", ...props }[kind]);",
        "const props = style([\"p-2\", ...other][index]);",
        "const props = style({ __proto__: \"p-2\" }[kind]);",
    ] {
        let result = compile(body);
        assert_eq!(emitted(&result), "{}", "{body}");
        assert_eq!(
            lints(&result),
            [&LintDiagnostic::InvalidStyleTable],
            "{body}"
        );
    }
}

#[test]
fn entries_without_literal_values_are_omitted_alone() {
    for body in [
        "const props = style({ file: \"p-2\", folder: other }[kind]);",
        "const props = style({ file: \"p-2\", get folder() { return \"m-2\"; } }[kind]);",
        "const props = style({ file: \"p-2\", folder: `m-${other}` }[kind]);",
    ] {
        let result = compile(body);
        assert_eq!(selecting_keys(&result), [Some("file")], "{body}");
        assert_eq!(
            lints(&result),
            [&LintDiagnostic::InvalidStyleTableEntry],
            "{body}"
        );
    }
}
