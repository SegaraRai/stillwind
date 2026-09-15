//! Ref and scope projection.
//!
//! Pins that one declaration projects into anchor, position, scope, and container names sharing one
//! identity, that named container queries emit rules for the ref's target, that scopes get readable
//! marker names, and that a selector which cannot hold the required ancestry is rejected.

use stillwind_diagnostics::LintKind;
use stillwind_js_oxc::OxcSourceAnalyzer;

use super::*;

#[test]
fn projects_one_ref_into_anchor_position_scope_and_container_css() {
    let component = SourceFile::new(
        FileId(1),
        "src/Popover.tsx",
        r#"import { style } from "stillwind/react";
const popover = style.ref();
const root = style(popover.anchorScope, popover.container("inline-size"));
const trigger = style(popover.anchor, "relative");
const panel = style(popover.position, "absolute");"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("anchor-name:--sw-r-"));
    assert!(result.css.contains("position-anchor:--sw-r-"));
    assert!(result.css.contains("anchor-scope:--sw-r-"));
    // The anchor and container names are separate families of the same identity, so
    // the ref marker reaches both through one declaration without sharing a spelling.
    assert!(result.css.contains("container-name:--sw-s-"));
    let hash_of = |prefix: &str| {
        let start = result.css.find(prefix).expect("declaration") + prefix.len();
        let rest = &result.css[start..];
        let end = rest.find([';', ':']).unwrap_or(rest.len());
        rest[..end].to_owned()
    };
    assert_eq!(
        hash_of("anchor-name:--sw-r-"),
        hash_of("container-name:--sw-s-")
    );
    assert!(result.css.contains_css("container-type:inline-size;"));
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(!transformed.contains("style.ref()"), "{transformed}");
    // The projection reads the declaration's canonical token, so the declaration stays a
    // value instead of being erased as unused.
    assert!(
        transformed.contains("const popover = {c:\"s_"),
        "{transformed}"
    );
    assert!(
        transformed.contains("({\"s_") && transformed.contains("})[popover.c]"),
        "{transformed}"
    );
}

#[test]
fn reports_a_position_ref_without_an_anchor_provider() {
    let component = SourceFile::new(
        FileId(1),
        "src/Popover.tsx",
        r#"import { style } from "stillwind/react";
const popover = style.ref();
const panel = style(popover.position);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(
        result
            .lint_findings
            .iter()
            .any(|item| matches!(item.kind, LintKind::MissingAnchor { .. }))
    );
    assert!(!result.css.is_empty());
}

#[test]
fn emits_named_container_rules_from_ref_queries() {
    let component = SourceFile::new(
        FileId(1),
        "src/Card.tsx",
        r#"import { style } from "stillwind/react";
const card = style.ref();
const root = style(card.container("inline-size"));
const content = style({[card.at("width >= 40rem")]: "grid-cols-2"});"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("@container --sw-r-"));
    assert!(result.css.contains("(width >= 40rem){"));
    assert!(
        result
            .css
            .contains_css("grid-template-columns:repeat(2,minmax(0,1fr));")
    );
}

#[test]
fn container_query_variants_support_calls_tags_and_nested_wrappers() {
    for input in [
        "{[card.at('width >= 40rem')]: {'hover:': 'grid-cols-2'}}",
        "{[card.at`width >= 40rem`]: {'hover:': 'grid-cols-2'}}",
        "`${card.at('width >= 40rem')}:hover:grid-cols-2`",
    ] {
        let source = SourceFile::new(FileId(1), "entry.tsx", format!(
            "import {{style}} from 'stillwind/react'; const card = style.ref(); const root = style(card.container('size')); const x = style({input});"
        )).unwrap();
        let theme = crate::css_fixture::source(FileId(2), "theme.css", "@theme {}").unwrap();
        let result = Compiler::compile_source(&OxcSourceAnalyzer, &source, &theme);
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        assert!(result.lint_findings.is_empty(), "{result:#?}");
        assert!(result.css.contains("@container --sw-r-"), "{}", result.css);
        assert!(result.css.contains(":hover"), "{}", result.css);
    }
}

#[test]
fn uses_ref_tagged_selectors_as_automatically_named_parent_groups() {
    let component = SourceFile::new(
        FileId(1),
        "src/Menu.tsx",
        r#"import { style } from "stillwind/react";
const trigger = style.ref();
const parent = style(trigger.anchor, "inline-flex");
const child = style({ [trigger`hover:`]: "text-accent" });"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        "@theme { --color-accent: red; }",
    )
    .unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let marker = "s_";
    assert!(result.transforms[0].replacement.contains(marker));
    assert!(result.css.contains(&format!(".{marker}")));
    assert!(result.css.contains(":where(:hover) ._s_"));
}

#[test]
fn scopes_refs_across_siblings_and_uses_readable_marker_names() {
    let component = SourceFile::new(
        FileId(1),
        "src/Menu.tsx",
        r#"import { style } from "stillwind/react";
const menu = style.scope("menu");
const trigger = style.ref(menu, "trigger");
export const Menu = () => <section {...style(menu)}><button {...style(trigger, "inline-flex")} /><span {...style({ [trigger`hover:`]: "text-accent" })} /></section>;"#,
    ).unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        "@theme { --color-accent: red; }",
    )
    .unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(transformed.contains("s_menu_"));
    assert!(transformed.contains("s_trigger_"));
    assert!(result.css.contains(".s_menu_"));
    assert!(result.css.contains(":has(.s_trigger_"));
    assert!(result.css.contains(":where(:hover)) ._s_"));
    assert!(
        result
            .symbols
            .iter()
            .any(|symbol| symbol.kind == "scope-marker" && symbol.name.starts_with("s_menu_"))
    );
    assert!(
        result
            .symbols
            .iter()
            .any(|symbol| symbol.kind == "ref-marker" && symbol.name.starts_with("s_trigger_"))
    );
}

#[test]
fn rejects_a_scoped_ref_selector_target_outside_its_scope() {
    let component = SourceFile::new(
        FileId(1),
        "src/Menu.tsx",
        r#"import { style } from "stillwind/react";
const menu = style.scope();
const trigger = style.ref(menu);
export const Menu = () => <><section {...style(menu)}><button {...style(trigger)} /></section><span {...style({ [trigger`hover:`]: "text-accent" })} /></>;"#,
    ).unwrap();
    let theme = crate::css_fixture::source(
        FileId(2),
        "stillwind.css",
        "@theme { --color-accent: red; }",
    )
    .unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(
        result
            .lint_findings
            .iter()
            .any(|item| matches!(item.kind, LintKind::SelectorOutsideScope { .. })),
        "{:#?}",
        result.lint_findings
    );
}

#[test]
fn rejects_a_scope_attached_outside_its_ref_ancestry() {
    let component = SourceFile::new(
        FileId(1),
        "src/Menu.tsx",
        r#"import { style } from "stillwind/react";
const menu = style.scope();
const trigger = style.ref(menu);
export const Menu = () => <><section {...style(menu)} /><button {...style(trigger)} /></>;"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(
        result
            .lint_findings
            .iter()
            .any(|item| matches!(item.kind, LintKind::MissingScopeAncestor { .. })),
        "{:#?}",
        result.lint_findings
    );
}
