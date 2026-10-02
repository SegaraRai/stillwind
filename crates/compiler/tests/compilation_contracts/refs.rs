//! Ref names, projections, scope lists, and provider diagnostics.

#[path = "../../../test-support/css.rs"]
mod css_fixture;

use std::collections::BTreeMap;

use stillwind_compiler::{CompileResult, Compiler, CompilerOptions};
use stillwind_diagnostics::{CompileDiagnostic, LintDiagnostic};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::{OxcSourceAnalyzer, parse};
use stillwind_semantic::{AssetKind, ExportedValue, IdentityId, SymbolicValue, TypeFact};
use stillwind_source::{CssSourceName, FileId, ModuleId, SourceFile};

use super::compile;

fn compile_registry(code: &str, css: &str) -> (SourceFile, CompileResult) {
    let css = css_fixture::source(
        FileId(2),
        format!("@import \"stillwind/builtins.css\"; {css}"),
    )
    .unwrap();
    compile(code, css.text())
}

fn marker(result: &CompileResult, label: &str) -> String {
    result
        .symbols
        .iter()
        .find(|symbol| {
            symbol.kind == "ref-marker" && symbol.name.starts_with(&format!("sw_{label}_"))
        })
        .unwrap_or_else(|| panic!("missing marker for {label}: {:?}", result.symbols))
        .name
        .clone()
}

#[test]
fn scoped_presence_call_tag_and_shorthand_emit_equivalent_conditions() {
    let mut selectors = Vec::new();
    for expression in ["item().in(parent)", "item``.in(parent)", "item.in(parent)"] {
        let (_, result) = compile_registry(
            &format!(
                "const parent=style.ref('parent'); const item=style.ref('item'); const x=style({{[{expression}]: 'a'}});"
            ),
            "@utility a { color:red; }",
        );
        assert!(result.diagnostics.is_empty(), "{expression}: {result:#?}");
        let parent = marker(&result, "parent");
        let item = marker(&result, "item");
        assert!(
            result.css.contains(&format!(":has(:is(:where(.{item})")),
            "{}",
            result.css
        );
        assert!(
            result.css.contains(&format!(":where(.{parent})")),
            "{}",
            result.css
        );
        selectors.push(result.css);
    }
    assert_eq!(selectors[0], selectors[1]);
    assert_eq!(selectors[0], selectors[2]);
}

