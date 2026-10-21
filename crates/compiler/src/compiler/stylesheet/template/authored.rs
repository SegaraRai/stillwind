//! Resolves authored CSS before dependency discovery or template emission.
//!
//! [`render_authored`] collects registration requests, expands directives, resolves
//! theme references, and lowers conditionals. Use it for both document dependency
//! discovery and included authored nodes. It neither validates requested registration
//! names against the registry nor assigns template placement.

use std::ops::Range;

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{CssNode, CssNodeKind, parse_structure};
use stillwind_design_system::{ThemeIndex, take_property_requirements};
use stillwind_diagnostics::CompileDiagnostic;
use stillwind_source::ReplaceText;

use super::super::authored_directives::expand_authored_directives;
use crate::CompilerOptions;
use crate::compiler::conditionals::lower;
use crate::identifiers::{IdentifierContext, IdentifierKind, default_prefix};

/// Returns expanded authored CSS and appends its explicit registration dependencies.
///
/// `required_properties` retains its existing entries and may gain duplicate names.
/// Requirements written in the input precede those collected from `@apply` expansion.
///
/// `options` selects runtime-conditional lowering and the generated conditional-helper
/// prefix. The final rewrite replaces every `--sw-if` substring in the resulting text,
/// including authored occurrences; it is not restricted to generated names. `css` and
/// `theme` are unchanged. Unexpandable directives may remain in the output, so
/// successful rendering does not imply complete CSS validation.
///
/// # Errors
///
/// Returns an error for malformed requirements, malformed theme references, or a
/// conditional unsupported by the chosen strategy. Dependencies already appended
/// remain in the accumulator on error; no partial CSS string is returned.
pub(super) fn render_authored(
    css: &str,
    theme: &ThemeIndex,
    required_properties: &mut Vec<LeanStr>,
    options: &CompilerOptions,
    identifiers: &IdentifierContext,
) -> Result<LeanString, CompileDiagnostic> {
    let (css, requirements) = take_property_requirements(css).map_err(|error| {
        CompileDiagnostic::MalformedPropertyRequirement {
            offset: error.offset,
        }
    })?;
    required_properties.extend(requirements);
    let css = expand_authored_directives(&css, theme, required_properties, identifiers)?;
    let css = theme.resolve_css_functions(&css).unwrap_or(css);
    lower(
        &resolve_theme_references(&css, theme)?,
        options.use_css_if,
        identifiers,
    )
    // The helper prefix is the default prefix plus the role letter, so replacing the
    // marker puts every role's spelling under the configured custom-property prefix.
    .map(|lowered| {
        let css = lowered.css;
        let marker =
            format_args!("--{}", default_prefix(IdentifierKind::ConditionHelper)).to_lean_string();
        css.replace_text(
            &marker,
            &format_args!("{}if", options.prefix).to_lean_string(),
        )
    })
}

/// Lowers the `--theme()` references of the authored stylesheet `css`.
///
/// A declaration whose value names an inactive token without a usable fallback is
/// omitted. A rule or at-rule whose selector or prelude names one is omitted with its
/// block. Invalid input thus contributes nothing, and stylesheet lint reports the
/// reference as `invalid-theme-reference`.
///
/// # Errors
///
/// Returns a malformed reference with its byte offset in `css`.
fn resolve_theme_references(
    css: &str,
    theme: &ThemeIndex,
) -> Result<LeanString, CompileDiagnostic> {
    if !css
        .as_bytes()
        .windows(b"--theme".len())
        .any(|window| window.eq_ignore_ascii_case(b"--theme"))
    {
        return Ok(css.to_lean_string());
    }
    let mut edits = Vec::new();
    collect_theme_edits(css, &parse_structure(css), theme, &mut edits)?;
    let mut output = LeanString::with_capacity(css.len());
    let mut cursor = 0;
    for (range, replacement) in edits {
        output.push_str(&css[cursor..range.start]);
        output.push_str(&replacement);
        cursor = range.end;
    }
    output.push_str(&css[cursor..]);
    Ok(output)
}

/// Appends the edits that lower the references of `nodes`, in source order and without
/// overlap.
fn collect_theme_edits(
    css: &str,
    nodes: &[CssNode],
    theme: &ThemeIndex,
    edits: &mut Vec<(Range<usize>, LeanString)>,
) -> Result<(), CompileDiagnostic> {
    for node in nodes {
        let (part, block) = match &node.kind {
            CssNodeKind::Declaration { value, .. } => (value, None),
            CssNodeKind::StyleRule { selector, block } => (selector, Some(block)),
            CssNodeKind::AtRule { prelude, block, .. } => (prelude, block.as_ref()),
        };
        let resolved = theme
            .resolve_theme_references(&css[part.clone()])
            .map_err(|error| CompileDiagnostic::InvalidThemeReference {
                offset: part.start + error.offset(),
                detail: error.to_lean_str(),
            })?;
        match resolved {
            None => {
                edits.push((node.range.clone(), LeanString::new()));
                continue;
            }
            Some(resolved) if resolved != css[part.clone()] => {
                edits.push((part.clone(), resolved));
            }
            Some(_) => {}
        }
        if let Some(block) = block {
            collect_theme_edits(css, &block.children, theme, edits)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::parse_theme;
    use stillwind_source::{CssSourceName, FileId, SourceFile};

    use super::*;

    #[test]
    fn unresolved_references_omit_their_declaration_or_rule() {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            "@theme { --color-red: red; --breakpoint-md: 48rem; }",
        )
        .unwrap();
        let theme = parse_theme(&source, &CssSourceName::from("test.css")).unwrap();
        for (css, expected) in [
            (
                ".a{color:--theme(--color-nope);display:flex}",
                ".a{display:flex}",
            ),
            (
                ".a{color:--theme(--color-red);background:--theme(--nope, blue)}",
                ".a{color:var(--color-red);background:blue}",
            ),
            (
                ".a{&:hover{color:--theme(--nope)}}.b{color:red}",
                ".a{&:hover{}}.b{color:red}",
            ),
            (
                "@media (width >= --theme(--breakpoint-nope)) { .a{color:red} } .b{}",
                " .b{}",
            ),
            (
                "@media (width >= --theme(--breakpoint-md)) { .a{color:--theme(--nope)} }",
                "@media (width >= var(--breakpoint-md)) { .a{} }",
            ),
            (
                ".a{content:'--theme(--nope)'}",
                ".a{content:'--theme(--nope)'}",
            ),
        ] {
            assert_eq!(
                resolve_theme_references(css, &theme).unwrap(),
                expected,
                "{css}"
            );
        }
        assert!(matches!(
            resolve_theme_references(".a{color:--theme(nope)}", &theme),
            Err(CompileDiagnostic::InvalidThemeReference { offset: 9, .. })
        ));
    }
}
