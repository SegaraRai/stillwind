//! Registration requirements embedded in utility definitions and authored CSS.
//!
//! The CSS parser supplies directive boundaries. This module validates names and removes
//! requirement directives before their declarations are emitted.

use std::ops::Range;

use lean_string::{LeanStr, LeanString};

use stillwind_css::{CssNode, CssNodeKind, parse_structure, whitespace_separated_ranges};

/// A malformed property requirement and its byte offset in the supplied CSS body.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("invalid property requirement at byte {offset}")]
pub struct PropertyRequirementError {
    /// Byte offset of the directive within the supplied CSS body.
    pub offset: usize,
}

/// One registration requirement directive and its named properties.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PropertyRequirement {
    /// Byte range of the full directive in its CSS body.
    range: Range<usize>,
    /// Required custom-property names in authored order.
    names: Vec<LeanStr>,
}

/// Appends requirements from `nodes` and their descendants in source order. Node ranges
/// must index `css`. Unknown names are left for registry validation.
///
/// # Errors
///
/// Returns the offset of a directive with a block, missing semicolon, or invalid names.
fn collect_requirements(
    css: &str,
    nodes: &[CssNode],
    requirements: &mut Vec<PropertyRequirement>,
) -> Result<(), PropertyRequirementError> {
    for node in nodes {
        if let CssNodeKind::AtRule {
            name,
            prelude,
            block,
        } = &node.kind
            && name == "stillwind-require-properties"
        {
            let names = whitespace_separated_ranges(&css[prelude.clone()])
                .into_iter()
                .map(|range| {
                    LeanStr::from(&css[prelude.start + range.start..prelude.start + range.end])
                })
                .collect::<Vec<_>>();
            if block.is_some()
                || names.is_empty()
                || names.iter().any(|name| {
                    !name.starts_with("--")
                        || name.len() == 2
                        || !name.chars().all(|character| {
                            character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
                        })
                })
                || css.as_bytes().get(node.range.end.saturating_sub(1)) != Some(&b';')
            {
                return Err(PropertyRequirementError {
                    offset: node.range.start,
                });
            }
            requirements.push(PropertyRequirement {
                range: node.range.clone(),
                names,
            });
        }
        collect_requirements(css, node.children(), requirements)?;
    }
    Ok(())
}

/// Removes property requirement directives and returns their names in source order.
/// Other CSS bytes are preserved. Duplicate names are retained for the consumer to
/// resolve against its registry. Directive-shaped strings and comments are ignored.
///
/// # Errors
///
/// Returns the byte offset of the first malformed directive.
pub fn take_property_requirements(
    css: &str,
) -> Result<(LeanString, Vec<LeanStr>), PropertyRequirementError> {
    let mut requests = Vec::new();
    if css.contains("@stillwind-require-properties") {
        collect_requirements(css, &parse_structure(css), &mut requests)?;
    }
    let mut output = LeanString::new();
    let mut properties = Vec::new();
    let mut cursor = 0;
    for PropertyRequirement { range, names } in requests {
        output.push_str(&css[cursor..range.start]);
        cursor = range.end;
        properties.extend(names);
    }
    output.push_str(&css[cursor..]);
    Ok((output, properties))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requirements_are_lexical_and_reject_non_names() {
        let css = r".x{content:'@stillwind-require-properties --fake;';@stillwind-require-properties --real;}";
        let (lowered, names) = take_property_requirements(css).unwrap();
        assert_eq!(names, ["--real"]);
        assert!(lowered.contains("'@stillwind-require-properties --fake;'"));
        assert!(take_property_requirements("@stillwind-require-properties red;").is_err());
        assert!(take_property_requirements("@stillwind-require-properties --x{}").is_err());
    }
}
