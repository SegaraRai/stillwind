//! Ref projection and boundaries.
//!
//! Pins that one declaration projects into anchor, position-anchor, anchor-scope, and
//! container names sharing one identity, that named container queries emit rules for
//! the ref's target, that boundary refs get readable marker names, and that a selector
//! which cannot hold the required ancestry is rejected.

use stillwind_compiler::CompilerOptions;
use stillwind_diagnostics::LintKind;
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;

use super::*;

#[test]
fn ref_projection_inside_variant_keeps_the_selector_gate() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const trigger = style.ref();
export const item = style({ "hover:": trigger.positionAnchor });"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme {}").unwrap();
    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        "test.tsx",
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        "stillwind.css",
        &CompilerOptions::default(),
    );
    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result.css.contains("position-anchor:--sw-a-"),
        "{}",
        result.css
    );
    assert!(result.css.contains(":hover"), "{}", result.css);
}

#[test]
fn conditional_region_collects_utilities_and_ref_projection_into_one_class() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const anchor = style.ref();
export const item = style(enabled && ["p-4", anchor.positionAnchor, "text-red"]);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme {}").unwrap();
    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        "test.tsx",
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        "stillwind.css",
        &CompilerOptions::default(),
    );
    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.classes.len(), 1, "{:?}", result.classes);
    let class = &result.classes[0].class_name;
    assert!(
        result.css.contains(&format!(".{class}{{padding:")),
        "{}",
        result.css
    );
    assert!(
        result.css.contains(&format!(".{class}:where(.sw_")),
        "{}",
        result.css
    );
    assert!(result.css.contains("position-anchor:--sw-a-"));
    assert!(result.css.contains("color:"));
    let output = result.transforms[0].replacement.as_str();
    assert!(output.contains("stillwindClsx("), "{output}");
    assert!(!output.contains("stillwindMerge("), "{output}");
    assert!(output.contains("enabled"), "{output}");
    assert!(output.contains("(anchor).c"), "{output}");
    assert!(!output.contains("[(anchor).c]"), "{output}");
}

#[test]
fn unconditional_projection_and_custom_property_materialize_direct_props() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const anchor = style.ref();
export const item = style("flex", anchor.positionAnchor, { "--x": size });"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme {}").unwrap();
    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        "test.tsx",
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        "stillwind.css",
        &CompilerOptions::default(),
    );
    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.classes.len(), 1, "{:?}", result.classes);
    let output = result.transforms[0].replacement.as_str();
    assert!(output.contains("className:"), "{output}");
    assert!(output.contains("style:{"), "{output}");
    assert!(!output.contains("stillwindMerge("), "{output}");
}

#[test]
fn projects_one_ref_into_anchor_position_and_container_names() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const popover = style.ref();
const root = style(popover.anchorScope, popover.container("inline-size"));
const trigger = style(popover.anchor, "relative");
const panel = style(popover.positionAnchor, "absolute");"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme {}").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        "test.tsx",
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        "stillwind.css",
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("anchor-name:--sw-a-"));
    assert!(result.css.contains("position-anchor:--sw-a-"));
    assert!(result.css.contains("anchor-scope:--sw-a-"));
    // The anchor and container names are separate families: both are written for the
    // ref's identity, and each keeps its own namespace.
    assert!(result.css.contains("container-name:sw-c-"));
    let hash_of = |prefix: &str| {
        let start = result.css.find(prefix).expect("declaration") + prefix.len();
        let rest = &result.css[start..];
        let end = rest.find([';', ':']).unwrap_or(rest.len());
        rest[..end].to_owned()
    };
    let anchor = hash_of("anchor-name:--sw-a-");
    let container = hash_of("container-name:sw-c-");
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
        transformed.contains("const popover = {c:\"sw_"),
        "{transformed}"
    );
    assert!(transformed.contains("(popover).c"), "{transformed}");
    assert!(!transformed.contains("})[(popover).c]"), "{transformed}");
}

#[test]
fn reports_a_position_anchor_use_without_an_anchor_provider() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const popover = style.ref();
const panel = style(popover.positionAnchor);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme {}").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        "test.tsx",
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        "stillwind.css",
        &CompilerOptions::default(),
    );

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
        None,
        None,
        r#"import { style } from "stillwind/react";
const card = style.ref();
const root = style(card.container("inline-size"));
const content = style({[card.at("width >= 40rem")]: "grid-cols-2"});"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme {}").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        "test.tsx",
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        "stillwind.css",
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    // The container projection writes the name and the query reads the same one back,
    // so a ref at() query can only match the container it declared.
    let name = result
        .css
        .split("container-name:")
        .nth(1)
        .and_then(|rest| rest.split(';').next())
        .expect("container declaration");
    assert!(name.starts_with("sw-c-"), "{name}");
    assert!(
        result
            .css
            .contains(&format!("@container {name} (width >= 40rem){{")),
        "{}",
        result.css
    );
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
        let source = SourceFile::new(FileId(1), None, None, format!(
            "import {{style}} from 'stillwind/react'; const card = style.ref(); const root = style(card.container('size')); const x = style({input});"
        )).unwrap();
        let theme = crate::css_fixture::source(FileId(2), "@theme {}").unwrap();
        let result = Compiler::compile_source(
            &OxcSourceAnalyzer,
            &source,
            "test.tsx",
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
            &theme,
            "stillwind.css",
            &CompilerOptions::default(),
        );
        assert!(result.diagnostics.is_empty(), "{result:#?}");
        assert!(result.lint_findings.is_empty(), "{result:#?}");
        assert!(result.css.contains("@container sw-c-"), "{}", result.css);
        assert!(result.css.contains(":hover"), "{}", result.css);
    }
}

