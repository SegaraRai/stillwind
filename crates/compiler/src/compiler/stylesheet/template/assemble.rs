//! Coordinates template assembly and validates destinations for generated output.
//!
//! [`render_mapped`] selects support, checks its slots, and emits the template;
//! [`render`] adapts raw CSS without contribution metadata. This module owns error
//! precedence and destination checks; dependency selection and node traversal belong
//! to `support` and `emit`, respectively.

use stillwind_css::{CssOutputNode, CssOutputTemplate};
use stillwind_design_system::{CssSourceGraph, ThemeIndex};
use stillwind_diagnostics::CompileDiagnostic;

use super::emit::emit;
use super::support::SupportSelection;
use crate::identifiers::IdentifierContext;
use crate::{CompilerOptions, MappedStylesheet};

/// Assembles raw CSS against `template` and returns the document or carrier text.
///
/// `theme` supplies the definitions indexed by the template. `utility_css` has no
/// generated-keyframe classification or explicit registration metadata; callers with
/// generated artifacts must use [`render_mapped`]. `document` includes authored rules
/// when true. Neither the template nor the input CSS is modified.
///
/// # Errors
///
/// Propagates all errors from [`render_mapped`], including property-usage parsing
/// and conditional-lowering failures. No partial CSS is returned.
pub(in super::super) fn render(
    template: &CssOutputTemplate,
    theme: &ThemeIndex,
    utility_css: &str,
    options: &CompilerOptions,
    document: bool,
    identifiers: &IdentifierContext,
) -> Result<String, CompileDiagnostic> {
    render_mapped(
        template,
        theme,
        &MappedStylesheet {
            css: utility_css.into(),
            ..MappedStylesheet::default()
        },
        options,
        document,
        None,
        identifiers,
    )
    .map(|output| output.css)
}

/// Renders a stylesheet with source provenance, for a document or a single carrier.
///
/// `template` must index the supplied `theme`; `utility_css` carries rule text,
/// separately classified keyframes, and explicit registration dependencies. The
/// returned CSS follows template order, including repeated slot occurrences.
/// `document` includes independent authored rules when true; carriers still retain
/// declarations inside template style nodes and reserve layer order. `options` sets
/// conditional lowering and generated names. Utility rules must already be lowered;
/// producer keyframes are inserted as supplied, without another lowering pass.
///
/// `sources` resolves template origins to source buffers. Without it, template text
/// is unmapped; mappings already attached to input rules and keyframes are retained.
///
/// # Errors
///
/// Returns [`CompileDiagnostic`] for malformed or unknown registration requirements,
/// unresolved authored theme references, invalid property-usage syntax, or a missing
/// destination for a nonempty generated contribution. Conditional lowering can also
/// fail for included authored CSS, selected theme tokens, or selected theme keyframes.
/// Registration destinations are checked before lowering theme fragments; theme and
/// keyframe destinations are checked before node emission. The utilities destination
/// is checked after emission. Inputs are unchanged and errors return no partial CSS.
pub(crate) fn render_mapped(
    template: &CssOutputTemplate,
    theme: &ThemeIndex,
    utility_css: &MappedStylesheet,
    options: &CompilerOptions,
    document: bool,
    sources: Option<&CssSourceGraph>,
    identifiers: &IdentifierContext,
) -> Result<MappedStylesheet, CompileDiagnostic> {
    let selection = SupportSelection::select(
        template,
        theme,
        utility_css,
        options,
        document,
        sources,
        identifiers,
    )?;
    let properties = &selection.properties;
    if !properties.registrations.is_empty()
        && !has_slot(&template.nodes, &CssOutputNode::Properties)
    {
        return Err(CompileDiagnostic::MissingPropertySlot {
            slot: "properties".into(),
        });
    }
    if !properties.initials.is_empty()
        && !has_slot(&template.nodes, &CssOutputNode::PropertyInitials)
    {
        return Err(CompileDiagnostic::MissingPropertySlot {
            slot: "property-initials".into(),
        });
    }
    let support = selection.render(utility_css)?;
    for (needed, slot, name) in [
        (
            !support.theme_variables.css.is_empty(),
            CssOutputNode::ThemeVariables,
            "theme",
        ),
        (
            !support.keyframes.css.is_empty(),
            CssOutputNode::Keyframes,
            "keyframes",
        ),
    ] {
        if needed && !has_slot(&template.nodes, &slot) {
            return Err(CompileDiagnostic::MissingPropertySlot { slot: name.into() });
        }
    }
    let (css, slots) = emit(
        template,
        theme,
        utility_css,
        options,
        document,
        sources,
        &support,
        identifiers,
    )?;
    if !utility_css.css.is_empty() && slots == 0 {
        return Err(CompileDiagnostic::MissingUtilitiesSlot);
    }
    Ok(css)
}

/// Tests for `slot` anywhere in the node list, including style and group children.
fn has_slot(nodes: &[CssOutputNode], slot: &CssOutputNode) -> bool {
    nodes.iter().any(|node| {
        node == slot
            || matches!(node,
                CssOutputNode::Group { children, .. } | CssOutputNode::Style { children, .. }
                if has_slot(children, slot)
            )
    })
}
