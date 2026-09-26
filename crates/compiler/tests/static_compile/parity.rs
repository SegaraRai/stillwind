//! Comparison against the checked-in Tailwind v4 fixtures.
//!
//! Fixture candidates are compiled through Stillwind's own entry points, and the
//! resulting rules are compared to Tailwind's output using test-only normalization.
//! Production CSS spelling is not a compatibility contract. Variant generation order is compared against
//! fixtures whose candidates are spelled out of canonical order, so the check cannot
//! pass by preserving the input order.

use stillwind_compiler::CompilerOptions;
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;

use super::*;

#[test]
fn compiles_candidates_taken_directly_from_tailwind_compatibility_fixtures() {
    let foundations = include_str!("../../../../fixtures/tailwind-v4/cases/foundations.fixture.md");
    let variants = include_str!("../../../../fixtures/tailwind-v4/cases/variants.fixture.md");
    let responsive =
        include_str!("../../../../fixtures/tailwind-v4/cases/responsive-container.fixture.md");
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
        None,
        None,
        format!(
            "import {{ style }} from \"stillwind/react\"; const props = style(\"{candidates}\");"
        ),
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
        result
            .css
            .contains_css("background-color:var(--color-red-500);")
    );
    assert!(result.css.contains("@media (width >= 48rem)"));
    assert!(result.css.contains("[aria-busy=\"true\"]"));
    assert!(result.css.contains(">svg{"));
}

#[test]
fn matches_tailwind_fixture_utility_rules_after_selector_substitution() {
    let cases = [
        ("foundations.fixture.md", "p-4"),
        ("transitions-animation.fixture.md", "transition-colors"),
        ("layout-positioning.fixture.md", "aspect-video"),
        ("layout.fixture.md", "grid-cols-3"),
        ("backgrounds-gradients.fixture.md", "bg-cover"),
        ("borders-effects.fixture.md", "border"),
        ("typography.fixture.md", "text-3xl"),
        ("filters-transforms.fixture.md", "blur-sm"),
        ("accessibility-interaction.fixture.md", "sr-only"),
        ("variants.fixture.md", "hover:bg-red-500"),
        ("candidate-syntax.fixture.md", "content-['hello']"),
        ("default-completions.fixture.md", "inset-shadow-sm/50"),
    ];
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/tailwind-v4/cases");

    for (fixture_name, candidate) in cases {
        let fixture =
            fs::read_to_string(root.join(fixture_name)).expect("fixture must be readable");
        let section = fixture_case_for_candidate(&fixture, candidate);
        let expected_css = fixture_code_block(section, "Expected CSS", "css")
            .expect("fixture must provide expected CSS");
        let input_css = fixture_code_block(section, "Input CSS", "css")
            .expect("fixture must provide input CSS");
        let component = SourceFile::new(
            FileId(1),
            None,
            None,
            format!(
                "import {{ style }} from \"stillwind/react\"; const props = style(\"{candidate}\");"
            ),
        )
        .unwrap();
        let result = Compiler::compile_source(
            &OxcSourceAnalyzer,
            &component,
            "test.tsx",
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
            &crate::css_fixture::source(FileId(2), stillwind_fixture_css(&input_css)).unwrap(),
            "stillwind.css",
            &CompilerOptions::default(),
        );
        assert!(
            result.diagnostics.is_empty(),
            "{candidate}: {:#?}",
            result.diagnostics
        );
        let class_name = &result.classes[0].class_name;
        let actual = css_rule(&result.css, class_name);
        let expected_selector = tailwind_class_selector(candidate);
        let expected = css_rule(&expected_css, &expected_selector);
        let expected = expected.replacen(
            &format!(".{expected_selector}"),
            &format!(".{class_name}"),
            1,
        );
        assert_eq!(
            minify_css(&actual),
            minify_css(&expected),
            "Tailwind semantic utility mismatch for {candidate}"
        );
    }
}

