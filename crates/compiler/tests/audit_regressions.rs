//! Public compiler regressions for implementation defects found during the comment audit.

use std::collections::BTreeMap;

use stillwind_compiler::{CompileResult, Compiler};
use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_diagnostics::{LintKind, UtilityFailure};
use stillwind_ir::{ClassExpression, SourceAnalyzer, StyleInput};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_semantic::{AssetKind, ExportedValue, IdentityId, SymbolicValue, TypeFact};
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
fn utility_precedence_is_independent_of_literal_and_array_order() {
    let css = "@utility a { color:red; } @utility b { color:blue; } \
               @stillwind-utility builtin { color:green; }";
    for input in [
        "'a b builtin'",
        "'builtin b a'",
        "['b', 'builtin', 'a']",
        "'b', 'a', 'builtin'",
    ] {
        let (_, result) = compile(&format!("export const x = style({input});"), css);
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        assert!(result.lint_findings.is_empty(), "{result:#?}");
        assert!(
            result.css.ends_with("{color:red;color:blue;color:green;}"),
            "{input}: {}",
            result.css
        );
    }
    for input in ["'hover:b hover:a'", "['hover:a', 'hover:b']"] {
        let (_, result) = compile(
            &format!("export const x = style({input});"),
            &format!("{css} @variant hover {{ &:hover {{ @slot; }} }}"),
        );
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        let red = result.css.find("color:red;").unwrap();
        let blue = result.css.find("color:blue;").unwrap();
        assert!(red < blue, "{input}: {}", result.css);
    }
}

