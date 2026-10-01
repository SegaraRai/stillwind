//! Ref names, projections, scope lists, and provider diagnostics.

use std::collections::BTreeMap;

use stillwind_compiler::{Compiler, CompilerOptions};
use stillwind_diagnostics::{CompileDiagnostic, LintDiagnostic};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::{OxcSourceAnalyzer, parse};
use stillwind_semantic::{AssetKind, ExportedValue, IdentityId, RefAlternative, TypeFact};
use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile};

use super::compile;

#[test]
fn punctuation_labels_produce_valid_javascript_strings() {
    let (_, result) = compile(
        r#"export const boundary = style.ref('a"b/c'); export const ref = style.ref(boundary, 'd"e/f'); export const child = style({[ref()]: 'a'});"#,
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
            None,
            None,
            format!("const value = {};", edit.replacement),
        )
        .unwrap();
        assert!(
            parse(
                &source,
                &ModuleId::from("test.tsx"),
                &[],
                &BTreeMap::default(),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous
            )
            .diagnostics
            .is_empty(),
            "{}",
            source.text()
        );
    }
    // The marker spells the normalized label, and the stylesheet escapes the punctuation
    // the label carries.
    assert!(!result.css.contains("d\"e"), "{}", result.css);
    let marker = result
        .symbols
        .iter()
        .find(|symbol| symbol.kind == "ref-marker" && symbol.name.starts_with("sw_d\"e/f_"))
        .expect("ref marker symbol");
    let escaped = marker.name.replace('"', "\\\"").replace('/', "\\/");
    assert!(result.css.contains(&escaped), "{}", result.css);
}

