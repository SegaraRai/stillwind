//! Checks labels and the names allocated to declarations.

use super::*;

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
        .find(|symbol| {
            symbol.kind == GeneratedSymbolKind::RefMarker && symbol.name.starts_with("sw_d\"e/f_")
        })
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
    for kind in [GeneratedSymbolKind::RefMarker, GeneratedSymbolKind::Ref] {
        let names = result
            .symbols
            .iter()
            .filter(|symbol| symbol.kind == kind)
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert!(
            names.iter().any(|name| name.contains("領域")),
            "{kind:?}: {names:?}"
        );
        assert!(
            names.iter().any(|name| name.contains("項目")),
            "{kind:?}: {names:?}"
        );
    }
    // A utility class is named for one call, not for a declaration, so it never carries a label.
    for symbol in result
        .symbols
        .iter()
        .filter(|symbol| symbol.kind == GeneratedSymbolKind::Class)
    {
        for authored in ["領域", "項目", "🧭"] {
            assert!(!symbol.name.contains(authored), "{}", symbol.name);
        }
    }
}
