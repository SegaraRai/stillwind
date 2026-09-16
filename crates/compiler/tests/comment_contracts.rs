//! Public API boundary cases used to verify the Rust documentation audit.

use std::collections::BTreeMap;
use std::sync::Arc;

use stillwind_compiler::{
    CompilerOptions, DesignSystemQuery, Identifiers, ModuleCompiler, StylesheetRenderer,
};
use stillwind_css::relative_css_urls;
use stillwind_design_system::{
    DesignSystemOptions, DesignSystemSnapshot, parse_theme, resolve_utility,
};
use stillwind_diagnostics::CompileErrorKind;
use stillwind_ir::SourceAnalyzer;
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_semantic::{ModuleId, TypeFacts};
use stillwind_source::{
    BytePos, CoordinateIndex, FileId, Fingerprint, Position, ProjectionIndex, ProjectionMap,
    SourceFile, Span, Spanned, VolarMapping,
};

fn snapshot(css: &str) -> DesignSystemSnapshot {
    DesignSystemSnapshot::from_source(
        &SourceFile::new(FileId(90), "theme.css", css).unwrap(),
        DesignSystemOptions::default(),
    )
    .unwrap()
}

#[test]
fn spanned_equality_includes_the_source_range() {
    let a = Spanned::new("same", Span::new(FileId(1), 0, 4));
    let b = Spanned::new("same", Span::new(FileId(1), 5, 9));
    assert_ne!(a, b);
    assert_eq!(a.value, b.value);
}

#[test]
fn line_terminators_and_eof_are_addressable_scalar_boundaries() {
    let source = SourceFile::new(FileId(1), "x", "😀\r\n").unwrap();
    let index = CoordinateIndex::new(&source);
    assert_eq!(
        index
            .at_position(Position {
                line: 0,
                utf16_column: 2
            })
            .unwrap(),
        BytePos(4)
    );
    assert_eq!(
        index
            .at_position(Position {
                line: 0,
                utf16_column: 3
            })
            .unwrap(),
        BytePos(5)
    );
    assert!(
        index
            .at_position(Position {
                line: 0,
                utf16_column: 4
            })
            .is_err()
    );
    assert_eq!(
        index
            .at_position(Position {
                line: 1,
                utf16_column: 0
            })
            .unwrap(),
        BytePos(6)
    );
    assert!(
        index
            .at_position(Position {
                line: 0,
                utf16_column: 1
            })
            .is_err()
    );
}

#[test]
fn agreeing_overlapping_projection_ranges_are_accepted() {
    let original = SourceFile::new(FileId(1), "original", "abcd").unwrap();
    let generated = SourceFile::new(FileId(2), "generated", "abcd").unwrap();
    let map = ProjectionMap::Volar {
        mappings: vec![VolarMapping {
            source_offsets: vec![0, 1],
            generated_offsets: vec![0, 1],
            lengths: vec![3, 3],
            generated_lengths: None,
        }],
    };
    let index = ProjectionIndex::new(&generated, &original, &map).unwrap();
    assert_eq!(index.original_byte(BytePos(2)), Some(BytePos(2)));
    assert_eq!(
        index.verify_exact(generated.full_span()).unwrap().original,
        original.full_span()
    );
}

#[test]
fn active_theme_tokens_apply_resets_then_sort_by_name() {
    let theme = parse_theme(&SourceFile::new(FileId(1), "theme.css",
        "@theme { --color-z: red; --color-a: blue; --color-*: initial; --color-z: green; --color-a: black; }").unwrap()).unwrap();
    let tokens = theme.active_tokens();
    assert_eq!(
        tokens
            .iter()
            .map(|t| t.name.value.as_str())
            .collect::<Vec<_>>(),
        ["--color-a", "--color-z"]
    );
    assert_eq!(tokens[1].value.value, "green");
}

