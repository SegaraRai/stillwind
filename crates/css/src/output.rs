//! Source-ordered CSS output, independent of the host's module/chunk policy.
//!
//! A [`CssOutputTemplate`] is the plan for one output carrier: an ordered tree of nodes
//! saying which authored CSS, layer statements, groups, theme declarations and
//! generated utilities appear, and in what order. The design system builds templates
//! while expanding imports; the compiler renders them and decides what each carrier
//! contains.
//!
//! Node order is source order and must not be sorted. `origin` spans point into the
//! original editable source, never a generated buffer, and may be `None` when a node
//! has no authored origin, for example the utilities slot.

use stillwind_source::Span;

/// Failure to build an output template from authored CSS.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CssOutputError {
    /// A compile-time definition appeared inside a conditional group, where it cannot
    /// be attributed to one output order.
    #[error(
        "compile-time @{name} definitions cannot be conditional (byte {offset}); import definitions unconditionally and put conditions around @stillwind-slot utilities;"
    )]
    ConditionalDefinition {
        /// Byte offset of the offending definition.
        offset: usize,
        /// Directive name, such as `utility` or `variant`.
        name: String,
    },
    /// The template used an output slot Stillwind does not know.
    #[error("unsupported Stillwind output slot at byte {offset}: {name}")]
    Slot {
        /// Byte offset of the slot reference.
        offset: usize,
        /// Slot name as written.
        name: String,
    },
    /// A declaration slot appeared outside a style rule, or a rule slot inside one.
    #[error("Stillwind output slot {name} has an invalid CSS context")]
    SlotContext {
        /// Slot whose rule/declaration context is invalid.
        name: String,
    },
    /// A block could not be parsed or was never closed.
    #[error("invalid CSS output block at byte {offset}")]
    Block {
        /// Byte offset where parsing failed.
        offset: usize,
    },
}

/// One node of an output template.
///
/// The compiler consumes the variants in tree order; a carrier that omits a node (for
/// example authored CSS outside the document carrier) still keeps its position in the
/// order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CssOutputNode {
    /// Authored CSS copied through, usually the user's own rules.
    Authored {
        /// The CSS text, in authored order.
        css: String,
        /// Where the CSS came from, when the parser could attribute it.
        origin: Option<Span>,
    },
    /// One `@layer` order statement, retained at its position in the template.
    LayerOrder {
        /// The order statement text.
        css: String,
        /// Where the statement came from, when known.
        origin: Option<Span>,
    },
    /// A wrapper such as `@layer utilities { ... }` around child nodes.
    Group {
        /// The at-rule header, including its name and prelude.
        header: String,
        /// Nodes emitted inside the wrapper, in order.
        children: Vec<Self>,
        /// Where the wrapper was authored, when known.
        origin: Option<Span>,
    },
    /// A CSS style rule containing declaration slots and authored declarations.
    Style {
        /// Authored selector list.
        header: String,
        /// Declaration contributions in source order.
        children: Vec<Self>,
        /// Source location of the rule.
        origin: Option<Span>,
    },
    /// One `@theme` block's definitions, selected for emission through output slots.
    Theme {
        /// Indices into the compiled theme's token list for the tokens authored inside
        /// this block. The consumer resolves them, so an index is only valid for the
        /// theme the template was built from.
        tokens: Vec<usize>,
        /// Keyframe definitions authored inside this block. Referenced definitions
        /// contribute to the keyframes slot independently of the token declarations.
        keyframes: String,
        /// Where the block was authored, when known.
        origin: Option<Span>,
    },
    /// The slot where generated utilities are inserted.
    ///
    /// Carries no CSS of its own: the compiler knows which utilities the importing
    /// carrier selected, and renders the same template per carrier.
    Utilities,
    /// Emits live theme declarations inside the authored style rule containing the slot.
    ThemeVariables,
    /// Emits selected theme and generated animation definitions at this layer and
    /// condition, preserving the assembler's contribution order.
    Keyframes,
    /// Emits required custom-property registrations.
    Properties,
    /// Emits fallback initialization declarations inside an authored style rule.
    PropertyInitials,
}

/// The ordered output plan for one entry.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CssOutputTemplate {
    /// Host identity of the entry, shared by every output carrier.
    pub source_identity: String,
    /// Output nodes; the order is significant and must be preserved.
    pub nodes: Vec<CssOutputNode>,
}

impl CssOutputTemplate {
    /// Concatenates the authored CSS of every node, ignoring slots and theme
    /// contributions.
    ///
    /// Used where a carrier needs only the user's own rules; the result is not a
    /// complete stylesheet.
    pub fn authored_css(&self) -> String {
        authored_css(&self.nodes)
    }

    /// Rewrites every `origin` span through `map`, keeping `None` as `None`.
    ///
    /// Callers use this when a template moves between coordinate systems, for example
    /// from a parsed buffer to original sources; a span the map cannot resolve is
    /// dropped rather than kept stale.
    pub fn map_origins(&mut self, mut map: impl FnMut(Span) -> Option<Span>) {
        fn visit_nodes(nodes: &mut [CssOutputNode], map: &mut impl FnMut(Span) -> Option<Span>) {
            for node in nodes {
                match node {
                    CssOutputNode::Authored { origin, .. }
                    | CssOutputNode::LayerOrder { origin, .. }
                    | CssOutputNode::Theme { origin, .. } => {
                        *origin = origin.and_then(&mut *map);
                    }
                    CssOutputNode::Style {
                        origin, children, ..
                    }
                    | CssOutputNode::Group {
                        origin, children, ..
                    } => {
                        *origin = origin.and_then(&mut *map);
                        visit_nodes(children, map);
                    }
                    CssOutputNode::Utilities
                    | CssOutputNode::ThemeVariables
                    | CssOutputNode::Keyframes
                    | CssOutputNode::Properties
                    | CssOutputNode::PropertyInitials => {}
                }
            }
        }
        visit_nodes(&mut self.nodes, &mut map);
    }
}

/// Renders the authored part of a node list, wrapping groups in their headers.
fn authored_css(nodes: &[CssOutputNode]) -> String {
    nodes
        .iter()
        .map(|node| match node {
            CssOutputNode::Authored { css, .. } | CssOutputNode::LayerOrder { css, .. } => {
                css.clone()
            }
            CssOutputNode::Style {
                header, children, ..
            }
            | CssOutputNode::Group {
                header, children, ..
            } => {
                format!("{header}{{{}}}", authored_css(children))
            }
            CssOutputNode::Theme { .. }
            | CssOutputNode::Utilities
            | CssOutputNode::ThemeVariables
            | CssOutputNode::Keyframes
            | CssOutputNode::Properties
            | CssOutputNode::PropertyInitials => String::new(),
        })
        .collect()
}
