//! Ref projections and selector scopes.
//!
//! Pins that one declaration projects into anchor, position-anchor, anchor-scope, and
//! container names sharing one identity, that named container queries emit rules for
//! the ref's target, and that scope refs get readable marker names.

use stillwind_compiler::CompilerOptions;
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, ModuleId};
use stillwind_test_support::assertions::CssAssertions as _;
use stillwind_test_support::css;

use super::*;

#[test]
fn consumer_inside_variant_keeps_variant_without_marker_gate() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const trigger = style.ref();
export const item = style({ "hover:": trigger.positionAnchor() });"#,
    )
    .unwrap();
    let theme = css::source(FileId(2), "@theme {}").unwrap();
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
export const item = style(enabled && ["p-4", anchor.positionAnchor(), "text-red"]);"#,
    )
    .unwrap();
    let theme = css::source(FileId(2), "@theme {}").unwrap();
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
    assert_eq!(result.classes.len(), 1, "{:?}", result.classes);
    let class = &result.classes[0].class_name;
    assert!(
        result.css.contains(&format!(".{class}{{")),
        "{}",
        result.css
    );
    assert!(
        !result.css.contains(&format!(".{class}:where(.sw_")),
        "{}",
        result.css
    );
    assert!(result.css.contains("position-anchor:--sw-a-"));
    assert!(result.css.contains("color:"));
    let output = result.transforms[0].replacement.as_str();
    assert!(output.contains("enabled"), "{output}");
    assert!(!output.contains("(anchor).c"), "{output}");
    assert!(!output.contains("[(anchor).c]"), "{output}");
}

#[test]
fn unconditional_projection_keeps_class_and_custom_property_output() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const anchor = style.ref();
export const item = style("flex", anchor.positionAnchor(), { "--x": size });"#,
    )
    .unwrap();
    let theme = css::source(FileId(2), "@theme {}").unwrap();
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
    assert_eq!(result.classes.len(), 1, "{:?}", result.classes);
    let output = result.transforms[0].replacement.as_str();
    assert!(output.contains("className:"), "{output}");
    assert!(output.contains("style:{"), "{output}");
}

#[test]
fn projects_one_ref_into_anchor_position_and_container_names() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const popover = style.ref();
const root = style(popover, style.anchorScope([popover]), popover.asContainer("inline-size"));
const trigger = style(popover, popover.asAnchor(), "relative");
const panel = style(popover.positionAnchor(), "absolute");"#,
    )
    .unwrap();
    let theme = css::source(FileId(2), "@theme {}").unwrap();

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
            .filter(
                |symbol| symbol.kind == stillwind_compiler::GeneratedSymbolKind::Ref
                    || symbol.kind == stillwind_compiler::GeneratedSymbolKind::Container
            )
            .count(),
        2,
        "{:?}",
        result.symbols
    );
    assert!(result.css.contains_css("container-type:inline-size;"));
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(!transformed.contains("style.ref()"), "{transformed}");
    // Static uses and attachments need no runtime identity record.
    assert!(
        transformed.contains("const popover = undefined"),
        "{transformed}"
    );
    assert!(!transformed.contains("(popover).c"), "{transformed}");
    assert!(!transformed.contains("})[(popover).c]"), "{transformed}");
}

#[test]
fn emits_named_container_rules_from_ref_queries() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const card = style.ref();
const root = style(card, card.asContainer("inline-size"));
const content = style({[card.at("width >= 40rem")]: "grid-cols-2"});"#,
    )
    .unwrap();
    let theme = css::source(FileId(2), "@theme {}").unwrap();

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
fn empty_ref_queries_emit_only_the_container_name() {
    for query in ["card.at()", "card.at('')", "card.at``"] {
        for input in [
            format!("{{[{query}]: {{'hover:': 'grid-cols-2'}}}}"),
            format!("`${{{query}}}:hover:grid-cols-2`"),
        ] {
            let source = SourceFile::new(
                FileId(1),
                None,
                None,
                format!(
                    "import {{style}} from 'stillwind/react'; const card = style.ref(); const root = style(card, card.asContainer('size')); const x = style({input});"
                ),
            )
            .unwrap();
            let theme = css::source(FileId(2), "@theme {}").unwrap();
            let result = Compiler::compile_source(
                &OxcSourceAnalyzer,
                &source,
                &ModuleId::from("test.tsx"),
                ScriptLanguage::Tsx,
                ScriptModuleKind::Unambiguous,
                &theme,
                &CssSourceName::from("stillwind.css"),
                &CompilerOptions::default(),
            );
            assert!(result.diagnostics.is_empty(), "{input}: {result:#?}");
            assert!(result.lint_findings.is_empty(), "{input}: {result:#?}");
            let name = result
                .css
                .split("container-name:")
                .nth(1)
                .and_then(|rest| rest.split(';').next())
                .expect("container declaration");
            assert!(
                result.css.contains(&format!("@container {name}{{")),
                "{input}: {}",
                result.css
            );
            assert!(result.css.contains(":hover"), "{input}: {}", result.css);
            assert!(
                result
                    .css
                    .contains_css("grid-template-columns:repeat(2,minmax(0,1fr));"),
                "{input}: {}",
                result.css
            );
        }
    }
}