#[test]
fn ordinary_project_tokens_are_pruned_but_static_tokens_are_retained() {
    let theme = parse_theme(
        &SourceFile::new(
            FileId(1),
            "theme.css",
            "@theme { --color-unused: red; } @theme static { --color-static: blue; }",
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        theme
            .tokens_required_by("", false)
            .iter()
            .map(|t| t.name.value.as_str())
            .collect::<Vec<_>>(),
        ["--color-static"]
    );
}

#[test]
fn failed_user_branch_can_resolve_through_the_builtin_registry() {
    let theme = parse_theme(&SourceFile::new(FileId(1), "theme.css",
        "@stillwind-utility x-* { color: --value([color]); } @utility x-* { width: --value([length]); }").unwrap()).unwrap();
    let resolved = resolve_utility("x-[red]", &theme).unwrap();
    assert!(
        resolved.declaration.contains("color:red"),
        "{}",
        resolved.declaration
    );
    assert!(!resolved.declaration.contains("width:"));
}

#[test]
fn unusable_checker_facts_become_a_compilation_error() {
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            "entry.tsx",
            "import { style } from 'stillwind/react'; export const x = style('flex');",
        )
        .unwrap(),
    );
    let scan = OxcSourceAnalyzer.scan(source);
    let facts = TypeFacts {
        schema: TypeFacts::SCHEMA,
        module_id: ModuleId::from("entry.tsx"),
        source_revision: Fingerprint::of("stale"),
        semantic_revision: Fingerprint::of("facts"),
        facts: vec![],
        assets: BTreeMap::new(),
    };
    let analysis = OxcSourceAnalyzer.finalize(scan, Some(&facts));
    assert!(analysis.semantic_revision.is_none());
    assert_eq!(analysis.integration_diagnostics.len(), 1);
    let compiled = ModuleCompiler::new(
        snapshot("@utility flex { display:flex; } @stillwind-slot utilities;"),
        CompilerOptions::default(),
    )
    .compile(&analysis);
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|d| d.kind == CompileErrorKind::InvalidTypeScriptFacts)
    );
    assert!(compiled.css.rules.is_empty());
    assert!(compiled.code_transforms.is_empty());
}

#[test]
fn document_rendering_keeps_the_module_fragment() {
    let design = snapshot("@utility flex { display:flex; } @stillwind-slot utilities;");
    let source = Arc::new(
        SourceFile::new(
            FileId(1),
            "entry.tsx",
            "import { style } from 'stillwind/react'; export const x = style('flex');",
        )
        .unwrap(),
    );
    let analysis = OxcSourceAnalyzer.finalize(OxcSourceAnalyzer.scan(source.clone()), None);
    let options = CompilerOptions::default();
    let compiled = ModuleCompiler::new(design.clone(), options.clone()).compile(&analysis);
    assert!(compiled.is_success());
    let output = StylesheetRenderer::new(&design, &options, &Identifiers::builtin())
        .render_compilation(&compiled, source.full_span());
    assert!(output.diagnostics.is_empty());
    assert!(!output.module_css.is_empty());
    assert_eq!(output.module_css, compiled.css.to_css());
}

#[test]
fn an_empty_utility_is_unresolved_and_produces_no_rules() {
    let query = DesignSystemQuery::new(snapshot("@utility empty {}"), CompilerOptions::default());
    let result = query.resolve_candidate("empty");
    assert!(!result.diagnostics.is_empty());
    assert!(result.generated_rules.is_empty());
}

#[test]
fn resource_specifiers_are_decoded_and_repeated_urls_share_placeholders() {
    let design = snapshot(
        "@utility image { background-image:url('./same.svg'); mask-image:url('./same.svg'); } @stillwind-slot utilities;",
    );
    let resources = &design.sources().resources;
    assert_eq!(resources.len(), 2);
    assert_eq!(resources[0].placeholder, resources[1].placeholder);
    let urls = relative_css_urls(r"a{background:url('./a\20 b.svg')}");
    assert_eq!(urls[0].specifier, "./a b.svg");
}
