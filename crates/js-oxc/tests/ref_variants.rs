use stillwind_ir::{StyleInput, StyleVariant};
use stillwind_js_oxc::parse;
use stillwind_source::{FileId, SourceFile, Span};

#[test]
fn template_interpolation_retains_ref_identity_and_original_utility_spans() {
    let source = SourceFile::new(
        FileId(1),
        "Ref.tsx",
        r#"import {style} from "stillwind/react";
// 日本語 🧭
const trigger = style.ref("trigger");
const props = style(`md:${trigger`hover:`}:active:text-accent p-4`);"#,
    )
    .unwrap();
    let parsed = parse(&source);
    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
    let inputs = &parsed.module.style_calls[0].inputs;
    let StyleInput::Variant {
        variant: StyleVariant::Prefix(prefix),
        inputs: nested,
    } = &inputs[0]
    else {
        panic!("ordinary variant")
    };
    assert_eq!(source.slice(prefix.span).unwrap(), "md:");
    let StyleInput::Variant {
        variant: StyleVariant::Ref { handles, prefix },
        inputs: nested,
    } = &nested[0]
    else {
        panic!("ref variant")
    };
    assert_eq!(source.slice(prefix.span).unwrap(), "hover:");
    assert_eq!(handles.len(), 1);
    assert_eq!(handles[0].name.value, "trigger");
    let StyleInput::UtilityString(utility) = &nested[0] else {
        panic!("utility")
    };
    assert_eq!(source.slice(utility.span).unwrap(), "active:text-accent");
    let StyleInput::UtilityString(plain) = &inputs[1] else {
        panic!("plain utility")
    };
    assert_eq!(source.slice(plain.span).unwrap(), "p-4");

    let shifted = parsed
        .module
        .try_map_spans::<()>(|span| Ok(Span::new(span.file, span.start.0 + 10, span.end.0 + 10)))
        .unwrap();
    let StyleInput::Variant { inputs: nested, .. } = &shifted.style_calls[0].inputs[0] else {
        panic!("wrapper")
    };
    let StyleInput::Variant {
        variant:
            StyleVariant::Ref {
                handles: moved,
                prefix: moved_prefix,
            },
        ..
    } = &nested[0]
    else {
        panic!("ref")
    };
    assert_eq!(moved_prefix.span.start.0, prefix.span.start.0 + 10);
    assert_eq!(
        moved[0].name.span.start.0,
        handles[0].name.span.start.0 + 10
    );
}

#[test]
fn utility_templates_require_direct_ref_variant_segments() {
    for expression in [
        r#"`${"hover"}:text-accent`"#,
        r"`${trigger`hover:`}text-accent`",
        r"`text-${trigger`hover:`}:accent`",
        r"`${trigger`hover:`} ${trigger`active:`}:text-accent`",
        r"`${trigger`hover:`}`",
        r#"`${trigger`hover:${"active"}`}:text-accent`"#,
        r"`${trigger(...variants)}:text-accent`",
        r#"`${trigger("hover", "focus")}:text-accent`"#,
        r#"`${trigger?.("hover")}:text-accent`"#,
    ] {
        let source = SourceFile::new(FileId(1), "Invalid.tsx", format!(
            "import {{style}} from 'stillwind/react'; const trigger = style.ref(); const props = style({expression});"
        )).unwrap();
        assert!(!parse(&source).diagnostics.is_empty(), "{expression}");
    }
}

#[test]
fn saved_interpolated_strings_are_not_followed() {
    let source = SourceFile::new(
        FileId(1),
        "Saved.tsx",
        r#"import {style} from "stillwind/react";
const trigger = style.ref();
const classes = `${trigger`hover:`}:text-accent`;
const props = style(classes);"#,
    )
    .unwrap();
    let parsed = parse(&source);
    assert!(!parsed.diagnostics.is_empty());
    assert!(parsed.module.style_calls[0].inputs.is_empty());
}
