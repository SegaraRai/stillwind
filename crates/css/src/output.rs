//! Source-ordered CSS output, independent of the host's module/chunk policy.

use stillwind_source::Span;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CssOutputError {
    #[error(
        "compile-time @{name} definitions cannot be conditional (byte {offset}); import definitions unconditionally and put conditions around @stillwind-slot utilities;"
    )]
    ConditionalDefinition { offset: usize, name: String },
    #[error("unsupported Stillwind output slot at byte {offset}: {name}")]
    Slot { offset: usize, name: String },
    #[error("invalid CSS output block at byte {offset}")]
    Block { offset: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CssOutputNode {
    Authored {
        css: String,
        origin: Option<Span>,
    },
    LayerOrder {
        css: String,
        origin: Option<Span>,
    },
    Group {
        header: String,
        children: Vec<Self>,
        origin: Option<Span>,
    },
    Theme {
        tokens: Vec<usize>,
        keyframes: String,
        origin: Option<Span>,
    },
    Utilities,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CssOutputTemplate {
    /// Host identity of the entry, shared by every output carrier.
    pub source_identity: String,
    pub nodes: Vec<CssOutputNode>,
}

impl CssOutputTemplate {
    pub fn authored_css(&self) -> String {
        authored_css(&self.nodes)
    }

    pub fn map_origins(&mut self, mut map: impl FnMut(Span) -> Option<Span>) {
        fn visit_nodes(nodes: &mut [CssOutputNode], map: &mut impl FnMut(Span) -> Option<Span>) {
            for node in nodes {
                match node {
                    CssOutputNode::Authored { origin, .. }
                    | CssOutputNode::LayerOrder { origin, .. }
                    | CssOutputNode::Theme { origin, .. } => *origin = origin.and_then(&mut *map),
                    CssOutputNode::Group {
                        origin, children, ..
                    } => {
                        *origin = origin.and_then(&mut *map);
                        visit_nodes(children, map);
                    }
                    CssOutputNode::Utilities => {}
                }
            }
        }
        visit_nodes(&mut self.nodes, &mut map);
    }
}

fn authored_css(nodes: &[CssOutputNode]) -> String {
    nodes
        .iter()
        .map(|node| match node {
            CssOutputNode::Authored { css, .. } | CssOutputNode::LayerOrder { css, .. } => {
                css.clone()
            }
            CssOutputNode::Group {
                header, children, ..
            } => {
                format!("{header}{{{}}}", authored_css(children))
            }
            CssOutputNode::Theme { .. } | CssOutputNode::Utilities => String::new(),
        })
        .collect()
}
