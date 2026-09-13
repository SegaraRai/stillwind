//! Import edits derived from resolved references and successful macro replacements.
//!
//! Removing a macro binding is only sound when every resolved read of it was replaced by
//! a lowering, so the plan is built from the frontend's reference list and rejected
//! wholesale when any read is unaccounted for; a partial removal would leave a dangling
//! specifier or a reference to a deleted binding. Only the named macro specifiers and
//! their list separators are removed. Delimiters, comments, default bindings, and
//! non-macro specifiers stay verbatim, so user formatting inside the declaration
//! survives.

use stillwind_diagnostics::{CompileError, CompileErrorKind};
use stillwind_ir::ModuleImports;
use stillwind_source::{SourceFile, Span};

use crate::SourceTransform;

/// Remove macro bindings only when every resolved read was successfully replaced.
///
/// Edits replace a declaration as a unit, so adjacent removed specifiers cannot create
/// overlapping edits, and the returned spans always cover the original import statement.
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
                CompileErrorKind::import_source_failure(&error),
                source.full_span(),
            )
        })?;
        let mut previous_end = import.span.start;
        let mut valid = true;
        for specifier in &import.specifiers {
            source.slice(specifier.span).map_err(|error| {
                CompileError::new(
                    CompileErrorKind::import_source_failure(&error),
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
                CompileErrorKind::InvalidImportMetadata { span: import.span },
                import.span,
            ));
        }
        for specifier in &import.specifiers {
            if !specifier.is_macro {
                continue;
            }
            if specifier.has_unmapped_references {
                return Err(CompileError::new(
                    CompileErrorKind::UnmappedMacroReference {
                        local: specifier.local.clone(),
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
                        && source
                            .slice(Span::new(
                                reference.file,
                                edit.call_span.start.0,
                                reference.start.0,
                            ))
                            .is_ok_and(callee_prefix)
                }) {
                    return Err(CompileError::new(
                        CompileErrorKind::UntransformedMacroReference {
                            local: specifier.local.clone(),
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
                replacement: String::new(),
            });
            continue;
        }
        // Keep delimiters and comments verbatim. Only named macro specifiers
        // and their list separators are removed; default/type bindings survive.
        let mut ranges = Vec::new();
        for (index, specifier) in import.specifiers.iter().enumerate() {
            if !removable[index] {
                continue;
            }
            let start = (specifier.span.start.0 - import.span.start.0) as usize;
            let end = (specifier.span.end.0 - import.span.start.0) as usize;
            ranges.push((start, end));
            let next_start = import
                .specifiers
                .get(index + 1)
                .map_or(declaration.len(), |next| {
                    (next.span.start.0 - import.span.start.0) as usize
                });
            if let Some(comma) = list_comma(&declaration[end..next_start]) {
                ranges.push((end + comma, end + comma + 1));
            } else if index > 0 {
                let previous_end =
                    (import.specifiers[index - 1].span.end.0 - import.span.start.0) as usize;
                if let Some(comma) = list_comma(&declaration[previous_end..start]) {
                    ranges.push((previous_end + comma, previous_end + comma + 1));
                }
            }
        }
        ranges.sort_unstable();
        let mut replacement = String::new();
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
            replacement,
        });
    }
    Ok(edits)
}

/// Byte offset of the comma joining two named specifiers inside `gap`.
///
/// Only commas outside comments and before the closing brace count. A `{` means the gap
/// has already left the named-specifier list, so the search stops instead of matching a
/// comma inside a nested clause.
fn list_comma(gap: &str) -> Option<usize> {
    let bytes = gap.as_bytes();
    let mut index = 0;
    let mut comma = None;
    while index < bytes.len() {
        match bytes[index] {
            b',' => {
                comma = Some(index);
                index += 1;
            }
            b'{' => return None,
            b'}' => return comma,
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index += 2;
                while index + 1 < bytes.len() && &bytes[index..index + 2] != b"*/" {
                    index += 1;
                }
                index += 2;
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                while index < bytes.len() && bytes[index] != b'\n' && bytes[index] != b'\r' {
                    index += 1;
                }
            }
            _ => index += 1,
        }
    }
    comma
}

/// True when `prefix` holds nothing but trivia up to the end of the callee reference.
///
/// The check accepts whitespace, comments, and an opening parenthesis, because none of
/// those can themselves be a reference to the macro binding. Any other token means the
/// replacement kept a read of the binding, so the import must stay.
fn callee_prefix(mut prefix: &str) -> bool {
    loop {
        prefix = prefix.trim_start_matches(|ch: char| ch.is_whitespace() || ch == '\u{feff}');
        if prefix.is_empty() {
            return true;
        }
        if let Some(rest) = prefix.strip_prefix('(') {
            prefix = rest;
        } else if let Some(rest) = prefix.strip_prefix("/*") {
            let Some(end) = rest.find("*/") else {
                return false;
            };
            prefix = &rest[end + 2..];
        } else if let Some(rest) = prefix.strip_prefix("//") {
            let Some(end) = rest.find(['\n', '\r', '\u{2028}', '\u{2029}']) else {
                return false;
            };
            prefix = &rest[end..];
        } else {
            return false;
        }
    }
}
