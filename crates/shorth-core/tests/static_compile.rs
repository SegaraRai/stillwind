use shorth_core::Compiler;
use shorth_source::{FileId, SourceFile};
use std::{fs, path::Path};

fn fixture_candidates(fixture: &str, case_name: &str) -> String {
    let case_start = fixture
        .find(&format!("## `{case_name}`"))
        .expect("fixture case must exist");
    let candidates_start = fixture[case_start..]
        .find("### Candidates\n\n```text\n")
        .expect("fixture candidates must exist")
        + case_start
        + "### Candidates\n\n```text\n".len();
    fixture[candidates_start..]
        .split_once("\n```")
        .expect("fixture candidates must close")
        .0
        .trim()
        .to_owned()
}

fn fixture_code_block(section: &str, heading: &str, language: &str) -> Option<String> {
    let marker = format!("### {heading}\n\n```{language}\n");
    let start = section.find(&marker)? + marker.len();
    Some(section[start..].split_once("\n```")?.0.trim().to_owned())
}

#[test]
fn supports_at_least_ninety_five_percent_of_tailwind_fixture_candidates() {
    let case_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/tailwind-v4/cases");
    let fixtures = fs::read_dir(case_root)
        .expect("fixture directory must exist")
        .map(|entry| entry.expect("fixture entry must be readable").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .map(|path| {
            (
                path.file_name()
                    .and_then(|name| name.to_str())
                    .expect("fixture file name must be UTF-8")
                    .to_owned(),
                fs::read_to_string(path).expect("fixture must be readable"),
            )
        })
        .collect::<Vec<_>>();

    let mut candidate_count = 0_usize;
    let mut unknown = Vec::new();
    for (fixture_name, fixture) in fixtures {
        for section in fixture.split("\n## `").skip(1) {
            let Some(candidates) = fixture_code_block(section, "Candidates", "text") else {
                continue;
            };
            candidate_count += candidates.split_whitespace().count();
            let input_css = fixture_code_block(section, "Input CSS", "css")
                .unwrap_or_else(|| "@theme {}".to_owned());
            let component = SourceFile::new(
                FileId(1),
                "src/Fixture.tsx",
                format!(
                    "import {{ style }} from \"stillwind/react\"; const props = style({:?});",
                    candidates.replace('\n', " ")
                ),
            );
            let theme = SourceFile::new(FileId(2), "stillwind.css", input_css);
            let result = Compiler::compile_react(&component, &theme);
            unknown.extend(
                result
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.code == "SH001")
                    .map(|diagnostic| format!("{fixture_name}: {}", diagnostic.message)),
            );
        }
    }
    unknown.sort();

    assert!(
        unknown.len() * 100 <= candidate_count * 5,
        "supported {}/{} fixture candidates; unresolved: {unknown:#?}",
        candidate_count - unknown.len(),
        candidate_count,
    );
}

#[test]
fn compiles_candidates_taken_directly_from_tailwind_compatibility_fixtures() {
    let foundations = include_str!("../../../fixtures/tailwind-v4/cases/foundations.fixture.md");
    let variants = include_str!("../../../fixtures/tailwind-v4/cases/variants.fixture.md");
    let responsive =
        include_str!("../../../fixtures/tailwind-v4/cases/responsive-container.fixture.md");
    let candidates = [
        fixture_candidates(foundations, "foundations.preflight-and-default-theme"),
        fixture_candidates(
            variants,
            "variants.state-attribute-responsive-and-arbitrary",
        ),
        fixture_candidates(responsive, "responsive.breakpoints-and-containers"),
    ]
    .join(" ");
    let component = SourceFile::new(
        FileId(1),
        "src/FixtureCard.tsx",
        format!(
            "import {{ style }} from \"stillwind/react\"; const props = style(\"{candidates}\");"
        ),
    );
    let theme = SourceFile::new(FileId(2), "stillwind.css", "@theme {}");

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result
            .css
            .contains("background-color:var(--color-red-500);")
    );
    assert!(result.css.contains("@media (width >= 48rem)"));
    assert!(result.css.contains("[aria-busy=\"true\"]"));
    assert!(result.css.contains(">svg{"));
    assert!(result.css.contains("@container (width >= 24rem)"));
}

