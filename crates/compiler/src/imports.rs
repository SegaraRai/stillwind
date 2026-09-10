//! Import edits derived from resolved references and successful macro replacements.

use stillwind_ir::ModuleImports;
use stillwind_source::{SourceError, SourceFile, Span};

use crate::SourceTransform;

#[derive(Debug, thiserror::Error)]
pub enum ImportPlanError {
    #[error("invalid macro import source span: {0}")]
    Source(#[from] SourceError),
    #[error("invalid macro import metadata at {span:?}")]
    InvalidMetadata { span: Span },
    #[error("macro binding `{local}` has a reference that was not transformed at {span:?}")]
    UntransformedReference { local: String, span: Span },
    #[error(
        "macro binding `{local}` has references without verified source correspondence at {span:?}"
    )]
    UnmappedReference { local: String, span: Span },
}

/// Remove macro bindings only when every resolved read was successfully replaced.
/// Edits replace declarations
/// as a unit so adjacent removed specifiers cannot create overlapping edits.
///
/// # Errors
/// Rejects invalid metadata, untransformed macro reads, and unverified projection
/// references. No partial edit plan is returned on failure.
pub fn plan_macro_import_removals(
    source: &SourceFile,
    imports: &ModuleImports,
    lowered: &[SourceTransform],
) -> Result<Vec<SourceTransform>, ImportPlanError> {
    let mut edits = Vec::new();
    for import in &imports.macros {
        let declaration = source.slice(import.span)?;
        let mut previous_end = import.span.start;
        let mut valid = true;
        for specifier in &import.specifiers {
            source.slice(specifier.span)?;
            valid &= specifier.span.file == import.span.file
                && specifier.span.start >= previous_end
                && specifier.span.end <= import.span.end;
            previous_end = specifier.span.end;
        }
        if !valid {
            return Err(ImportPlanError::InvalidMetadata { span: import.span });
        }
        for specifier in &import.specifiers {
            if !specifier.is_macro {
                continue;
            }
            if specifier.has_unmapped_references {
                return Err(ImportPlanError::UnmappedReference {
                    local: specifier.local.clone(),
                    span: specifier.span,
                });
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
                    return Err(ImportPlanError::UntransformedReference {
                        local: specifier.local.clone(),
                        span: *reference,
                    });
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

// Gaps contain trivia and delimiters. A brace ends the named-specifier list;
// commas inside comments or beyond it must never be used as separators.
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

// A lowering removes the macro callee, but may copy argument expressions into
// its replacement. Only the leading callee reference is therefore consumed.
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