/// The compatibility gate normalizes math and CSS in the test oracle.
/// Production emitters do not optimize CSS to satisfy this comparison.
#[test]
fn matches_tailwind_fixture_rules_after_minification() {
    let cases = [
        ("foundations.fixture.md", "p-4"),
        ("transitions-animation.fixture.md", "transition-colors"),
        ("layout-positioning.fixture.md", "aspect-video"),
        ("layout.fixture.md", "grid-cols-3"),
        ("backgrounds-gradients.fixture.md", "bg-cover"),
        ("borders-effects.fixture.md", "border"),
        ("typography.fixture.md", "text-3xl"),
        ("filters-transforms.fixture.md", "blur-sm"),
        ("accessibility-interaction.fixture.md", "sr-only"),
        ("variants.fixture.md", "hover:bg-red-500"),
        ("variants.fixture.md", "md:px-4"),
        ("variants.fixture.md", "aria-[busy=true]:opacity-60"),
        ("variants.fixture.md", "data-[state=open]:grid"),
        ("media-preference-variants.fixture.md", "dark:bg-slate-950"),
        ("responsive-container.fixture.md", "max-md:hidden"),
        ("responsive-container.fixture.md", "@sm:grid"),
        ("candidate-syntax.fixture.md", "!flex"),
        ("candidate-syntax.fixture.md", "flex!"),
        ("candidate-syntax.fixture.md", "-mt-4"),
        ("candidate-syntax.fixture.md", "w-(--sidebar)"),
        ("candidate-syntax.fixture.md", "[color:rebeccapurple]"),
        (
            "candidate-syntax.fixture.md",
            "supports-[display:grid]:grid",
        ),
        ("candidate-syntax.fixture.md", "content-['hello']"),
        ("default-completions.fixture.md", "inset-shadow-sm/50"),
    ];
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/tailwind-v4/cases");

    for (fixture_name, candidate) in cases {
        let fixture =
            fs::read_to_string(root.join(fixture_name)).expect("fixture must be readable");
        let section = fixture_case_for_candidate(&fixture, candidate);
        let expected_css = fixture_code_block(section, "Expected CSS", "css")
            .expect("fixture must provide expected CSS");
        let input_css = fixture_code_block(section, "Input CSS", "css")
            .expect("fixture must provide input CSS");
        let component = SourceFile::new(
            FileId(1),
            None,
            None,
            format!(
                "import {{ style }} from \"stillwind/react\"; const props = style(\"{candidate}\");"
            ),
        )
        .unwrap();
        let result = Compiler::compile_source(
            &OxcSourceAnalyzer,
            &component,
            "test.tsx",
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
            &crate::css_fixture::source(FileId(2), stillwind_fixture_css(&input_css)).unwrap(),
            "stillwind.css",
            &CompilerOptions::default(),
        );
        assert!(
            result.diagnostics.is_empty(),
            "{candidate}: {:#?}",
            result.diagnostics
        );
        let class_name = &result.classes[0].class_name;
        let selector = tailwind_class_selector(candidate);
        let actual = css_rule(&result.css, class_name).replace(class_name, &selector);
        let expected = css_rule(&expected_css, &selector);
        assert_eq!(
            minify_css(&actual),
            minify_css(&expected),
            "{fixture_name}: {candidate}"
        );
    }
}

#[test]
fn matches_tailwind_variant_generation_order_from_markdown_fixtures() {
    let fixture = include_str!("../../../../fixtures/tailwind-v4/cases/variant-order.fixture.md");
    for case_name in [
        "variant-order.states",
        "variant-order.conditions",
        "variant-order.mixed",
    ] {
        let section = fixture_case_named(fixture, case_name);
        let candidates = fixture_code_block(section, "Candidates", "text")
            .expect("fixture must provide candidates");
        let input_css = fixture_code_block(section, "Input CSS", "css")
            .expect("fixture must provide input CSS");
        let expected_css = fixture_code_block(section, "Expected CSS", "css")
            .expect("fixture must provide expected CSS");
        let component = SourceFile::new(
            FileId(1),
            None,
            None,
            format!(
                "import {{ style }} from \"stillwind/react\"; const props = style({candidates:?});"
            ),
        )
        .unwrap();
        let result = Compiler::compile_source(
            &OxcSourceAnalyzer,
            &component,
            "test.tsx",
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
            &crate::css_fixture::source(FileId(2), stillwind_fixture_css(&input_css)).unwrap(),
            "stillwind.css",
            &CompilerOptions::default(),
        );
        assert!(
            result.diagnostics.is_empty(),
            "{case_name}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.classes.len(), 1, "{case_name}");

        let mut expected = expected_css;
        for candidate in candidates.split_whitespace() {
            expected = expected.replace(
                &format!(".{}", tailwind_class_selector(candidate)),
                ".CLASS",
            );
        }
        let actual = result
            .css
            .replace(&format!(".{}", result.classes[0].class_name), ".CLASS");
        assert_eq!(
            class_rule_signatures(&actual),
            class_rule_signatures(&expected),
            "Tailwind variant generation order mismatch for {case_name}\nStillwind: {actual}\nTailwind: {expected}"
        );
    }
}
