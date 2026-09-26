//! Resolves authored CSS before dependency discovery or template emission.
//!
//! [`render_authored`] collects registration requests, expands directives, resolves
//! theme references, and lowers conditionals. Use it for both document dependency
//! discovery and included authored nodes. It neither validates requested registration
//! names against the registry nor assigns template placement.

use stillwind_css::take_property_requirements;
use stillwind_design_system::ThemeIndex;
use stillwind_diagnostics::CompileErrorKind;

use super::super::authored_directives::expand_authored_directives;
use crate::CompilerOptions;
use crate::compiler::conditionals::lower;
use crate::identifiers::{IdentifierContext, IdentifierKind, builtin_prefix};

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
/// Returns an error for malformed requirements, unresolved theme references, or a
/// conditional unsupported by the chosen strategy. Dependencies already appended
/// remain in the accumulator on error; no partial CSS string is returned.
pub(super) fn render_authored(
    css: &str,
    theme: &ThemeIndex,
    required_properties: &mut Vec<String>,
    options: &CompilerOptions,
    identifiers: &IdentifierContext,
) -> Result<String, CompileErrorKind> {
    let (css, requirements) = take_property_requirements(css).map_err(|error| {
        CompileErrorKind::MalformedPropertyRequirement {
            offset: error.offset,
        }
    })?;
    required_properties.extend(requirements);
    theme
        .resolve_theme_references(&expand_authored_directives(
            &css,
            theme,
            required_properties,
            identifiers,
        )?)
        .map_err(|error| CompileErrorKind::InvalidThemeReference {
            offset: error.offset(),
            detail: error.to_string(),
        })?
        .ok_or(CompileErrorKind::UnresolvedThemeReference)
        .and_then(|css| lower(&css, options.use_css_if, identifiers))
        // The helper prefix is the built-in marker plus the role letter, so replacing the
        // marker puts every role's spelling under the configured custom-property prefix.
        .map(|css| {
            let marker = format!("--{}", builtin_prefix(IdentifierKind::ConditionHelper));
            css.replace(&marker, &format!("{}if", options.prefix))
        })
}
