//! Import edits derived from resolved references and successful macro replacements.
//!
//! Removing a macro binding is only sound when every resolved read of it was replaced
//! by a lowering, so the plan is built from the frontend's reference list and rejected
//! wholesale when any read is unaccounted for; a partial removal would leave a dangling
//! specifier or a reference to a deleted binding. Only the named macro specifiers and
//! their list separators are removed. Delimiters, comments, default bindings, and
//! non-macro specifiers stay verbatim, so user formatting inside the declaration
//! survives.

use lean_string::LeanString;

use stillwind_diagnostics::{CompileDiagnostic, CompileError, SourceFailureContext};
use stillwind_ir::ModuleImports;
use stillwind_source::{SourceFile, Span};

use crate::SourceTransform;

/// Remove macro bindings only when every resolved read was successfully replaced.
///
/// Edits replace a declaration as a unit, so adjacent removed specifiers cannot create
/// overlapping edits, and the returned spans always cover the original import
/// statement.
///
/// # Errors
///
/// Rejects invalid metadata, untransformed macro reads, and unverified projection
/// references. No partial edit plan is returned on failure. Metadata and residual-read
/// failures label the offending import or use; source-coordinate failures label the
/// whole supplied source and retain rejected coordinates in the cause.
#[allow(clippy::too_many_lines)] // Validate every reference before returning an atomic import edit plan.
pub fn plan_macro_import_removals(
    source: &SourceFile,
    imports: &ModuleImports,
    lowered: &[SourceTransform],
) -> Result<Vec<SourceTransform>, CompileError> {
    let mut edits = Vec::new();
    for import in &imports.macros {
        let declaration = source.slice(import.span).map_err(|error| {
            CompileError::new(
                CompileDiagnostic::source_failure(&error, SourceFailureContext::MacroImport),
                source.full_span(),
            )
        })?;
        let mut previous_end = import.span.start;
        let mut valid = true;
        for specifier in &import.specifiers {
            source.slice(specifier.span).map_err(|error| {
                CompileError::new(
                    CompileDiagnostic::source_failure(&error, SourceFailureContext::MacroImport),
                    source.full_span(),
                )
            })?;
            valid &= specifier.span.file == import.span.file
                && specifier.span.start >= previous_end
                && specifier.span.end <= import.span.end;
            previous_end = specifier.span.end;
        }
        if !valid {
            return Err(CompileError::new(
                CompileDiagnostic::InvalidMacroImportMetadata { span: import.span },
                import.span,
            ));
        }
        for specifier in &import.specifiers {
            if !specifier.is_macro {
                continue;
            }
            if specifier.has_unmapped_references {
                return Err(CompileError::new(
                    CompileDiagnostic::UnmappedMacroReference {
                        local: (specifier.local.clone()),
                        span: specifier.span,
                    },
                    specifier.span,
                ));
            }
            for reference in &specifier.references {
                if !lowered.iter().any(|edit| {
                    edit.call_span.file == reference.file
                        && edit.call_span.start <= reference.start
                        && reference.end <= edit.call_span.end
                        && (imports.consumed_uses.iter().any(|use_| {
                            use_.file == reference.file
                                && edit.call_span.start <= use_.start
                                && use_.end <= edit.call_span.end
                                && use_.start == reference.start
                                && reference.end <= use_.end
                        }) || imports.leading_reads.iter().any(|read| {
                            read.reference == *reference
                                && read.call.file == edit.call_span.file
                                && read.call.start == edit.call_span.start
                        }))
                }) {
                    return Err(CompileError::new(
                        CompileDiagnostic::UntransformedMacroReference {
                            local: (specifier.local.clone()),
                            span: *reference,
                        },
                        *reference,
                    ));
                }
            }
        }
        let removable = import
            .specifiers
            .iter()
            .map(|specifier| specifier.is_macro)
            .collect::<Vec<_>>();
        if !removable.iter().any(|remove| *remove) {
            continue;
        }
        if removable.iter().all(|remove| *remove) {
            edits.push(SourceTransform {
                call_span: import.span,
                replacement: (LeanString::new()),
                expressions: Vec::new(),
            });
            continue;
        }
        // Keep delimiters and comments verbatim. Only named macro specifiers
        // and their list separators are removed; default/type bindings survive.
        let local = |span: Span| {
            (
                (span.start.0 - import.span.start.0) as usize,
                (span.end.0 - import.span.start.0) as usize,
            )
        };
        let mut ranges = Vec::new();
        for (index, specifier) in import.specifiers.iter().enumerate() {
            if !removable[index] {
                continue;
            }
            ranges.push(local(specifier.span));
            if let Some(separator) = specifier.separator {
                ranges.push(local(separator));
            }
        }
        ranges.sort_unstable();
        let mut replacement = LeanString::new();
        let mut cursor = 0;
        for (start, end) in ranges {
            if start >= cursor {
                replacement.push_str(&declaration[cursor..start]);
            }
            cursor = cursor.max(end);
        }
        replacement.push_str(&declaration[cursor..]);
        edits.push(SourceTransform {
            call_span: import.span,
            replacement: (replacement),
            expressions: Vec::new(),
        });
    }
    Ok(edits)
}

#[cfg(test)]
mod tests {
    use lean_string::ToLeanString;

    use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
    use stillwind_js_oxc::parse;
    use stillwind_source::{FileId, ModuleId};

    use super::*;
    use crate::{plan_macro_import_removals, render_transforms};

    /// Rewrites a module the way a compilation pass would: lower the recorded calls,
    /// then plan the import removal against those replacements.
    fn rewrite(text: &str, lower: bool, unknown: bool) -> Result<LeanString, CompileError> {
        let source = SourceFile::new(FileId(1), None, None, text).unwrap();
        let mut parsed = parse(
            &source,
            &ModuleId::from("test.tsx"),
            &[],
            &std::collections::BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
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
        let reparsed = parse(
            &SourceFile::new(FileId(2), None, None, code.as_str()).unwrap(),
            &ModuleId::from("test.tsx"),
            &[],
            &std::collections::BTreeMap::default(),
            ScriptLanguage::Tsx,
            ScriptModuleKind::Unambiguous,
        );
        assert!(
            reparsed.diagnostics.is_empty(),
            "{code}: {:?}",
            reparsed.diagnostics
        );
        Ok((code).into())
    }

    /// A binding is deleted only when every resolved read was lowered; a residual read
    /// fails the plan instead of leaving a free identifier.
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
                diagnostic: CompileDiagnostic::UntransformedMacroReference { .. },
                ..
            })
        ));
        assert!(matches!(
            rewrite(input, true, true),
            Err(CompileError {
                diagnostic: CompileDiagnostic::UnmappedMacroReference { .. },
                ..
            })
        ));
        let residual = format_args!("{input} const macro = s;").to_lean_string();
        assert!(matches!(
            rewrite(&residual, true, false),
            Err(CompileError {
                diagnostic: CompileDiagnostic::UntransformedMacroReference { .. },
                ..
            })
        ));
    }

    /// Other specifiers, comments, and separators stay verbatim around the removed ones.
    #[test]
    fn mixed_imports_preserve_other_bindings_and_comment_commas() {
        for specifiers in [
            "style, type Theme",
            "type Theme, style",
            "type Theme, /* , */ style /* , */",
            "style /* , */, // ,\n type Theme",
            "type Theme /* { } */, style // }\n",
            "style, style as other, type Theme",
            "type Theme, style, style as other",
        ] {
            let input = format_args!(
                "import {{ {specifiers} }} from 'stillwind/react'; const a = style('flex');"
            )
            .to_lean_string();
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
}