#[test]
fn emits_nested_supports_from_css_utility_definitions() {
    let component = SourceFile::new(
        FileId(1),
        "src/Gradient.tsx",
        r#"import { style } from "stillwind/react";
const props = style("bg-linear-to-r");"#,
    );
    let theme = SourceFile::new(FileId(2), "stillwind.css", "@theme {}");

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("--tw-gradient-position:to right;"));
    assert!(
        result
            .css
            .contains("@supports (background-image:linear-gradient(in lab,red,red)){._s_")
    );
    assert!(
        result
            .css
            .contains("--tw-gradient-position:to right in oklab;")
    );
}

#[test]
fn emits_selector_bearing_css_utilities_on_their_target_children() {
    let component = SourceFile::new(
        FileId(1),
        "src/Divider.tsx",
        r#"import { style } from "stillwind/react";
const props = style("divide-x-[2px]");"#,
    );
    let theme = SourceFile::new(FileId(2), "stillwind.css", "@theme {}");

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result
            .css
            .contains(">:not(:last-child){border-inline-end-width:2px;"),
        "{}",
        result.css
    );
}

#[test]
fn emits_starting_style_variants_as_a_css_wrapper() {
    let component = SourceFile::new(
        FileId(1),
        "src/EnteringPanel.tsx",
        r#"import { style } from "stillwind/react";
const props = style("starting:opacity-0");"#,
    );
    let theme = SourceFile::new(FileId(2), "stillwind.css", "@theme {}");

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("@starting-style{._s_"));
    assert!(result.css.contains("{opacity:0;}"));
}

#[test]
fn compiles_a_real_react_style_call_against_a_css_theme() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";

export function SaveButton() {
  return <button {...style("inline-flex items-center rounded-md bg-accent px-4 text-center")}>Save</button>;
}"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "src/styles/shorth.css",
        "@theme { --color-accent: oklch(.6 .2 250); --spacing-4: 1rem; --radius-md: .375rem; }",
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.classes.len(), 1);
    assert!(
        result
            .css
            .starts_with("@layer theme, base, components, utilities;")
    );
    assert!(result.css.contains("--color-accent:oklch(.6 .2 250);"));
    assert!(result.css.contains("@layer base"));
    assert!(result.css.contains("._s_bc6dda5a{display:inline-flex;align-items:center;border-radius:var(--radius-md);background-color:var(--color-accent);padding-inline:var(--spacing-4);text-align:center;}"));
}

#[test]
fn reports_an_unknown_token_at_the_utility_not_the_whole_call() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
const props = style("bg-accnet");"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "src/styles/shorth.css",
        "@theme { --color-accent: red; }",
    );

    let result = Compiler::compile_react(&component, &theme);
    let diagnostic = result
        .diagnostics
        .first()
        .expect("an unknown utility diagnostic");

    assert_eq!(diagnostic.code, "SH001");
    assert_eq!(
        component.slice(diagnostic.labels[0].span).unwrap(),
        "bg-accnet"
    );
    assert!(result.css.is_empty());
}

#[test]
fn emits_css_for_a_runtime_conditional_branch_without_evaluating_it() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
declare const loading: boolean;
const props = style("bg-accent", loading && "opacity-60");"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "src/styles/shorth.css",
        "@theme { --color-accent: red; }",
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.classes.len(), 2);
    assert!(result.css.contains("background-color:var(--color-accent);"));
    assert!(result.css.contains("opacity:0.6;"));
}

#[test]
fn compiles_both_sides_of_a_static_ternary_branch() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
declare const active: boolean;
const props = style(active ? "bg-accent" : "bg-muted");"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "stillwind.css",
        "@theme { --color-accent: red; --color-muted: gray; }",
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.classes.len(), 2);
    assert!(result.css.contains("background-color:var(--color-accent);"));
    assert!(result.css.contains("background-color:var(--color-muted);"));
    assert!(result.transforms[0].replacement.contains("active&&"));
    assert!(result.transforms[0].replacement.contains("!active&&"));
}

