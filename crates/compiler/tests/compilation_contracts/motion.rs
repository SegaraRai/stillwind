//! Motion retention, dynamic candidates, placement, and conditional contributions.

use std::collections::BTreeMap;
use std::sync::Arc;

use stillwind_compiler::{Compiler, CompilerOptions};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind, SourceAnalyzer};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_semantic::{AssetKind, ExportedValue, SymbolicValue, TypeFact};
use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile};

use super::compile;

#[test]
fn motion_css_is_retained_for_uses_and_exports_only() {
    for (code, expected) in [
        ("const a = style.keyframes({from:'x'});", false),
        ("export const a = style.keyframes({from:'x'});", true),
        (
            "const a = style.keyframes({from:'x'}); const x = style(a(''));",
            true,
        ),
    ] {
        let (_, result) = compile(code, "@utility x { opacity:0; }");
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(
            result.css.contains("@keyframes"),
            expected,
            "{}",
            result.css
        );
    }
    for (code, expected) in [
        ("const a = style.viewTransition({old:'x'});", false),
        ("export const a = style.viewTransition({old:'x'});", true),
        (
            "const a = style.viewTransition({old:'x'}); const x = style(a);",
            true,
        ),
    ] {
        let (_, result) = compile(code, "@utility x { opacity:0; }");
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(
            result.css.contains("::view-transition-old"),
            expected,
            "{}",
            result.css
        );
    }
}

#[test]
fn dynamic_motion_keeps_every_checker_candidate_and_runtime_binding() {
    let source = SourceFile::new(FileId(1), None, None, "import { style } from 'stillwind/react'; const a=style.keyframes({from:'x'}); const b=style.keyframes({to:'x'}); const selected=flag?a:b; const x=style(selected(''));").unwrap();
    let scan = OxcSourceAnalyzer.scan(
        Arc::new(source.clone()),
        ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    let candidates = scan
        .summary
        .assets
        .iter()
        .filter(|a| a.identity.kind() == AssetKind::Keyframes)
        .map(|a| a.identity.clone())
        .collect::<Vec<_>>();
    let start = u32::try_from(source.text().rfind("selected(").unwrap()).unwrap();
    let facts = [TypeFact {
        start,
        end: start + 8,
        value: ExportedValue::Keyframes(
            SymbolicValue::finite(AssetKind::Keyframes, candidates).unwrap(),
        )
        .into(),
    }];
    let theme = SourceFile::new(
        FileId(2),
        None,
        None,
        "@utility x { opacity:0; } @stillwind-slot utilities; @stillwind-slot keyframes;",
    )
    .unwrap();
    let result = Compiler::compile_source_with_type_facts(
        &OxcSourceAnalyzer,
        &source,
        &ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        &CssSourceName::from("stillwind.css"),
        &facts,
        &BTreeMap::new(),
        &CompilerOptions::default(),
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.css.matches("@keyframes").count(), 2);
    let code = result.apply_to_source(&source).unwrap();
    assert!(!code.contains("const a=undefined"), "{code}");
    assert!(!code.contains("const b=undefined"), "{code}");
    // A declaration carries its keyframes name as a record, and the dynamic use reads
    // that name off the value instead of calling a runtime helper.
    assert!(code.contains("const a={k:\"sw-kf-"), "{code}");
    assert!(code.contains("[(selected).k]"), "{code}");
    let parsed = OxcSourceAnalyzer.scan(
        Arc::new(SourceFile::new(FileId(3), None, None, code).unwrap()),
        ModuleId::from("test.tsx"),
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
    );
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
}

#[test]
fn local_animation_definitions_follow_the_explicit_keyframes_slot() {
    let (_, result) = compile(
        "const keyframes=style.keyframes({from:'fade',to:'visible'});const x=style(keyframes(''));",
        r"
        @utility fade { opacity:0; }
        @utility visible { opacity:1; }
        @layer keyframes { @stillwind-slot keyframes; }
    ",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result.css.contains("@layer keyframes{@keyframes"),
        "{}",
        result.css
    );
    assert_eq!(result.css.matches("@keyframes").count(), 1);
}

#[test]
fn motion_definitions_transport_explicit_property_dependencies_without_directives() {
    let css = r#"
        @stillwind-property --dependency { activation: dependency; syntax: "*"; inherits: false; }
        @stillwind-slot properties;
        .reset { @stillwind-slot property-initials; }
        @utility consumer { @stillwind-require-properties --dependency; opacity:0; }
    "#;
    for (kind, definition) in [
        ("keyframes", "{from:'consumer'}"),
        ("keyframes", "{'[@media_print]:': {from:'consumer'}}"),
        ("viewTransition", "{old:'consumer'}"),
    ] {
        let (_, result) = compile(
            &format!("export const a = style.{kind}({definition});"),
            css,
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(
            result.css.matches("@property --dependency").count(),
            1,
            "{}",
            result.css
        );
        assert!(!result.module_css.contains("@stillwind-require-properties"));
        assert!(
            result
                .contributions
                .iter()
                .any(|part| part.required_properties == ["--dependency"])
        );
    }
}

#[test]
fn conditional_static_uses_share_one_unit_and_keep_transition_tokens() {
    let (source, result) = compile(
        "const r=style.ref();const a=style.viewTransition({old:'block'});const b=style.viewTransition({old:'block'});const k=style.keyframes({from:'block'});const x=style(r,flag&&[r.asAnchor(),a,b,k('')]);",
        "@stillwind-utility block {display:block;}",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        result
            .classes
            .iter()
            .filter(|class| !class.guard_path.is_empty())
            .count(),
        1,
        "{:?}",
        result.classes
    );
    assert!(result.css.contains("animation-name:"));
    assert!(result.css.contains("anchor-name:"));
    assert!(result.css.contains("view-transition-class:"));
    assert!(!result.apply_to_source(&source).unwrap().contains("(r).c"));
}

#[test]
fn css_wrapped_transitions_merge_into_the_same_conditional_unit() {
    let (_, result) = compile(
        "const r=style.ref();const a=style.viewTransition({old:'block'});const b=style.viewTransition({old:'block'});const x=style(r,flag&&[r.asAnchor(),{'hover:':[a.for('a'),b]}]);",
        "@stillwind-utility block {display:block;} @stillwind-variant hover { &:hover { @slot; } }",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        result
            .classes
            .iter()
            .filter(|class| !class.guard_path.is_empty())
            .count(),
        1,
        "{:?}",
        result.classes
    );
    assert!(result.css.contains(":hover"));
    assert!(result.css.contains("view-transition-class:"));
}