#[test]
fn uses_ref_tagged_selectors_as_automatically_named_parent_groups() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const trigger = style.ref();
const parent = style(trigger.anchor, "inline-flex");
const child = style({ [trigger`hover:`]: "text-accent" });"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme { --color-accent: red; }").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        "test.tsx",
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        "stillwind.css",
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let marker = "sw_";
    assert!(result.transforms[0].replacement.contains(marker));
    assert!(result.css.contains(&format!(".{marker}")));
    assert!(result.css.contains(":where(:hover) .sw_"));
}

#[test]
fn boundaries_place_refs_across_siblings_and_use_readable_marker_names() {
    let component = SourceFile::new(
        FileId(1), None, None,
        r#"import { style } from "stillwind/react";
const menu = style.ref("menu");
const trigger = style.ref(menu, "trigger");
export const Menu = () => <section {...style(menu)}><button {...style(trigger, "inline-flex")} /><span {...style({ [trigger`hover:`]: "text-accent" })} /></section>;"#,
    ).unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme { --color-accent: red; }").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        "test.tsx",
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        "stillwind.css",
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let transformed = result.apply_to_source(&component).unwrap();
    // A marker spells its declaration's normalized label, which is what makes it
    // readable in generated markup.
    let marker_of = |prefix: &str| {
        result
            .symbols
            .iter()
            .find(|symbol| symbol.kind == "ref-marker" && symbol.name.starts_with(prefix))
            .unwrap_or_else(|| panic!("missing {prefix}: {:?}", result.symbols))
            .name
            .clone()
    };
    let boundary = marker_of("sw_menu_");
    let trigger = marker_of("sw_trigger_");
    assert!(trigger.starts_with("sw_trigger_"), "{trigger}");
    assert!(transformed.contains(&boundary), "{transformed}");
    assert!(transformed.contains(&trigger), "{transformed}");
    assert!(
        result.css.contains(&format!(".{boundary}")),
        "{}",
        result.css
    );
    assert!(
        result.css.contains(&format!(":has(.{trigger}")),
        "{}",
        result.css
    );
    assert!(
        result.css.contains(":where(:hover)) .sw_"),
        "{}",
        result.css
    );
}

#[test]
fn rejects_a_bounded_ref_selector_target_outside_its_boundary() {
    let component = SourceFile::new(
        FileId(1), None, None,
        r#"import { style } from "stillwind/react";
const menu = style.ref();
const trigger = style.ref(menu);
export const Menu = () => <><section {...style(menu)}><button {...style(trigger)} /></section><span {...style({ [trigger`hover:`]: "text-accent" })} /></>;"#,
    ).unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme { --color-accent: red; }").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        "test.tsx",
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        "stillwind.css",
        &CompilerOptions::default(),
    );

    assert!(
        result
            .lint_findings
            .iter()
            .any(|item| matches!(item.kind, LintKind::SelectorOutsideBoundary { .. })),
        "{:#?}",
        result.lint_findings
    );
}

#[test]
fn rejects_a_boundary_attached_outside_its_ref_ancestry() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const menu = style.ref();
const trigger = style.ref(menu);
export const Menu = () => <><section {...style(menu)} /><button {...style(trigger)} /></>;"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme {}").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        "test.tsx",
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        "stillwind.css",
        &CompilerOptions::default(),
    );

    assert!(
        result
            .lint_findings
            .iter()
            .any(|item| matches!(item.kind, LintKind::MissingBoundaryAncestor { .. })),
        "{:#?}",
        result.lint_findings
    );
}

#[test]
fn one_ref_is_both_a_selector_subject_and_a_boundary() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const menu = style.ref("menu");
const trigger = style.ref(menu, "trigger");
export const Menu = () => (
  <section {...style(menu)}>
    <button {...style(trigger)}>Open</button>
    <div {...style({ [trigger`hover:`]: "block" })}>Menu</div>
    <p {...style({ [menu`focus-within:`]: "ring-2" })}>Hint</p>
  </section>
);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme {}").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        "test.tsx",
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        "stillwind.css",
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result.lint_findings.is_empty(),
        "{:#?}",
        result.lint_findings
    );
    // The boundary element keeps the unbounded relation for its own ref: a target inside
    // the boundary element matches by ancestry.
    assert!(
        result.css.contains(":where(:focus-within) .sw_"),
        "{}",
        result.css
    );
    // The bounded ref reads the boundary instead, so its target may be a sibling.
    assert!(result.css.contains(":has(.sw_trigger_"), "{}", result.css);
}

#[test]
fn a_bounded_ref_can_bound_another_ref() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const outer = style.ref("outer");
const inner = style.ref(outer, "inner");
const leaf = style.ref(inner, "leaf");
export const View = () => (
  <section {...style(outer)}>
    <div {...style(inner)}>
      <span {...style(leaf)} />
      <b {...style({ [leaf`hover:`]: "block" })} />
    </div>
  </section>
);"#,
    )
    .unwrap();
    let theme = crate::css_fixture::source(FileId(2), "@theme {}").unwrap();

    let result = Compiler::compile_source(
        &OxcSourceAnalyzer,
        &component,
        "test.tsx",
        ScriptLanguage::Tsx,
        ScriptModuleKind::Unambiguous,
        &theme,
        "stillwind.css",
        &CompilerOptions::default(),
    );

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result.lint_findings.is_empty(),
        "{:#?}",
        result.lint_findings
    );
    // The leaf is bounded by the inner ref, so its selector reads the inner marker as the
    // boundary even though that marker is itself bounded by the outer ref.
    assert!(result.css.contains(".sw_inner_"), "{}", result.css);
    assert!(result.css.contains(":has(.sw_leaf_"), "{}", result.css);
}