#[test]
fn plans_react_props_for_classes_a_conditional_and_an_aria_attribute() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
declare const loading: boolean;
const props = style("bg-accent", loading && "opacity-60", { "aria-busy": loading });"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "src/styles/shorth.css",
        "@theme { --color-accent: red; }",
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.transforms.len(), 1);
    let transform = &result.transforms[0];
    assert_eq!(transform.call_span, result.classes[0].call_span);
    assert_eq!(
        transform.replacement,
        format!(
            "{{className:stillwindClsx(\"{}\",loading&&\"{}\"),\"aria-busy\":loading}}",
            result.classes[0].class_name, result.classes[1].class_name,
        ),
    );
}

#[test]
fn emits_tailwind_shaped_layers_theme_variables_and_variants() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
const props = style("bg-accent hover:bg-accent/90 md:px-4 aria-[busy=true]:opacity-60");"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "src/styles/shorth.css",
        r"@theme {
  --color-accent: oklch(.6 .2 250);
  --spacing-4: 1rem;
  --breakpoint-md: 48rem;
}",
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result
            .css
            .starts_with("@layer theme, base, components, utilities;\n")
    );
    assert!(result.css.contains("--color-accent:oklch(.6 .2 250);"));
    assert!(result.css.contains("--spacing-4:1rem;"));
    assert!(result.css.contains("--breakpoint-md:48rem;"));
    assert!(result.css.contains("background-color:var(--color-accent);"));
    assert!(
        result
            .css
            .contains("background-color:color-mix(in oklab,var(--color-accent) 90%,transparent);")
    );
    assert!(result.css.contains(":hover{"));
    assert!(result.css.contains("[aria-busy=\"true\"]{"));
    assert!(result.css.contains("@media (width >= 48rem){"));
}

#[test]
fn retains_authored_custom_properties_and_theme_selectors() {
    let component = SourceFile::new(
        FileId(1),
        "src/App.tsx",
        r#"import { style } from "stillwind/react";
const props = style("bg-accent");"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "src/styles/shorth.css",
        r#"@theme inline { --color-accent: var(--app-accent); }
:root { --app-accent: oklch(.6 .2 250); }
[data-theme="dark"] { --app-accent: oklch(.3 .1 250); }"#,
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("--color-accent:var(--app-accent);"));
    assert!(
        result
            .css
            .contains(":root { --app-accent: oklch(.6 .2 250); }")
    );
    assert!(
        result
            .css
            .contains("[data-theme=\"dark\"] { --app-accent: oklch(.3 .1 250); }")
    );
}

#[test]
fn resolves_v4_default_spacing_and_arbitrary_custom_properties() {
    let component = SourceFile::new(
        FileId(1),
        "src/App.tsx",
        r#"import { style } from "stillwind/react";
const props = style("px-4 -mt-2 bg-red-500 [--gutter:1rem] w-[calc(var(--gutter)*2)]");"#,
    );
    let theme = SourceFile::new(FileId(2), "src/styles/shorth.css", "@theme {}");

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result
            .css
            .contains("padding-inline:calc(var(--spacing) * 4);")
    );
    assert!(result.css.contains("margin-top:calc(var(--spacing) * -2);"));
    assert!(
        result
            .css
            .contains("background-color:var(--color-red-500);")
    );
    assert!(result.css.contains("--gutter:1rem;"));
    assert!(result.css.contains("width:calc(var(--gutter)*2);"));
}

#[test]
fn resolves_v4_default_animations_and_emits_their_keyframes() {
    let component = SourceFile::new(
        FileId(1),
        "src/App.tsx",
        r#"import { style } from "stillwind/react";
const props = style("animate-spin");"#,
    );
    let theme = SourceFile::new(FileId(2), "src/styles/shorth.css", "@theme {}");

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("animation:var(--animate-spin);"));
    assert!(result.css.contains("@keyframes spin"));
}

#[test]
fn resolves_css_defined_custom_utilities_and_variants() {
    let component = SourceFile::new(
        FileId(1),
        "src/App.tsx",
        r#"import { style } from "stillwind/react";
const props = style("content-auto data-checked:content-auto tab-8");"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "src/styles/shorth.css",
        r#"@utility content-auto { content-visibility: auto; }
@utility tab-* { tab-size: --value(integer); }
@custom-variant data-checked (&[data-ui~="checked"]);"#,
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("content-visibility: auto;"));
    assert!(result.css.contains("[data-ui~=\"checked\"]{"));
    assert!(result.css.contains("tab-size: 8;"));
}

