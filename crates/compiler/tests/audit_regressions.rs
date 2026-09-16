//! Public compiler regressions for implementation defects found during the comment audit.

use std::collections::BTreeMap;

use stillwind_compiler::{CompileResult, Compiler};
use stillwind_css::MAX_VALUE_NESTING;
use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_diagnostics::{CompileErrorKind, LintKind, UtilityFailure};
use stillwind_ir::{ClassExpression, SourceAnalyzer, StyleInput};
use stillwind_js_oxc::{OxcSourceAnalyzer, parse, parse_with_type_facts};
use stillwind_semantic::{
    AssetKind, ExportedValue, IdentityId, RefAlternative, SymbolicValue, TypeFact,
};
use stillwind_source::{FileId, SourceError, SourceFile, Span};

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
fn deeply_nested_utility_values_fail_with_a_nesting_diagnostic() {
    let levels = MAX_VALUE_NESTING + 1;
    let value = format!("{}1{}", "(".repeat(levels), ")".repeat(levels));
    let utility = format!("w-[calc({value})]");
    let (source, result) = compile(
        &format!("export const x = style(\"{utility}\");"),
        "@theme { --spacing: 1px; }",
    );
    let diagnostic = result.diagnostics.first().expect("a nesting diagnostic");
    assert!(
        matches!(
            diagnostic.kind,
            CompileErrorKind::CssNestingTooDeep { limit, .. }
                if limit == MAX_VALUE_NESTING
        ),
        "{:#?}",
        result.diagnostics
    );
    assert_eq!(source.slice(diagnostic.labels[0].span).unwrap(), utility);
    assert!(result.css.is_empty(), "{}", result.css);
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
fn punctuation_labels_produce_valid_javascript_strings() {
    let (_, result) = compile(
        r#"export const subtree = style.subtree('a"b/c'); export const ref = style.ref(subtree, 'd"e/f'); export const child = style({[ref()]: 'a'});"#,
        "@utility a { color:red; }",
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    for edit in &result.transforms {
        // Import insertions and removals are statements of their own; every other edit
        // has to carry a parseable value.
        if edit.replacement.starts_with("import ") || edit.replacement.is_empty() {
            continue;
        }
        let source = SourceFile::new(
            FileId(3),
            "generated.ts",
            format!("const value = {};", edit.replacement),
        )
        .unwrap();
        assert!(parse(&source).diagnostics.is_empty(), "{}", source.text());
    }
    // The marker spells the normalized label, and the stylesheet escapes the punctuation
    // the label carries.
    assert!(!result.css.contains("d\"e"), "{}", result.css);
    let marker = result
        .symbols
        .iter()
        .find(|symbol| symbol.kind == "ref-marker")
        .expect("ref marker symbol");
    assert!(marker.name.starts_with("s_d\"e/f_"), "{}", marker.name);
    let escaped = marker.name.replace('"', "\\\"").replace('/', "\\/");
    assert!(result.css.contains(&escaped), "{}", result.css);
}

#[test]
fn normalized_labels_reach_only_the_marker_names() {
    let (_, result) = compile(
        "const subtree = style.subtree('領域 :'); const ref = style.ref(subtree, '項目 🧭'); \
         export const root = style(subtree); export const item = style(ref); \
         export const child = style({[ref()]: 'a'});",
        "@utility a { color:red; }",
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    // A marker spells its declaration's normalized label, which is what makes it readable
    // in generated markup; every other family hashes the declaration.
    let markers = result
        .symbols
        .iter()
        .filter(|symbol| symbol.kind.ends_with("marker"))
        .map(|symbol| symbol.name.as_str())
        .collect::<Vec<_>>();
    assert!(
        markers.iter().any(|name| name.contains("領域")),
        "{markers:?}"
    );
    assert!(
        markers.iter().any(|name| name.contains("項目")),
        "{markers:?}"
    );
    for symbol in result
        .symbols
        .iter()
        .filter(|symbol| !symbol.kind.ends_with("marker"))
    {
        for authored in ["領域", "項目", "🧭"] {
            assert!(!symbol.name.contains(authored), "{}", symbol.name);
        }
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
fn static_ref_projection_reads_the_use_site_receiver() {
    let source = SourceFile::new(
        FileId(1),
        "consumer.tsx",
        "import { style } from 'stillwind/react'; import { primary as p } from './refs'; export const x = style(p.anchor);",
    )
    .unwrap();
    let receiver: u32 = source.text().rfind("p.anchor").unwrap().try_into().unwrap();
    let reference = IdentityId::new(AssetKind::Ref, "refs.tsx", 6, 13, "primary");
    let facts = [TypeFact {
        start: receiver,
        end: receiver + 1,
        value: ExportedValue::Refs(vec![RefAlternative::new(reference, None).unwrap()]),
    }];
    let theme = SourceFile::new(
        FileId(2),
        "theme.css",
        "@theme {} @stillwind-slot utilities; @stillwind-slot keyframes;",
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
    let code = result.apply_to_source(&source).unwrap();
    let value = code.split("export const x = ").nth(1).unwrap();
    assert!(value.contains("p.c"), "{value}");
    assert!(!value.contains("primary"), "{value}");
}

#[test]
fn static_ref_projection_reads_a_member_receiver() {
    let source = SourceFile::new(
        FileId(1),
        "consumer.tsx",
        "import { style } from 'stillwind/react'; export const x = style(holder.trigger.anchor);",
    )
    .unwrap();
    let receiver: u32 = source
        .text()
        .rfind("holder.trigger.anchor")
        .unwrap()
        .try_into()
        .unwrap();
    let reference = IdentityId::new(AssetKind::Ref, "holder.tsx", 6, 13, "trigger");
    let facts = [TypeFact {
        start: receiver,
        end: receiver + u32::try_from("holder.trigger".len()).unwrap(),
        value: ExportedValue::Refs(vec![RefAlternative::new(reference, None).unwrap()]),
    }];
    let theme = SourceFile::new(
        FileId(2),
        "theme.css",
        "@theme {} @stillwind-slot utilities; @stillwind-slot keyframes;",
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
    let code = result.apply_to_source(&source).unwrap();
    let value = code.split("export const x = ").nth(1).unwrap();
    assert!(value.contains("holder.trigger.c"), "{value}");
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
    let identity = IdentityId::new(AssetKind::Keyframes, "producer.tsx", 4000, 4006, "motion");
    let facts = [TypeFact {
        start,
        end: start + 6,
        value: ExportedValue::Keyframes(SymbolicValue::exact(identity.clone())),
    }];
    let parsed = parse_with_type_facts(&source, &facts, &BTreeMap::new());
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let mapped = parsed
        .module
        .try_map_spans(|span| {
            source.slice(span)?;
            Ok::<_, SourceError>(span)
        })
        .unwrap();
    let StyleInput::Class(ClassExpression::KeyframesUse { keyframes, .. }) =
        &mapped.style_calls[0].inputs[0]
    else {
        panic!("expected static keyframes");
    };
    assert_eq!(keyframes.identity, identity);
    assert_eq!(source.slice(keyframes.span).unwrap(), "motion");
    assert!(keyframes.local_declaration.is_none());
}

#[test]
fn local_motion_declaration_identity_follows_projection() {
    let source = SourceFile::new(FileId(1), "local.tsx", "import { style } from 'stillwind/react'; const a=style.keyframes({from:'x'}); const x=style(a(''));").unwrap();
    let parsed = parse(&source);
    let mapped = parsed
        .module
        .try_map_spans(|span| Ok::<_, ()>(Span::new(span.file, span.start.0 + 20, span.end.0 + 20)))
        .unwrap();
    let StyleInput::Class(ClassExpression::KeyframesUse { keyframes, .. }) =
        &mapped.style_calls[0].inputs[0]
    else {
        panic!("expected keyframes");
    };
    assert_eq!(
        keyframes.identity.start,
        mapped.keyframes_declarations[0].binding.span.start.0
    );
    assert_eq!(
        keyframes.identity.end,
        mapped.keyframes_declarations[0].binding.span.end.0
    );
    assert_eq!(
        keyframes.local_declaration,
        Some(mapped.keyframes_declarations[0].binding.span)
    );
}

#[test]
fn dynamic_motion_keeps_every_checker_candidate_and_runtime_binding() {
    let source = SourceFile::new(FileId(1), "dynamic.tsx", "import { style } from 'stillwind/react'; const a=style.keyframes({from:'x'}); const b=style.keyframes({to:'x'}); const selected=flag?a:b; const x=style(selected(''));").unwrap();
    let scan = OxcSourceAnalyzer.scan(std::sync::Arc::new(source.clone()));
    let candidates = scan
        .summary
        .assets
        .iter()
        .filter(|a| a.identity.kind == AssetKind::Keyframes)
        .map(|a| a.identity.clone())
        .collect::<Vec<_>>();
    let start = u32::try_from(source.text().rfind("selected(").unwrap()).unwrap();
    let facts = [TypeFact {
        start,
        end: start + 8,
        value: ExportedValue::Keyframes(
            SymbolicValue::finite(AssetKind::Keyframes, candidates).unwrap(),
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
