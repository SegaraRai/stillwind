//! Source provenance of rendered stylesheets.
//!
//! Pins that authored CSS reached through an import keeps its physical file and byte range in the
//! mapping, that the same CSS renders identically with and without mappings, and that import
//! conditions survive into the output.

use stillwind_compiler::{CompilerOptions, StylesheetRenderer};
use stillwind_design_system::{
    CssSourceName, DesignSystemLoadStep, DesignSystemLoader, DesignSystemOptions,
};

#[test]
fn imported_authored_css_and_theme_tokens_keep_their_physical_sources() {
    let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
    let DesignSystemLoadStep::NeedFile(requests) = loader.provide(
        0,
        CssSourceName::from("entry.css"),
        "@import 'child.css' layer(parts) supports(display:grid) screen;\n@theme { --color-brand: red; }".into()
    ) else {
        panic!("import")
    };
    let child = "/* 日本語 🧭 */\n.imported { color:var(--color-brand); }";
    let DesignSystemLoadStep::Complete(Ok(snapshot)) = loader.provide(
        requests[0].id,
        CssSourceName::from("child.css"),
        child.into(),
    ) else {
        panic!("complete")
    };
    let output = StylesheetRenderer::new(&snapshot, &CompilerOptions::default())
        .render_base_stylesheet_with_map()
        .unwrap();
    assert_eq!(
        output.css,
        StylesheetRenderer::new(&snapshot, &CompilerOptions::default())
            .render_base_stylesheet()
            .unwrap()
    );
    let authored = output
        .mappings
        .iter()
        .find(|mapping| mapping.source.path() == "child.css")
        .unwrap();
    assert!(authored.exact);
    assert_eq!(
        &output.css[authored.generated.clone()],
        &child[authored.original.clone()]
    );
    assert!(
        output
            .mappings
            .iter()
            .any(|mapping| mapping.source.path() == "entry.css"
                && output.css[mapping.generated.clone()].contains("--color-brand"))
    );
    assert!(output.css.contains("@supports (display:grid)"));
}