#[test]
fn applies_a_transform_plan_without_touching_surrounding_source() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
const props = style("bg-accent");
export { props };"#,
    );
    let theme = SourceFile::new(FileId(2), "shorth.css", "@theme { --color-accent: red; }");

    let result = Compiler::compile_react(&component, &theme);
    let transformed = result.apply_to_source(&component).unwrap();

    assert!(
        transformed.starts_with(
            "import { style } from \"stillwind/react\";\nconst props = {className:\"_s_"
        )
    );
    assert!(transformed.ends_with("};\nexport { props };"));
}

#[test]
fn compiles_nested_arrays_attributes_and_element_local_selectors() {
    let component = SourceFile::new(
        FileId(1),
        "src/SaveButton.tsx",
        r#"import { style } from "stillwind/react";
declare const loading: boolean;
const props = style(
  ["bg-accent", ["px-4"], loading && "opacity-60"],
  { "aria-busy": loading, "&:hover": "bg-accent/90" },
);"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "src/styles/stillwind.css",
        "@theme { --color-accent: red; --spacing-4: 1rem; }",
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.classes.len(), 2);
    assert!(result.css.contains("background-color:var(--color-accent);"));
    assert!(result.css.contains("padding-inline:var(--spacing-4);"));
    assert!(result.css.contains(":hover{background-color:color-mix("));
    assert!(result.css.contains("opacity:0.6;"));
    assert!(
        result.transforms[0]
            .replacement
            .contains("\"aria-busy\":loading")
    );
}

#[test]
fn resolves_negative_and_transition_utilities_from_declared_theme_tokens() {
    let component = SourceFile::new(
        FileId(1),
        "src/Card.tsx",
        r#"import { style } from "stillwind/react";
const props = style("-mt-4 duration-fast ease-standard");"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "src/styles/stillwind.css",
        "@theme { --spacing-4: 1rem; --duration-fast: 120ms; --ease-standard: ease-out; }",
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result
            .css
            .contains("margin-top:calc(var(--spacing-4) * -1);")
    );
    assert!(
        result
            .css
            .contains("transition-duration:var(--duration-fast);")
    );
    assert!(
        result
            .css
            .contains("transition-timing-function:var(--ease-standard);")
    );
}

#[test]
fn projects_one_handle_into_anchor_position_scope_and_container_css() {
    let component = SourceFile::new(
        FileId(1),
        "src/Popover.tsx",
        r#"import { style } from "stillwind/react";
const popover = style.handle();
const root = style(popover.scope, popover.container("inline-size"));
const trigger = style(popover.anchor, "relative");
const panel = style(popover.position, "absolute");"#,
    );
    let theme = SourceFile::new(FileId(2), "stillwind.css", "@theme {}");

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("anchor-name:--sw-h-"));
    assert!(result.css.contains("position-anchor:--sw-h-"));
    assert!(result.css.contains("anchor-scope:--sw-h-"));
    assert!(result.css.contains("container-name:--sw-h-"));
    assert!(result.css.contains("container-type:inline-size;"));
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(!transformed.contains("style.handle()"));
    assert!(transformed.contains("const popover = undefined;"));
}

#[test]
fn reports_a_position_handle_without_an_anchor_provider() {
    let component = SourceFile::new(
        FileId(1),
        "src/Popover.tsx",
        r#"import { style } from "stillwind/react";
const popover = style.handle();
const panel = style(popover.position);"#,
    );
    let theme = SourceFile::new(FileId(2), "stillwind.css", "@theme {}");

    let result = Compiler::compile_react(&component, &theme);

    assert!(
        result
            .diagnostics
            .iter()
            .any(|item| item.code == "SHHANDLE003")
    );
    assert!(result.css.is_empty());
}

#[test]
fn emits_named_container_rules_from_handle_queries() {
    let component = SourceFile::new(
        FileId(1),
        "src/Card.tsx",
        r#"import { style } from "stillwind/react";
const card = style.handle();
const root = style(card.container("inline-size"));
const content = style(card.at("width >= 40rem", "grid-cols-2"));"#,
    );
    let theme = SourceFile::new(FileId(2), "stillwind.css", "@theme {}");

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("@container --sw-h-"));
    assert!(result.css.contains("(width >= 40rem){"));
    assert!(
        result
            .css
            .contains("grid-template-columns:repeat(2,minmax(0,1fr));")
    );
}