#[test]
fn stateful_scope_prefixes_apply_to_the_scope_and_ref_elements() {
    let (_, result) = compile_registry(
        "const parent=style.ref('parent'); const item=style.ref('item'); const x=style({[item('hover').in(parent('focus-within'))]: 'active:a'});",
        "@utility a { color:red; }",
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    let parent = marker(&result, "parent");
    let item = marker(&result, "item");
    assert!(
        result
            .css
            .contains(&format!(":where(.{parent}):focus-within")),
        "{}",
        result.css
    );
    assert!(
        result
            .css
            .contains(&format!(":has(:is(:where(.{item}):hover")),
        "{}",
        result.css
    );
    assert!(
        result.css.contains("@media (hover: hover)"),
        "{}",
        result.css
    );
    assert!(result.css.contains(":active:is("), "{}", result.css);
}

#[test]
fn custom_ref_and_scope_alternatives_expand_each_combination() {
    let (_, result) = compile_registry(
        "const parent=style.ref('parent'); const item=style.ref('item'); const x=style({[item('hocus').in(parent('hocus'))]: 'a'});",
        "@variant hocus { &:hover { @slot; } &:focus { @slot; } } @utility a { color:red; }",
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    let item = marker(&result, "item");
    let parent = marker(&result, "parent");
    for item_state in ["hover", "focus"] {
        for parent_state in ["hover", "focus"] {
            assert!(
                result.css.split('{').any(|selector| selector
                    .contains(&format!(":has(:is(:where(.{item}):{item_state}))"))
                    && selector.contains(&format!(":is(:where(.{parent}):{parent_state})"))),
                "{}",
                result.css
            );
        }
    }
}

#[test]
fn repeated_scopes_conjoin_independently_of_call_order() {
    let mut css = Vec::new();
    for chain in [
        "item().in(inner('focus')).in(outer('active'))",
        "item().in(outer('active')).in(inner('focus'))",
    ] {
        let (_, result) = compile_registry(
            &format!(
                "const outer=style.ref('outer'); const inner=style.ref('inner'); const item=style.ref('item'); const x=style({{[{chain}]: 'a'}});"
            ),
            "@utility a { color:red; }",
        );
        assert!(result.diagnostics.is_empty(), "{chain}: {result:#?}");
        let outer = marker(&result, "outer");
        let inner = marker(&result, "inner");
        let item = marker(&result, "item");
        let selector = result
            .css
            .split('{')
            .find(|part| part.contains(&item))
            .expect("utility selector");
        assert!(
            selector.contains(&format!(":where(.{outer}):active")),
            "{selector}"
        );
        assert!(
            selector.contains(&format!(":where(.{inner}):focus")),
            "{selector}"
        );
        assert!(selector.contains(":has("), "{selector}");
        assert!(!selector.contains(','), "{selector}");
        assert_eq!(
            selector
                .matches(&format!(":has(:is(:where(.{item})"))
                .count(),
            2,
            "{selector}"
        );
        css.push(result.css);
    }
    assert_eq!(css[0], css[1]);
}

#[test]
fn equivalent_finite_scope_conditions_expand_once_when_repeated() {
    for stateful in [false, true] {
        let mut outputs = Vec::new();
        for (count, alternate_expression) in [(1, false), (12, false), (12, true)] {
            let chain = (0..count)
                .map(|index| {
                    let choice = if alternate_expression && index % 2 == 1 {
                        "otherFlag ? t : s"
                    } else {
                        "flag ? s : t"
                    };
                    if stateful {
                        format!(".in(({choice})('focus:hover'))")
                    } else {
                        format!(".in({choice})")
                    }
                })
                .collect::<String>();
            let (_, result) = compile_registry(
                &format!(
                    "const r=style.ref('r'); const s=style.ref('s'); const t=style.ref('t'); const x=style({{[r(){chain}]: 'a'}});"
                ),
                "@utility a { color:red; }",
            );
            assert!(
                result.diagnostics.is_empty(),
                "{count}, {stateful}: {:?}",
                result.diagnostics
            );
            assert_eq!(
                result.css.matches("color:red;").count(),
                2,
                "{count}, {stateful}"
            );
            let r = marker(&result, "r");
            let s = marker(&result, "s");
            let t = marker(&result, "t");
            let selectors = result
                .css
                .split('{')
                .filter(|part| part.contains(&r))
                .collect::<Vec<_>>();
            assert_eq!(selectors.len(), 2);
            for selector in selectors {
                assert_eq!(selector.matches(":has(").count(), 1, "{selector}");
                assert_ne!(selector.contains(&s), selector.contains(&t), "{selector}");
                if stateful {
                    assert!(selector.contains(":focus:hover"), "{selector}");
                }
            }
            let mut css = result.css;
            for (index, class) in result.classes.iter().enumerate() {
                css = css.replace(&class.class_name, &format!("test_class_{index}"));
            }
            outputs.push(css);
        }
        assert_eq!(outputs[0], outputs[1]);
        assert_eq!(outputs[0], outputs[2]);
    }
}

#[test]
fn distinct_finite_scope_conditions_expand_their_cartesian_product() {
    let (_, result) = compile(
        "const r=style.ref('r'); const s=style.ref('s'); const t=style.ref('t'); const u=style.ref('u'); const v=style.ref('v'); const x=style({[r().in(flag ? s : t).in(otherFlag ? u : v)]: 'a'});",
        "@utility a { color:red; }",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.css.matches("color:red;").count(), 4);
    let reference = marker(&result, "r");
    let first_scopes = [marker(&result, "s"), marker(&result, "t")];
    let second_scopes = [marker(&result, "u"), marker(&result, "v")];
    let selectors = result
        .css
        .split('{')
        .filter(|part| part.contains(&reference))
        .collect::<Vec<_>>();
    assert_eq!(selectors.len(), 4);
    for first in &first_scopes {
        for second in &second_scopes {
            let selector = selectors
                .iter()
                .find(|selector| selector.contains(first) && selector.contains(second))
                .expect("scope combination");
            assert_eq!(selector.matches(":has(").count(), 2, "{selector}");
            assert_ne!(
                selector.contains(&first_scopes[0]),
                selector.contains(&first_scopes[1]),
                "{selector}"
            );
            assert_ne!(
                selector.contains(&second_scopes[0]),
                selector.contains(&second_scopes[1]),
                "{selector}"
            );
        }
    }
}

#[test]
fn a_container_query_inside_a_scoped_ref_variant_stays_relative_to_the_target() {
    let (_, result) = compile_registry(
        "const parent=style.ref('parent'); const item=style.ref('item'); const card=style.ref('card'); const root=style(card,card.asContainer('size')); const parentStyle=style(parent); const itemStyle=style(item); const x=style({[item('hover').in(parent('focus'))]: {[card.at('width > 20rem')]: 'a'}});",
        "@utility a { color:red; }",
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    assert!(result.lint_findings.is_empty(), "{result:#?}");
    let name = result
        .css
        .split("container-name:")
        .nth(1)
        .and_then(|part| part.split(';').next())
        .expect("container name");
    assert!(
        result
            .css
            .contains(&format!("@container {name} (width > 20rem){{")),
        "{}",
        result.css
    );
    let item = marker(&result, "item");
    let parent = marker(&result, "parent");
    assert!(
        result
            .css
            .contains(&format!(":has(:is(:where(.{item}):hover")),
        "{}",
        result.css
    );
    assert!(
        result.css.contains(&format!(":where(.{parent}):focus")),
        "{}",
        result.css
    );
}

#[test]
fn scoped_refs_and_container_queries_test_the_final_child_target() {
    let mut outputs = Vec::new();
    for input in [
        "{[item('hover').in(parent)]: {[card.at('width > 20rem')]: '*:a'}}",
        "{[card.at('width > 20rem')]: {[item('hover').in(parent)]: '*:a'}}",
        "{'*:': {[item('hover').in(parent)]: {[card.at('width > 20rem')]: 'a'}}}",
        "`${item('hover').in(parent)}:${card.at('width > 20rem')}:*:a`",
    ] {
        let (_, result) = compile_registry(
            &format!(
                "const parent=style.ref('parent'); const item=style.ref('item'); const card=style.ref('card'); const provider=style(card,card.asContainer('size')); const x=style({input});"
            ),
            "@utility a { color:red; }",
        );
        assert!(result.diagnostics.is_empty(), "{input}: {result:#?}");
        let item = marker(&result, "item");
        let parent = marker(&result, "parent");
        let class = &result.classes.last().expect("target class").class_name;
        assert!(
            result.css.contains(&format!(
                ":is(.{class} > *):is(:is(:where(.{parent})):has(:is(:where(.{item}):hover)) *)"
            )),
            "{input}: {}",
            result.css
        );
        assert!(result.css.contains("@container sw-c-"), "{}", result.css);
        let mut css = result.css;
        for (index, class) in result.classes.iter().enumerate() {
            css = css.replace(&class.class_name, &format!("test_class_{index}"));
        }
        outputs.push(css);
    }
    for output in &outputs[1..] {
        assert_eq!(output, &outputs[0]);
    }
}

#[test]
fn ref_conditions_guard_the_element_that_owns_before_and_after() {
    for (expression, scoped) in [("item('hover')", false), ("item('hover').in(parent)", true)] {
        for pseudo in ["before", "after"] {
            let (_, result) = compile_registry(
                &format!(
                    "const parent=style.ref('parent'); const item=style.ref('item'); const x=style({{[{expression}]: '{pseudo}:a'}});"
                ),
                "@utility a { color:red; }",
            );
            assert!(
                result.diagnostics.is_empty(),
                "{expression}, {pseudo}: {result:#?}"
            );
            let item = marker(&result, "item");
            let guard = if scoped {
                let parent = marker(&result, "parent");
                format!(":is(:is(:where(.{parent})):has(:is(:where(.{item}):hover)) *)")
            } else {
                format!(":is(:is(:where(.{item}):hover) *)")
            };
            let class = &result.classes[0].class_name;
            assert!(
                result.css.contains(&format!(".{class}{guard}::{pseudo}{{")),
                "{}",
                result.css
            );
            assert!(result.css.contains("color:red;"), "{}", result.css);
        }
    }
}

#[test]
fn ref_conditions_guard_the_final_child_before_its_pseudo_element() {
    for (expression, scoped) in [("item('hover')", false), ("item('hover').in(parent)", true)] {
        for pseudo in ["before", "after"] {
            let (_, result) = compile_registry(
                &format!(
                    "const parent=style.ref('parent'); const item=style.ref('item'); const x=style({{[{expression}]: 'child-{pseudo}:a'}});"
                ),
                &format!(
                    "@variant child-{pseudo} {{ & > *::{pseudo} {{ @slot; }} }} @utility a {{ color:red; }}"
                ),
            );
            assert!(
                result.diagnostics.is_empty(),
                "{expression}, {pseudo}: {result:#?}"
            );
            let item = marker(&result, "item");
            let guard = if scoped {
                let parent = marker(&result, "parent");
                format!(":is(:is(:where(.{parent})):has(:is(:where(.{item}):hover)) *)")
            } else {
                format!(":is(:is(:where(.{item}):hover) *)")
            };
            let class = &result.classes[0].class_name;
            assert!(
                result
                    .css
                    .contains(&format!(".{class} > *{guard}::{pseudo}{{")),
                "{}",
                result.css
            );
            assert!(result.css.contains("color:red;"), "{}", result.css);
        }
    }
}

#[test]
fn ref_and_scope_predicates_keep_ancestors_outside_the_scope() {
    let (_, result) = compile_registry(
        "const parent=style.ref('parent'); const item=style.ref('item'); const x=style({[item('[.outside>&]').in(parent('[.shell>&]'))]: 'a'});",
        "@utility a { color:red; }",
    );
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    let item = marker(&result, "item");
    let parent = marker(&result, "parent");
    assert!(
        result
            .css
            .contains(&format!(":has(:is(.outside>:where(.{item})))")),
        "{}",
        result.css
    );
    assert!(
        result
            .css
            .contains(&format!(":is(.shell>:where(.{parent})):has(")),
        "{}",
        result.css
    );
}

#[test]
fn finite_ref_candidates_keep_one_identity_across_all_scope_conditions() {
    let source = SourceFile::new(FileId(1), None, None,
        "import {style} from 'stillwind/react'; const inner=style.ref('inner'); const outer=style.ref('outer'); const first=style.ref('first'); const second=style.ref('second'); const x=style({[choice().in(inner).in(outer)]: 'a'});"
    ).unwrap();
    let start = u32::try_from(source.text().find("choice()").unwrap()).unwrap();
    let facts = [TypeFact {
        start,
        end: start + 6,
        value: ExportedValue::Refs(
            SymbolicValue::finite(
                AssetKind::Ref,
                ["first", "second"].map(|name| {
                    let start = u32::try_from(source.text().find(name).unwrap()).unwrap();
                    IdentityId::new(
                        AssetKind::Ref,
                        ModuleId::from("test.tsx"),
                        start,
                        start + u32::try_from(name.len()).unwrap(),
                        name,
                    )
                }),
            )
            .unwrap(),
        ),
    }];
    let theme = SourceFile::new(
        FileId(2),
        None,
        None,
        "@utility a { color:red; } @stillwind-slot utilities;",
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
    assert!(result.diagnostics.is_empty(), "{result:#?}");
    let first = marker(&result, "first");
    let second = marker(&result, "second");
    for candidate in [&first, &second] {
        let selector = result
            .css
            .split('{')
            .find(|part| part.contains(candidate))
            .expect("candidate selector");
        assert_eq!(
            selector
                .matches(&format!(":has(:is(:where(.{candidate})"))
                .count(),
            2,
            "{selector}"
        );
        let other = if candidate == &first { &second } else { &first };
        assert!(!selector.contains(other), "{selector}");
    }
}

#[test]
fn registry_resolved_has_and_pseudo_element_conditions_reject_invalid_ref_placement() {
    for (expression, variants) in [
        ("item('has-hover').in(parent)", ""),
        (
            "item('custom-has').in(parent)",
            "@variant custom-has { &:has(.active) { @slot; } }",
        ),
        ("item('before')", ""),
        ("item().in(parent('before'))", ""),
        (
            "item('custom-pseudo')",
            "@variant custom-pseudo { &::before { @slot; } }",
        ),
        (
            "item('nested-has')",
            "@variant nested-has { &:has(:has(.active)) { @slot; } }",
        ),
    ] {
        let (_, result) = compile_registry(
            &format!(
                "const parent=style.ref(); const item=style.ref(); const x=style({{[{expression}]: 'a'}});"
            ),
            &format!("{variants} @utility a {{ color:red; }}"),
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|error| error.diagnostic == CompileDiagnostic::UnavailableVariantPrefix),
            "{expression}: {result:#?}"
        );
        assert!(result.transforms.is_empty(), "{expression}: {result:#?}");
    }
    for expression in ["item('has-hover')", "item().in(parent('has-hover'))"] {
        let (_, valid) = compile_registry(
            &format!(
                "const parent=style.ref(); const item=style.ref(); const x=style({{[{expression}]: 'a'}});"
            ),
            "@utility a { color:red; }",
        );
        assert!(valid.diagnostics.is_empty(), "{expression}: {valid:#?}");
        assert!(valid.css.contains(":has(:hover)"), "{}", valid.css);
    }
}

#[test]
fn punctuation_labels_produce_valid_javascript_strings() {
    let (_, result) = compile(
        r#"export const boundary = style.ref('a"b/c'); export const ref = style.ref('d"e/f'); export const child = style({[ref().in(boundary)]: 'a'});"#,
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
        "const boundary = style.ref('領域 :'); const ref = style.ref('項目 🧭'); \
         export const root = style(boundary); export const item = style(ref); \
         export const child = style({[ref().in(boundary)]: 'a'});",
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
