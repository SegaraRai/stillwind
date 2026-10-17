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

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_source::{CssSourceName, Span};

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
        css: LeanStr,
        /// Where the CSS came from, when the parser could attribute it.
        origin: Option<Span>,
    },
    /// One `@layer` order statement, retained at its position in the template.
    LayerOrder {
        /// The order statement text.
        css: LeanStr,
        /// Where the statement came from, when known.
        origin: Option<Span>,
    },
    /// A wrapper such as `@layer utilities { ... }` around child nodes.
    Group {
        /// The at-rule header, including its name and prelude.
        header: LeanStr,
        /// Nodes emitted inside the wrapper, in order.
        children: Vec<Self>,
        /// Where the wrapper was authored, when known.
        origin: Option<Span>,
    },
    /// A CSS style rule containing declaration slots and authored declarations.
    Style {
        /// Authored selector list.
        header: LeanStr,
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
        keyframes: LeanStr,
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
    PropertyInitials(
        /// Registrations whose initial values the slot emits.
        PropertyInheritance,
    ),
}

/// Selects registered custom properties by their `inherits` descriptor.
///
/// A property that inherits needs its initial value only on the root of a tree, because
/// descendants inherit it. A property that does not inherit needs it on every element,
/// so that a value set on an ancestor does not reach descendants.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PropertyInheritance {
    /// Every property. Serialized as no slot argument.
    #[default]
    All,
    /// Properties registered with `inherits: true`. Serialized as `inherited`.
    Inherited,
    /// Properties registered with `inherits: false`. Serialized as `non-inherited`.
    NonInherited,
}

impl PropertyInheritance {
    /// Whether a property with the given `inherits` descriptor belongs to this
    /// selection.
    #[must_use]
    pub const fn includes(self, inherits: bool) -> bool {
        match self {
            Self::All => true,
            Self::Inherited => inherits,
            Self::NonInherited => !inherits,
        }
    }
}

/// The ordered output plan for one entry.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CssOutputTemplate {
    /// Host identity of the entry, shared by every output carrier.
    pub source_identity: CssSourceName,
    /// Output nodes; the order is significant and must be preserved.
    pub nodes: Vec<CssOutputNode>,
}

impl CssOutputTemplate {
    /// Concatenates the authored CSS of every node, ignoring slots and theme
    /// contributions.
    ///
    /// Used where a carrier needs only the user's own rules; the result is not a
    /// complete stylesheet.
    pub fn authored_css(&self) -> LeanString {
        authored_css(&self.nodes)
    }

    /// Writes the template as CSS, with each slot as its `@stillwind-slot` statement.
    ///
    /// Editors show this text for a template name. Theme blocks are omitted, because
    /// they define tokens rather than place output.
    pub fn outline(&self) -> LeanString {
        outline(&self.nodes)
    }

    /// Rewrites every `origin` span through `map`, keeping `None` as `None`.
    ///
    /// Callers use this when a template moves between coordinate systems, for example
    /// from a parsed buffer to original sources; a span the map cannot resolve is
    /// dropped rather than kept stale.
    pub fn map_origins(&mut self, mut map: impl FnMut(Span) -> Option<Span>) {
        visit_nodes(&mut self.nodes, &mut map);
    }
}

/// Writes a node list as CSS with its slot statements, as
/// [`CssOutputTemplate::outline`] describes.
fn outline(nodes: &[CssOutputNode]) -> LeanString {
    nodes
        .iter()
        .map(|node| match node {
            CssOutputNode::Authored { css, .. } | CssOutputNode::LayerOrder { css, .. } => {
                css.to_lean_string()
            }
            CssOutputNode::Style {
                header, children, ..
            }
            | CssOutputNode::Group {
                header, children, ..
            } => format_args!("{header}{{{}}}", outline(children)).to_lean_string(),
            CssOutputNode::Theme { .. } => LeanString::new(),
            CssOutputNode::Utilities => "@stillwind-slot utilities;".to_lean_string(),
            CssOutputNode::ThemeVariables => "@stillwind-slot theme;".to_lean_string(),
            CssOutputNode::Keyframes => "@stillwind-slot keyframes;".to_lean_string(),
            CssOutputNode::Properties => "@stillwind-slot properties;".to_lean_string(),
            CssOutputNode::PropertyInitials(inheritance) => match inheritance {
                PropertyInheritance::All => "@stillwind-slot property-initials;",
                PropertyInheritance::Inherited => "@stillwind-slot property-initials inherited;",
                PropertyInheritance::NonInherited => {
                    "@stillwind-slot property-initials non-inherited;"
                }
            }
            .to_lean_string(),
        })
        .collect()
}

/// Renders the authored part of a node list, wrapping groups in their headers.
fn authored_css(nodes: &[CssOutputNode]) -> LeanString {
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
            } => format_args!("{header}{{{}}}", authored_css(children)).to_lean_str(),
            CssOutputNode::Theme { .. }
            | CssOutputNode::Utilities
            | CssOutputNode::ThemeVariables
            | CssOutputNode::Keyframes
            | CssOutputNode::Properties
            | CssOutputNode::PropertyInitials(_) => LeanStr::new(),
        })
        .collect()
}

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
            | CssOutputNode::PropertyInitials(_) => {}
        }
    }
}
