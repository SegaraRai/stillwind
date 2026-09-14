//! Macro import removal.
//!
//! Pins that a binding is only deleted when every reference was lowered, that a residual
//! reference fails the plan instead of leaving a free identifier, and that mixed imports keep
//! their other specifiers, comments, and separators intact.

use stillwind_compiler::{SourceTransform, plan_macro_import_removals, render_transforms};
use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_js_oxc::parse;
use stillwind_source::{FileId, SourceFile};

fn rewrite(text: &str, lower: bool, unknown: bool) -> Result<String, CompileError> {
    let source = SourceFile::new(FileId(1), "entry.ts", text).unwrap();
    let mut parsed = parse(&source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let lowered = if lower {
        parsed
            .module
            .style_calls
            .iter()
            .map(|call| SourceTransform {
                call_span: call.span,
                replacement: "({})".into(),
                expressions: Vec::new(),
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    if unknown {
        parsed.module.imports.macros[0].specifiers[0].has_unmapped_references = true;
    }
    let mut edits = plan_macro_import_removals(&source, &parsed.module.imports, &lowered)?;
    edits.extend(lowered);
    let code = render_transforms(&source, &edits).unwrap().code;
    let reparsed = parse(&SourceFile::new(FileId(2), "output.ts", code.as_str()).unwrap());
    assert!(
        reparsed.diagnostics.is_empty(),
        "{code}: {:?}",
        reparsed.diagnostics
    );
    Ok(code)
}

#[test]
fn deletion_requires_every_reference_to_be_lowered() {
    let input = "import {style as s} from 'stillwind/react'; const a = s('flex');";
    assert!(!rewrite(input, true, false).unwrap().contains("import"));
    assert!(
        rewrite(
            "import {style as s} from 'stillwind/react'; const a = ( /*callee*/ s)('flex');",
            true,
            false
        )
        .is_err()
    );
    assert!(matches!(
        rewrite(input, false, false),
        Err(CompileError {
            kind: CompileErrorKind::UntransformedMacroReference { .. },
            ..
        })
    ));
    assert!(matches!(
        rewrite(input, true, true),
        Err(CompileError {
            kind: CompileErrorKind::UnmappedMacroReference { .. },
            ..
        })
    ));
    let residual = format!("{input} const macro = s;");
    assert!(matches!(
        rewrite(&residual, true, false),
        Err(CompileError {
            kind: CompileErrorKind::UntransformedMacroReference { .. },
            ..
        })
    ));
}

#[test]
fn mixed_imports_preserve_other_bindings_and_comment_commas() {
    for specifiers in [
        "style, type Theme",
        "type Theme, style",
        "type Theme, /* , */ style /* , */",
        "style /* , */, // ,\n type Theme",
        "style, style as other, type Theme",
        "type Theme, style, style as other",
    ] {
        let input =
            format!("import {{ {specifiers} }} from 'stillwind/react'; const a = style('flex');");
        let output = rewrite(&input, true, false).unwrap();
        assert!(output.contains("type Theme"), "{output}");
        assert!(!output.contains("style as other"), "{output}");
        if input.contains("/* , */") {
            assert!(output.contains("/* , */"));
        }
    }
    let output = rewrite(
        "import Default, {style} from 'stillwind/react'; style('flex');",
        true,
        false,
    );
    let output = output.unwrap();
    assert!(output.contains("import Default, {}"), "{output}");
}
