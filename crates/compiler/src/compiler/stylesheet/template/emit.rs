//! Emits selected contributions in template order with source provenance.
//!
//! [`emit`] preserves authored wrappers and carrier layer reservations. Anonymous
//! layers share identities across document and carrier walks. [`resolve_origin`]
//! resolves retained source buffers for both template and support fragments. Pass
//! prepared support to `emit`; it does not discover dependencies, validate missing
//! slots, or lower the supplied utility and support fragments.

use std::sync::Arc;

use cssparser::{Parser, Token};

use stillwind_css::{CssOutputNode, CssOutputTemplate};
use stillwind_design_system::{CssSourceGraph, ThemeIndex};
use stillwind_diagnostics::CompileErrorKind;
use stillwind_source::{SourceFile, Span};

use super::authored::render_authored;
use super::support::SupportOutput;
use crate::identifiers::{IdentifierContext, LayerContext};
use crate::{CompilerOptions, MappedStylesheet};

/// Walks the template and returns mapped CSS with the number of utilities slots visited.
///
/// `template`, `theme`, `options`, and `support` must describe the same selection.
/// `utility_css.css` is inserted at every utilities slot; its separate keyframes must
/// already be included in `support`. Existing fragment mappings retain their origins
/// with generated offsets adjusted to each insertion.
///
/// `document` includes independent authored nodes when true. Carriers retain style-node
/// declarations and layer reservations. `sources` resolves template origins; missing
/// buffers leave those nodes unmapped. Each call starts fresh layer and slot counters,
/// including slots with empty contributions, and leaves all inputs unchanged.
///
/// # Errors
///
/// Returns malformed authored registration requests, unresolved authored theme
/// references, or conditional-lowering errors. No partial output is returned.
#[allow(clippy::too_many_arguments)] // One borrow per compilation-wide input.
pub(super) fn emit(
    template: &CssOutputTemplate,
    theme: &ThemeIndex,
    utility_css: &MappedStylesheet,
    options: &CompilerOptions,
    document: bool,
    sources: Option<&CssSourceGraph>,
    support: &SupportOutput,
    identifiers: &IdentifierContext,
) -> Result<(MappedStylesheet, usize), CompileErrorKind> {
    let mut context = Context {
        theme,
        utility_css,
        options,
        document,
        sources,
        support,
        identifiers,
        slots: 0,
        layer_identity: LayerContext {
            template: template.source_identity.clone(),
            namespace: options.class_namespace.clone(),
        },
        anonymous_layers: 0,
    };
    let css = context.nodes(&template.nodes)?;
    Ok((css, context.slots))
}

/// Prepared contributions and traversal counters for one document or carrier.
///
/// Recursive walks share this state so filtering carrier output cannot renumber
/// anonymous layers or lose nested utilities-slot counts.
struct Context<'a> {
    /// Theme used to resolve authored nodes; must match the prepared support.
    theme: &'a ThemeIndex,
    /// Mapped utility contribution inserted at utilities slots.
    utility_css: &'a MappedStylesheet,
    /// Generated-name configuration and runtime-conditional strategy for template expansion.
    options: &'a CompilerOptions,
    /// Whether authored document rules are included rather than only carrier support.
    document: bool,
    /// Utilities slots visited so far, starting at zero and including empty insertions.
    slots: usize,
    /// Prepared support fragments inserted at their template slots.
    support: &'a SupportOutput,
    /// Name allocation shared with the rest of this compilation.
    identifiers: &'a IdentifierContext,
    /// Context one anonymous layer name is generated from; shared by walks of the same
    /// template and class namespace, so a document and its carriers agree on layer names.
    layer_identity: LayerContext,
    /// Next zero-based anonymous-layer index in depth-first template order.
    /// Advances even for groups whose body a carrier omits.
    anonymous_layers: usize,
    /// Optional retained CSS source graph for resolving authored origin spans.
    sources: Option<&'a CssSourceGraph>,
}