#[test]
fn a_label_reaches_every_name_its_own_declaration_generates() {
    let (_, result) = compile(
        "const boundary = style.ref('領域 :'); const ref = style.ref(boundary, '項目 🧭'); \
         export const root = style(boundary); export const item = style(ref); \
         export const child = style({[ref()]: 'a'});",
        "@utility a { color:red; }",
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    // A declaration's label is spelled into every name generated for that declaration,
    // which is what makes the names readable in generated markup.
    for kind in ["ref-marker", "ref"] {
        let names = result
            .symbols
            .iter()
            .filter(|symbol| symbol.kind == kind)
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert!(
            names.iter().any(|name| name.contains("領域")),
            "{kind}: {names:?}"
        );
        assert!(
            names.iter().any(|name| name.contains("項目")),
            "{kind}: {names:?}"
        );
    }
    // A utility class is named for one call, not for a declaration, so it never carries a label.
    for symbol in result
        .symbols
        .iter()
        .filter(|symbol| symbol.kind == "class")
    {
        for authored in ["領域", "項目", "🧭"] {
            assert!(!symbol.name.contains(authored), "{}", symbol.name);
        }
    }
}

#[test]
fn static_ref_projection_resolves_alias_without_runtime_marker() {
    let source = SourceFile::new(
        FileId(1), None, None,
        "import { style } from 'stillwind/react'; import { primary as p } from './refs'; export const x = style(p.asAnchor());",
    )
    .unwrap();
    let receiver: u32 = source
        .text()
        .rfind("p.asAnchor()")
        .unwrap()
        .try_into()
        .unwrap();
    let reference = IdentityId::new(AssetKind::Ref, ModuleId::from("refs.tsx"), 6, 13, "primary");
    let facts = [TypeFact {
        start: receiver,
        end: receiver + 1,
        value: ExportedValue::Refs(vec![RefAlternative::new(reference, None).unwrap()]),
    }];
    let theme = SourceFile::new(
        FileId(2),
        None,
        None,
        "@theme {} @stillwind-slot utilities; @stillwind-slot keyframes;",
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
    let code = result.apply_to_source(&source).unwrap();
    let value = code.split("export const x = ").nth(1).unwrap();
    assert!(!value.contains("(p).c"), "{value}");
    assert!(result.css.contains("anchor-name:"));
    assert!(!value.contains("primary"), "{value}");
}

#[test]
fn static_ref_projection_resolves_member_without_runtime_marker() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        "import { style } from 'stillwind/react'; export const x = style(holder.trigger.asAnchor());",
    )
    .unwrap();
    let receiver: u32 = source
        .text()
        .rfind("holder.trigger.asAnchor()")
        .unwrap()
        .try_into()
        .unwrap();
    let reference = IdentityId::new(
        AssetKind::Ref,
        ModuleId::from("holder.tsx"),
        6,
        13,
        "trigger",
    );
    let facts = [TypeFact {
        start: receiver,
        end: receiver + u32::try_from("holder.trigger".len()).unwrap(),
        value: ExportedValue::Refs(vec![RefAlternative::new(reference, None).unwrap()]),
    }];
    let theme = SourceFile::new(
        FileId(2),
        None,
        None,
        "@theme {} @stillwind-slot utilities; @stillwind-slot keyframes;",
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
    let code = result.apply_to_source(&source).unwrap();
    let value = code.split("export const x = ").nth(1).unwrap();
    assert!(!value.contains("(holder.trigger).c"), "{value}");
    assert!(result.css.contains("anchor-name:"));
}

#[test]
fn dynamic_ref_projection_reads_the_receiver_and_plans_each_candidate() {
    let source = SourceFile::new(
        FileId(1),
        None,
        None,
        "import { style } from 'stillwind/react'; export const x = style(choice.asAnchor());",
    )
    .unwrap();
    let receiver: u32 = source
        .text()
        .find("choice.asAnchor()")
        .unwrap()
        .try_into()
        .unwrap();
    let refs = ["first", "second"]
        .into_iter()
        .map(|name| {
            RefAlternative::new(
                IdentityId::new(AssetKind::Ref, ModuleId::from("refs.tsx"), 6, 13, name),
                None,
            )
            .unwrap()
        })
        .collect();
    let facts = [TypeFact {
        start: receiver,
        end: receiver + u32::try_from("choice".len()).unwrap(),
        value: ExportedValue::Refs(refs),
    }];
    let theme = SourceFile::new(
        FileId(2),
        None,
        None,
        "@theme {} @stillwind-slot utilities; @stillwind-slot keyframes;",
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
    let code = result.apply_to_source(&source).unwrap();
    assert!(code.contains("(choice).c"), "{code}");
    assert!(code.contains("})[(choice).c]"), "{code}");
    assert_eq!(
        result.css.matches(":where(.sw_").count(),
        2,
        "{}",
        result.css
    );
}

#[test]
fn explicit_ref_uses_separate_attachment_gate_and_scope() {
    let (source, result) = compile(
        "const r=style.ref(); const a=style.ref(); export const x=style(r,r.asAnchor([a]),r.asContainer()); export const y=style(r.positionAnchor()); export const z=style(style.anchorScope([a]));",
        "",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(!result.css.contains("container-type:"));
    assert_eq!(result.css.matches("anchor-name:").count(), 1);
    assert_eq!(result.css.matches("anchor-scope:").count(), 2);
    assert_eq!(result.css.matches(":where(").count(), 2);
    let code = result.apply_to_source(&source).unwrap();
    assert!(!code.contains("(r).c"), "{code}");
    assert!(
        result.lint_findings.is_empty(),
        "{:?}",
        result.lint_findings
    );
}

#[test]
fn scope_lists_warn_for_empty_and_duplicate_identities() {
    let (_, result) = compile(
        "const a=style.ref();const x=style(a,a.asAnchor([]),style.anchorScope([a,a]));",
        "",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        result
            .lint_findings
            .iter()
            .filter(|f| matches!(f.diagnostic, LintDiagnostic::AnchorScopeList { .. }))
            .count(),
        2
    );
    assert_eq!(result.css.matches("anchor-scope:").count(), 1);
    let (_, omitted) = compile(
        "const a=style.ref();const x=style(a,a.asAnchor(),a.asContainer());",
        "",
    );
    assert!(!omitted.css.contains("anchor-scope:"));
    assert!(!omitted.css.contains("container-type:"));
    let (_, explicit) = compile(
        "const a=style.ref();const x=style(a,a.asAnchor('none'),a.asContainer('normal'));",
        "",
    );
    assert!(explicit.css.contains("anchor-scope:none"));
    assert!(explicit.css.contains("container-type:normal"));
}

#[test]
fn container_type_accepts_css_keyword_combinations_and_rejects_invalid_values() {
    for value in [
        "normal",
        "size",
        "inline-size",
        "scroll-state",
        "size scroll-state",
        "inline-size scroll-state",
        "scroll-state size",
        "scroll-state inline-size",
        "initial",
        "inherit",
        "unset",
        "revert",
        "revert-layer",
        "revert-rule",
    ] {
        let (_, result) = compile(
            &format!("const r=style.ref();const x=style(r,r.asContainer('{value}'));"),
            "",
        );
        assert!(
            result.diagnostics.is_empty(),
            "{value}: {:?}",
            result.diagnostics
        );
        assert!(!result.css.contains("container-type:scroll-state size"));
    }
    for value in [
        "normal scroll-state",
        "size inline-size",
        "size size",
        "var(--type)",
    ] {
        let (_, result) = compile(
            &format!("const r=style.ref();const x=style(r,r.asContainer('{value}'));"),
            "",
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|error| error.diagnostic == CompileDiagnostic::InvalidContainerType),
            "{value}"
        );
    }
}

#[test]
fn provider_lints_distinguish_missing_attachment_and_property_conflicts() {
    let (_, missing) = compile("const r=style.ref();const x=style(r.asAnchor());", "");
    assert!(
        missing
            .lint_findings
            .iter()
            .any(|f| matches!(f.diagnostic, LintDiagnostic::MissingRefAttachment { .. }))
    );
    let (_, conflict) = compile(
        "const r=style.ref();const x=style(r,r.asContainer('size'),'[container-type:normal]');",
        "",
    );
    assert!(
        conflict
            .lint_findings
            .iter()
            .any(|f| matches!(f.diagnostic, LintDiagnostic::ProviderConflict { .. })),
        "{:?}",
        conflict.lint_findings
    );
    let (_, omitted) = compile(
        "const r=style.ref();const x=style(r,r.asContainer(),'[container-type:normal]');",
        "",
    );
    assert!(
        !omitted
            .lint_findings
            .iter()
            .any(|f| matches!(f.diagnostic, LintDiagnostic::ProviderConflict { .. }))
    );
    let (_, duplicate) = compile(
        "const r=style.ref();const x=style(r,r.asContainer('size'),r.asContainer('normal'));",
        "",
    );
    assert!(
        duplicate
            .lint_findings
            .iter()
            .any(|f| matches!(f.diagnostic, LintDiagnostic::ProviderConflict { .. }))
    );
}

#[test]
fn exclusive_provider_marker_gates_do_not_conflict() {
    let (_, result) = compile(
        "const a=style.ref();const b=style.ref();const x=style(flag?a:b,a.asContainer('size'),b.asContainer('normal'));",
        "",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        !result.lint_findings.iter().any(|f| matches!(
            f.diagnostic,
            LintDiagnostic::ProviderConflict { .. } | LintDiagnostic::MissingRefAttachment { .. }
        )),
        "{:?}",
        result.lint_findings
    );
}

#[test]
fn dynamic_scope_includes_every_candidate_without_runtime_lookup() {
    let (source, result) = compile(
        "const a=style.ref();const b=style.ref();const x=style(style.anchorScope([flag?a:b]));const y=style((flag?a:b).positionAnchor());",
        "",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let scope = result
        .css
        .split("anchor-scope:")
        .nth(1)
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    assert_eq!(scope.split(',').count(), 2, "{scope}");
    for name in scope.split(',') {
        assert!(result.css.contains(&format!("position-anchor:{name};")));
    }
    let code = result.apply_to_source(&source).unwrap();
    let x = code
        .split("const x=")
        .nth(1)
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    assert!(!x.contains("flag"), "{x}");
    assert!(!result.css.contains(":where("));
}