#[test]
fn uses_handle_tagged_selectors_as_automatically_named_parent_groups() {
    let component = SourceFile::new(
        FileId(1),
        "src/Menu.tsx",
        r#"import { style } from "stillwind/react";
const trigger = style.handle();
const parent = style(trigger.anchor, "inline-flex");
const child = style({ [trigger`&:hover`]: "text-accent" });"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "stillwind.css",
        "@theme { --color-accent: red; }",
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    let marker = "sw-h-";
    assert!(result.transforms[0].replacement.contains(marker));
    assert!(result.css.contains(&format!(".{marker}")));
    assert!(result.css.contains(":hover ._s_"));
}

#[test]
fn compiles_local_utility_animations_and_selector_uses() {
    let component = SourceFile::new(
        FileId(1),
        "src/Dialog.tsx",
        r#"import { style } from "stillwind/react";
const enter = style.animation({ from: "opacity-0 translate-y-2 scale-98", to: "opacity-100 translate-y-0 scale-100" });
const exit = style.animation({ from: "opacity-100", to: "opacity-0 scale-98" });
const panel = style("rounded-md", enter("duration-normal ease-standard both"), { "&[data-state=closed]": exit("duration-fast ease-standard both") });"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "stillwind.css",
        "@theme { --radius-md: .375rem; --spacing-2: .5rem; --duration-fast: 120ms; --duration-normal: 180ms; --ease-standard: ease-out; }",
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("@keyframes --sw-a-"));
    assert!(
        result
            .css
            .contains("from{opacity:0;transform:translateY(var(--spacing-2)) scale(0.98);}")
    );
    assert!(result.css.contains("animation-name:--sw-a-"));
    assert!(
        result
            .css
            .contains("animation-duration:var(--duration-normal);")
    );
    assert!(
        result
            .css
            .contains("[data-state=closed]{animation-name:--sw-a-")
    );
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(!transformed.contains("style.animation"));
    assert!(transformed.contains("const enter = undefined;"));
}

#[test]
fn compiles_view_transition_assets_and_keyed_instances() {
    let component = SourceFile::new(
        FileId(1),
        "src/ProductCard.tsx",
        r#"import { style } from "stillwind/react";
declare const product: { id: string };
const title = style.viewTransition({ group: "duration-normal ease-standard", old: "opacity-0 -translate-y-1", new: "opacity-100" });
const card = style.viewTransition({ old: "opacity-0", new: "opacity-100" });
const heading = style("text-accent", title);
const item = style("rounded-md", card.for(product.id));"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "stillwind.css",
        "@theme { --color-accent: red; --radius-md: .375rem; --spacing-1: .25rem; --duration-normal: 180ms; --ease-standard: ease-out; }",
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("::view-transition-group(--sw-v-"));
    assert!(result.css.contains("::view-transition-old(--sw-v-"));
    assert!(result.css.contains("::view-transition-new(--sw-v-"));
    assert!(result.css.contains("view-transition-name:--sw-v-"));
    assert!(result.css.contains("view-transition-name:var(--sw-v-"));
    let transformed = result.apply_to_source(&component).unwrap();
    assert!(!transformed.contains("style.viewTransition"));
    assert!(transformed.contains("style:{\"--sw-v-"));
    assert!(transformed.contains("const title = undefined;"));
}

#[test]
fn compiles_an_explicitly_reasoned_unsafe_escape_hatch() {
    let component = SourceFile::new(
        FileId(1),
        "src/VendorWidget.tsx",
        r#"import { style } from "stillwind/react";
const props = style.unsafe({ selector: "& [data-vendor-widget] .tooltip", css: { zIndex: 100, "pointer-events": "none" }, reason: "The vendor widget exposes no styling part." });"#,
    );
    let theme = SourceFile::new(FileId(2), "stillwind.css", "@theme {}");

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result
            .css
            .contains("[data-vendor-widget] .tooltip{z-index:100;pointer-events:none;}")
    );
    assert!(result.transforms[0].replacement.contains("className:\"_s_"));
}

