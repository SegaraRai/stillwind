//! Ref and subtree projection.
//!
//! Pins that one declaration projects into anchor, position-anchor, anchor-scope, and
//! container names sharing one identity, that named container queries emit rules for
//! the ref's target, that subtrees get readable marker names, and that a selector which
//! cannot hold the required ancestry is rejected.

use stillwind_diagnostics::LintKind;
use stillwind_js_oxc::OxcSourceAnalyzer;

use super::*;

#[test]
fn projects_one_ref_into_anchor_position_and_container_names() {
    let component = SourceFile::new(
        FileId(1),
        "src/Popover.tsx",
        r#"import { style } from "stillwind/react";
const popover = style.ref();
const root = style(popover.anchorScope, popover.container("inline-size"));
const trigger = style(popover.anchor, "relative");
const panel = style(popover.positionAnchor, "absolute");"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "stillwind.css", "@theme {}").unwrap();

    let result = Compiler::compile_source(&OxcSourceAnalyzer, &component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("anchor-name:--sw-r-"));
    assert!(result.css.contains("position-anchor:--sw-r-"));
    assert!(result.css.contains("anchor-scope:--sw-r-"));
    // The anchor and container names are separate families: both are written for the
    // ref's identity, and each keeps its own namespace.
    assert!(result.css.contains("container-name:--sw-s-"));
    let hash_of = |prefix: &str| {
        let start = result.css.find(prefix).expect("declaration") + prefix.len();
        let rest = &result.css[start..];
        let end = rest.find([';', ':']).unwrap_or(rest.len());
        rest[..end].to_owned()
    };
    let anchor = hash_of("anchor-name:--sw-r-");
    let container = hash_of("container-name:--sw-s-");
    assert!(!anchor.is_empty() && !container.is_empty());
    // One declaration owns both names, so the marker reaches each of them.
    assert_eq!(
        result
            .symbols
            .iter()
            .filter(|symbol| symbol.kind == "ref" || symbol.kind == "container")
            .count(),
        2,
        "{:?}",
        result.symbols
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
fn reports_a_position_anchor_use_without_an_anchor_provider() {
    let component = SourceFile::new(
        FileId(1),
        "src/Popover.tsx",
        r#"import { style } from "stillwind/react";
const popover = style.ref();
const panel = style(popover.positionAnchor);"#,
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
fn subtrees_refs_across_siblings_and_uses_readable_marker_names() {
    let component = SourceFile::new(
        FileId(1),
        "src/Menu.tsx",
        r#"import { style } from "stillwind/react";
const menu = style.subtree("menu");
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
    // A marker spells its declaration's normalized label, which is what makes it
    // readable in generated markup.
    let marker_of = |kind: &str| {
        result
            .symbols
            .iter()
            .find(|symbol| symbol.kind == kind)
            .unwrap_or_else(|| panic!("missing {kind}: {:?}", result.symbols))
            .name
            .clone()
    };
    let subtree = marker_of("scope-marker");
    let trigger = marker_of("ref-marker");
    assert!(subtree.starts_with("s_menu_"), "{subtree}");
    assert!(trigger.starts_with("s_trigger_"), "{trigger}");
    assert!(transformed.contains(&subtree), "{transformed}");
    assert!(transformed.contains(&trigger), "{transformed}");
    assert!(
        result.css.contains(&format!(".{subtree}")),
        "{}",
        result.css
    );
    assert!(
        result.css.contains(&format!(":has(.{trigger}")),
        "{}",
        result.css
    );
    assert!(
        result.css.contains(":where(:hover)) ._s_"),
        "{}",
        result.css
    );
}

#[test]
fn rejects_a_scoped_ref_selector_target_outside_its_subtree() {
    let component = SourceFile::new(
        FileId(1),
        "src/Menu.tsx",
        r#"import { style } from "stillwind/react";
const menu = style.subtree();
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
            .any(|item| matches!(item.kind, LintKind::SelectorOutsideSubtree { .. })),
        "{:#?}",
        result.lint_findings
    );
}

#[test]
fn rejects_a_subtree_attached_outside_its_ref_ancestry() {
    let component = SourceFile::new(
        FileId(1),
        "src/Menu.tsx",
        r#"import { style } from "stillwind/react";
const menu = style.subtree();
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
            .any(|item| matches!(item.kind, LintKind::MissingSubtreeAncestor { .. })),
        "{:#?}",
        result.lint_findings
    );
}