#[test]
fn container_query_variants_support_calls_tags_and_nested_wrappers() {
    for input in [
        "{[card.at('width >= 40rem')]: {'hover:': 'grid-cols-2'}}",
        "{[card.at`width >= 40rem`]: {'hover:': 'grid-cols-2'}}",
        "`${card.at('width >= 40rem')}:hover:grid-cols-2`",
    ] {
        let source = SourceFile::new(FileId(1), None, None, format!(
            "import {{style}} from 'stillwind/react'; const card = style.ref(); const root = style(card, card.asContainer('size')); const x = style({input});"
        )).unwrap();
        let theme = css::source(FileId(2), "@theme {}").unwrap();
        let result = Compiler::compile_source(
            &OxcSourceAnalyzer,
            &source,
            &ModuleId::from("test.tsx"),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
            &theme,
            &CssSourceName::from("stillwind.css"),
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
const parent = style(trigger, trigger.asAnchor(), "inline-flex");
const child = style({ [trigger`hover:`]: "text-accent" });"#,
    )
    .unwrap();
    let theme = css::source(FileId(2), "@theme { --color-accent: red; }").unwrap();

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
    let marker = "sw_";
    assert!(result.transforms[0].replacement.contains(marker));
    assert!(result.css.contains(&format!(".{marker}")));
    assert!(result.css.contains(":hover) *)"));
}

#[test]
fn scopes_select_refs_across_siblings_and_use_readable_marker_names() {
    let component = SourceFile::new(
        FileId(1), None, None,
        r#"import { style } from "stillwind/react";
const menu = style.ref("menu");
const trigger = style.ref("trigger");
export const Menu = () => <section {...style(menu)}><button {...style(trigger, "inline-flex")} /><span {...style({ [trigger`hover:`.in(menu)]: "text-accent" })} /></section>;"#,
    ).unwrap();
    let theme = css::source(FileId(2), "@theme { --color-accent: red; }").unwrap();

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
    let transformed = result.apply_to_source(&component).unwrap();
    // A marker spells its declaration's normalized label, which is what makes it
    // readable in generated markup.
    let marker_of = |prefix: &str| {
        result
            .symbols
            .iter()
            .find(|symbol| {
                symbol.kind == stillwind_compiler::GeneratedSymbolKind::RefMarker
                    && symbol.name.starts_with(prefix)
            })
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
        result.css.contains(&format!(":has(:is(:where(.{trigger}")),
        "{}",
        result.css
    );
    assert!(result.css.contains(":hover)) *)"), "{}", result.css);
}

#[test]
fn one_ref_is_both_a_selector_subject_and_a_scope() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const menu = style.ref("menu");
const trigger = style.ref("trigger");
export const Menu = () => (
  <section {...style(menu)}>
    <button {...style(trigger)}>Open</button>
    <div {...style({ [trigger`hover:`.in(menu)]: "block" })}>Menu</div>
    <p {...style({ [menu`focus-within:`]: "ring-2" })}>Hint</p>
  </section>
);"#,
    )
    .unwrap();
    let theme = css::source(FileId(2), "@theme {}").unwrap();

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
    assert!(
        result.lint_findings.is_empty(),
        "{:#?}",
        result.lint_findings
    );
    // The menu condition selects targets by ancestry.
    assert!(result.css.contains(":focus-within) *)"), "{}", result.css);
    // The trigger condition selects siblings within the menu scope.
    assert!(
        result.css.contains(":has(:is(:where(.sw_trigger_"),
        "{}",
        result.css
    );
}

#[test]
fn a_scope_ref_can_have_an_ancestor_scope() {
    let component = SourceFile::new(
        FileId(1),
        None,
        None,
        r#"import { style } from "stillwind/react";
const outer = style.ref("outer");
const inner = style.ref("inner");
const leaf = style.ref("leaf");
export const View = () => (
  <section {...style(outer)}>
    <div {...style(inner)}>
      <span {...style(leaf)} />
      <b {...style({ [leaf`hover:`.in(inner).in(outer)]: "block" })} />
    </div>
  </section>
);"#,
    )
    .unwrap();
    let theme = css::source(FileId(2), "@theme {}").unwrap();

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
    assert!(
        result.lint_findings.is_empty(),
        "{:#?}",
        result.lint_findings
    );
    // The leaf condition selects targets within the inner scope.
    assert!(result.css.contains(".sw_inner_"), "{}", result.css);
    assert!(
        result.css.contains(":has(:is(:where(.sw_leaf_"),
        "{}",
        result.css
    );
}