#[test]
fn unicode_and_punctuation_labels_share_dom_and_selector_names() {
    let (_, result) = compile(
        "const scope = style.scope('領域 :'); const ref = style.ref(scope, '項目 🧭'); \
         export const root = style(scope); export const item = style(ref); \
         export const child = style({[ref()]: 'a'});",
        "@utility a { color:red; }",
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    let code = result
        .transforms
        .iter()
        .map(|edit| edit.replacement.as_str())
        .collect::<String>();
    for prefix in ["s_領域_20__3a__", "s_項目_20_🧭_"] {
        assert!(code.contains(prefix), "{code}");
        assert!(result.css.contains(prefix), "{}", result.css);
    }
}

#[test]
fn recursive_theme_macros_reject_the_candidate_without_partial_output() {
    for value in ["--value(--color-*)", "calc(--value(--color-*) + 1)"] {
        let (_, result) = compile(
            "const x = style('x-loop');",
            &format!(
                "@utility x-* {{ color: --value(--color-*); }} @theme inline {{ --color-loop: {value}; }}"
            ),
        );
        assert!(
            result.lint_findings.iter().any(|finding| matches!(
                finding.kind,
                LintKind::UnresolvedUtility {
                    classification: UtilityFailure::Unknown,
                    ..
                }
            )),
            "{result:#?}"
        );
        assert!(result.css.is_empty());
    }
}

#[test]
fn css_values_survive_emission_and_importance() {
    let (_, result) = compile(
        "const x = style('x!');",
        r#"@utility x { content: "calc(1 + 1);a"; width:calc(1px + 2px); background:url('data:text/plain;calc(1 + 1)'); }"#,
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result
            .css
            .contains(r#"content: "calc(1 + 1);a"!important;"#),
        "{}",
        result.css
    );
    assert!(
        result.css.contains("width:calc(1px + 2px)!important"),
        "{}",
        result.css
    );
    assert!(
        result.css.contains("data:text/plain;calc(1 + 1)"),
        "{}",
        result.css
    );
}

#[test]
fn mixed_variant_forms_override_in_source_order_even_on_one_line() {
    for (definitions, expected) in [
        (
            "@variant state { &:hover { @slot; } } @custom-variant state (&:focus);",
            ":focus",
        ),
        (
            "@custom-variant state (&:focus); @variant state { &:hover { @slot; } }",
            ":hover",
        ),
        (
            "@variant state { &:hover { @slot; } } @custom-variant state { &:active { @slot; } }",
            ":active",
        ),
    ] {
        let (_, result) = compile(
            "const x = style('state:x');",
            &format!("@utility x {{ color:red; }} {definitions}"),
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(result.css.contains(expected), "{}", result.css);
    }
}

#[test]
fn motion_css_is_retained_for_uses_and_exports_only() {
    for (code, expected) in [
        ("const a = style.animation({from:'x'});", false),
        ("export const a = style.animation({from:'x'});", true),
        (
            "const a = style.animation({from:'x'}); const x = style(a(''));",
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
fn reset_tokens_are_absent_from_definition_fingerprints() {
    let source = SourceFile::new(
        FileId(1),
        "theme.css",
        "@theme { --color-red:red; } @theme { --color-*:initial; --color-blue:blue; }",
    )
    .unwrap();
    let snapshot =
        DesignSystemSnapshot::from_source(&source, DesignSystemOptions::default()).unwrap();
    assert!(
        !snapshot
            .fingerprints()
            .theme_tokens
            .contains_key("--color-red")
    );
    assert!(
        !snapshot
            .fingerprints()
            .theme_tokens
            .contains_key("--color-*")
    );
    assert!(
        snapshot
            .fingerprints()
            .theme_tokens
            .contains_key("--color-blue")
    );
}

#[test]
fn escaped_utility_diagnostics_keep_the_literal_provenance() {
    let (source, result) = compile(r"const x = style('\u006eot-a-utility');", "");
    assert!(!result.lint_findings.is_empty());
    let spans = result
        .lint_findings
        .iter()
        .flat_map(|d| d.labels.iter())
        .map(|label| source.slice(label.span).unwrap())
        .collect::<Vec<_>>();
    assert!(spans.contains(&r"\u006eot-a-utility"), "{spans:?}");
}

#[test]
fn external_motion_identity_does_not_become_a_local_source_span() {
    let source = SourceFile::new(FileId(1), "consumer.tsx", "import { style } from 'stillwind/react'; import { motion } from './producer'; const x = style(motion(''));").unwrap();
    let start = u32::try_from(source.text().rfind("motion(").unwrap()).unwrap();
    let identity = IdentityId::new(AssetKind::Animation, "producer.tsx", 4000, 4006, "motion");
    let facts = [TypeFact {
        start,
        end: start + 6,
        value: ExportedValue::Animations(SymbolicValue::exact(identity.clone())),
    }];
    let parsed = stillwind_js_oxc::parse_with_type_facts(&source, &facts, &BTreeMap::new());
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let mapped = parsed
        .module
        .try_map_spans(|span| {
            source.slice(span)?;
            Ok::<_, stillwind_source::SourceError>(span)
        })
        .unwrap();
    let StyleInput::Class(ClassExpression::AnimationUse { animation, .. }) =
        &mapped.style_calls[0].inputs[0]
    else {
        panic!("expected static animation");
    };
    assert_eq!(animation.identity, identity);
    assert_eq!(source.slice(animation.span).unwrap(), "motion");
    assert!(animation.local_declaration.is_none());
}

#[test]
fn local_motion_declaration_identity_follows_projection() {
    let source = SourceFile::new(FileId(1), "local.tsx", "import { style } from 'stillwind/react'; const a=style.animation({from:'x'}); const x=style(a(''));").unwrap();
    let parsed = stillwind_js_oxc::parse(&source);
    let mapped = parsed
        .module
        .try_map_spans(|span| {
            Ok::<_, ()>(stillwind_source::Span::new(
                span.file,
                span.start.0 + 20,
                span.end.0 + 20,
            ))
        })
        .unwrap();
    let StyleInput::Class(ClassExpression::AnimationUse { animation, .. }) =
        &mapped.style_calls[0].inputs[0]
    else {
        panic!("expected animation");
    };
    assert_eq!(
        animation.identity.start,
        mapped.animation_declarations[0].name.span.start.0
    );
    assert_eq!(
        animation.identity.end,
        mapped.animation_declarations[0].name.span.end.0
    );
    assert_eq!(
        animation.local_declaration,
        Some(mapped.animation_declarations[0].name.span)
    );
}

#[test]
fn dynamic_motion_keeps_every_checker_candidate_and_runtime_binding() {
    let source = SourceFile::new(FileId(1), "dynamic.tsx", "import { style } from 'stillwind/react'; const a=style.animation({from:'x'}); const b=style.animation({to:'x'}); const selected=flag?a:b; const x=style(selected(''));").unwrap();
    let scan = OxcSourceAnalyzer.scan(std::sync::Arc::new(source.clone()));
    let candidates = scan
        .summary
        .assets
        .iter()
        .filter(|a| a.identity.kind == AssetKind::Animation)
        .map(|a| a.identity.clone())
        .collect::<Vec<_>>();
    let start = u32::try_from(source.text().rfind("selected(").unwrap()).unwrap();
    let facts = [TypeFact {
        start,
        end: start + 8,
        value: ExportedValue::Animations(
            SymbolicValue::finite(AssetKind::Animation, candidates).unwrap(),
        ),
    }];
    let theme = SourceFile::new(
        FileId(2),
        "theme.css",
        "@utility x { opacity:0; } @stillwind-slot utilities; @stillwind-slot keyframes;",
    )
    .unwrap();
    let result = Compiler::compile_source_with_type_facts(
        &OxcSourceAnalyzer,
        &source,
        &theme,
        &facts,
        &BTreeMap::new(),
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.css.matches("@keyframes").count(), 2);
    let code = result.apply_to_source(&source).unwrap();
    assert!(!code.contains("const a=undefined"), "{code}");
    assert!(!code.contains("const b=undefined"), "{code}");
    let parsed = OxcSourceAnalyzer.scan(std::sync::Arc::new(
        SourceFile::new(FileId(3), "generated.tsx", code).unwrap(),
    ));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
}

#[test]
fn literal_values_and_explicit_fallbacks_survive_utility_emission() {
    let (_, result) = compile(
        "const x = style('literal');",
        r#"
        @stillwind-property --registered { syntax: "<length>"; inherits: false; initial-value: 0px; }
        @stillwind-slot properties;
        *,:before,:after,::backdrop { @stillwind-slot property-initials; }
        @utility literal {
            content: ",var(--x) oklab(0% none none/0) deg*-1";
            transform: var(--component);
            width: var(--registered, 0px);
        }
    "#,
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result
            .css
            .contains(r#"",var(--x) oklab(0% none none/0) deg*-1""#),
        "{}",
        result.css
    );
    assert!(result.css.contains("var(--component)"), "{}", result.css);
    assert!(!result.css.contains("var(--component,)"));
    assert!(
        result.css.contains("var(--registered, 0px)"),
        "{}",
        result.css
    );
}

#[test]
fn explicit_registration_dependencies_survive_apply_and_do_not_leak_into_css() {
    let (_, result) = compile(
        "const x = style('consumer');",
        r#"
        @stillwind-property --dependency { activation: dependency; syntax: "*"; inherits: false; }
        @stillwind-slot properties;
        *,:before,:after,::backdrop { @stillwind-slot property-initials; }
        @utility consumer { @stillwind-require-properties --dependency; transform:var(--dependency,); }
        .authored { @apply consumer; }
    "#,
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.css.matches("@property --dependency").count(), 1);
    assert!(!result.css.contains("@stillwind-require-properties"));
    let (_, missing) = compile(
        "const x = style('consumer');",
        "@utility consumer { @stillwind-require-properties --missing; color:red; }",
    );
    assert!(!missing.diagnostics.is_empty());
}

#[test]
fn local_animation_definitions_follow_the_explicit_keyframes_slot() {
    let (_, result) = compile(
        "const animation=style.animation({from:'fade',to:'visible'});const x=style(animation(''));",
        r"
        @utility fade { opacity:0; }
        @utility visible { opacity:1; }
        @layer animations { @stillwind-slot keyframes; }
    ",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result.css.contains("@layer animations{@keyframes"),
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
        ("animation", "{from:'consumer'}"),
        ("animation", "{'[@media_print]:': {from:'consumer'}}"),
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
