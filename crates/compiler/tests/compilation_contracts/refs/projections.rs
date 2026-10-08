//! Checks static and runtime projections and explicit CSS roles.

use super::*;

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
        value: ExportedValue::Refs(SymbolicValue::exact(reference)),
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
        value: ExportedValue::Refs(SymbolicValue::exact(reference)),
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
    let refs = SymbolicValue::finite(
        AssetKind::Ref,
        ["first", "second"]
            .into_iter()
            .map(|name| IdentityId::new(AssetKind::Ref, ModuleId::from("refs.tsx"), 6, 13, name)),
    )
    .unwrap();
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