#[test]
fn validates_handle_selector_ancestry_when_jsx_placement_is_known() {
    let component = SourceFile::new(
        FileId(1),
        "src/Menu.tsx",
        r#"import { style } from "stillwind/react";
const trigger = style.handle();
export const Good = () => <div {...style(trigger.anchor)}><span {...style({ [trigger`&:hover`]: "text-accent" })} /></div>;
export const Bad = () => <><div {...style(trigger.anchor)} /><span {...style({ [trigger`&:hover`]: "text-accent" })} /></>;"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "stillwind.css",
        "@theme { --color-accent: red; }",
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(
        result
            .diagnostics
            .iter()
            .any(|item| item.code == "SHHANDLE007")
    );
}

#[test]
fn resolves_a_broader_css_first_tailwind_compatibility_slice() {
    let component = SourceFile::new(
        FileId(1),
        "src/ModernCard.tsx",
        r#"import { style } from "stillwind/react";
const props = style("grid place-items-center size-10 flex-col grow border outline-2 outline-offset-2 outline-accent z-popover position-area-block-end grid-cols-3 text-3xl");"#,
    );
    let theme = SourceFile::new(
        FileId(2),
        "stillwind.css",
        "@theme { --spacing-10: 2.5rem; --color-accent: red; --z-index-popover: 50; --text-3xl: 1.875rem; }",
    );

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(result.css.contains("place-items:center;"));
    assert!(
        result
            .css
            .contains("width:var(--spacing-10);height:var(--spacing-10);")
    );
    assert!(
        result
            .css
            .contains("grid-template-columns:repeat(3,minmax(0,1fr));")
    );
    assert!(result.css.contains("outline-color:var(--color-accent);"));
    assert!(result.css.contains("z-index:var(--z-index-popover);"));
}

#[test]
fn emits_tailwind_background_gradient_and_mask_declarations() {
    let component = SourceFile::new(
        FileId(1),
        "src/Hero.tsx",
        r#"import { style } from "stillwind/react";
const props = style("bg-red-500/50 bg-linear-to-r from-red-500 via-white to-blue-500 mask-linear-from-0 mask-linear-to-100");"#,
    );
    let theme = SourceFile::new(FileId(2), "stillwind.css", "@theme {}");

    let result = Compiler::compile_react(&component, &theme);

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert!(
        result
            .css
            .contains("background-color:color-mix(in oklab,var(--color-red-500) 50%,transparent);")
    );
    assert!(result.css.contains("--tw-gradient-position:to right;"));
    assert!(
        result
            .css
            .contains("--tw-gradient-from:var(--color-red-500);")
    );
    assert!(result.css.contains("--tw-gradient-via:var(--color-white);"));
    assert!(
        result
            .css
            .contains("--tw-gradient-to:var(--color-blue-500);")
    );
    assert!(
        result
            .css
            .contains("--tw-mask-linear-from-position:calc(var(--spacing) * 0);")
    );
    assert!(
        result
            .css
            .contains("--tw-mask-linear-to-position:calc(var(--spacing) * 100);")
    );
}

#[test]
fn names_symbols_from_normalized_module_paths_not_process_local_file_ids() {
    let module = r#"import { style } from "stillwind/react";
const trigger = style.handle();
const props = style(trigger.anchor);"#;
    let windows = SourceFile::new(FileId(1), "src\\Menu.tsx", module);
    let unix = SourceFile::new(FileId(1), "src/Menu.tsx", module);
    let other = SourceFile::new(FileId(1), "src/Other.tsx", module);
    let theme = SourceFile::new(FileId(2), "stillwind.css", "@theme {}");

    let windows_result = Compiler::compile_react(&windows, &theme);
    let unix_result = Compiler::compile_react(&unix, &theme);
    let other_result = Compiler::compile_react(&other, &theme);

    assert_eq!(windows_result.symbols[0].name, unix_result.symbols[0].name);
    assert_ne!(windows_result.symbols[0].name, other_result.symbols[0].name);
    assert!(windows_result.symbols[0].identity.contains("src/Menu.tsx"));
}