impl Context<'_> {
    /// Renders `nodes` in rule context using the current traversal counters.
    ///
    /// Delegates to [`Self::nodes_in_context`] with authored declarations excluded from
    /// carrier rule context. Existing counter values are retained and advanced, so recursive
    /// calls continue the same anonymous-layer numbering and utilities-slot count.
    ///
    /// # Errors
    ///
    /// Propagates authored rendering errors from [`Self::nodes_in_context`]. Counters may
    /// already have advanced on failure; no partial mapped output is returned.
    fn nodes(&mut self, nodes: &[CssOutputNode]) -> Result<MappedStylesheet, CompileErrorKind> {
        self.nodes_in_context(nodes, false)
    }

    /// Renders nodes in source order, advancing the shared layer and slot counters.
    ///
    /// `declarations` retains authored nodes inside style rules even for carriers, because
    /// omitting surrounding declarations changes the generated rule. Style children use
    /// declaration context; at-rule children use rule context. Every visited slot receives
    /// its contribution, including repeated slots.
    ///
    /// Anonymous layers are numbered before filtering their bodies. Carriers retain an
    /// empty layer as a statement to preserve the document's layer order; other empty
    /// at-rule groups are omitted. Style rules are omitted when their rendered body is
    /// empty. Document output retains at-rule groups even when empty.
    ///
    /// # Errors
    ///
    /// Returns malformed authored registration requests, unresolved theme references,
    /// or conditional-lowering errors. Counters may already have advanced on failure;
    /// no partial mapped output is returned.
    fn nodes_in_context(
        &mut self,
        nodes: &[CssOutputNode],
        declarations: bool,
    ) -> Result<MappedStylesheet, CompileErrorKind> {
        let mut output = MappedStylesheet::default();
        for node in nodes {
            match node {
                CssOutputNode::LayerOrder { css, origin } => {
                    output.text(css, resolve_origin(self.sources, *origin));
                }
                CssOutputNode::Authored { css, origin } if self.document || declarations => {
                    let authored = render_authored(
                        css,
                        self.theme,
                        &mut Vec::new(),
                        self.options,
                        self.identifiers,
                    )?;
                    output.text(&authored, resolve_origin(self.sources, *origin));
                }
                CssOutputNode::Authored { .. } | CssOutputNode::Theme { .. } => {}
                CssOutputNode::ThemeVariables => {
                    output.append(&self.support.theme_variables);
                }
                CssOutputNode::Keyframes => {
                    output.append(&self.support.keyframes);
                }
                CssOutputNode::Properties => {
                    output.text(&self.support.properties.registrations, None);
                }
                CssOutputNode::PropertyInitials => {
                    output.text(&self.support.properties.initials, None);
                }
                CssOutputNode::Style {
                    header,
                    children,
                    origin,
                } => {
                    let body = self.nodes_in_context(children, true)?;
                    if !body.css.is_empty() {
                        output.text(header, resolve_origin(self.sources, *origin));
                        output.text("{", None);
                        output.append(&body);
                        output.text("}", None);
                    }
                }
                CssOutputNode::Utilities => {
                    self.slots += 1;
                    output.append(self.utility_css);
                }
                CssOutputNode::Group {
                    header,
                    children,
                    origin,
                } => {
                    // Number before filtering the body so separate carriers assign the
                    // same name to the same anonymous layer, even when one omits its rules.
                    let header = if is_anonymous_layer(header) {
                        let index = self.anonymous_layers;
                        self.anonymous_layers += 1;
                        let name = self.identifiers.layer_name(&self.layer_identity)?;
                        format!("@layer {}-{index}", name.custom_ident())
                    } else {
                        header.clone()
                    };
                    let body = self.nodes(children)?;
                    if !self.document && body.css.trim().is_empty() && is_layer(&header) {
                        output.text(&header, resolve_origin(self.sources, *origin));
                        output.text(";", None);
                    } else if self.document || !body.css.trim().is_empty() {
                        output.text(&header, resolve_origin(self.sources, *origin));
                        output.text("{", None);
                        output.append(&body);
                        output.text("}", None);
                    }
                }
            }
        }
        Ok(output)
    }
}

/// Looks up a span's retained source buffer without changing its byte range.
///
/// Returns the first source whose file id matches `span.file`, cloning its `Arc` and
/// preserving the supplied span. A missing span, source graph, or matching file yields
/// `None`; callers can still emit unmapped text. This lookup does not validate that
/// the span's byte range lies within the source buffer.
pub(super) fn resolve_origin(
    sources: Option<&CssSourceGraph>,
    span: Option<Span>,
) -> Option<(Arc<SourceFile>, Span)> {
    let span = span?;
    let source = sources?
        .sources
        .iter()
        .find(|source| source.source.id() == span.file)?;
    Some((source.source.clone(), span))
}

/// Recognizes a case-insensitive `@layer` token with no following non-trivia tokens.
///
/// CSS whitespace and comments are skipped, so `@layer /* comment */` is anonymous.
/// Headers with a name or another trailing token return false.
fn is_anonymous_layer(header: &str) -> bool {
    let mut parser = Parser::new(header);
    matches!(parser.next(), Ok(Token::AtKeyword(name)) if name.eq_ignore_ascii_case("layer"))
        && parser.is_exhausted()
}

/// Tests whether the first non-trivia token is a case-insensitive `@layer` at-keyword.
///
/// Trailing tokens are not validated; the caller supplies a parsed template header.
fn is_layer(header: &str) -> bool {
    let mut parser = Parser::new(header);
    matches!(parser.next(), Ok(Token::AtKeyword(name)) if name.eq_ignore_ascii_case("layer"))
}
