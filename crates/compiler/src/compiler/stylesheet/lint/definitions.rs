//! Definition at-rules of one stylesheet, as the design-system registry reads them.
//!
//! The registry finds `@utility`, `@custom-variant`, `@stillwind-variant`, and
//! `@variant` definitions outside style rules, including inside wrappers such as
//! `@layer`. A top-level `@variant` defines a variant only when its body contains
//! `@slot`.

use std::ops::Range;

use stillwind_css::{CssBlock, CssNode, CssNodeKind};

/// The kind of a definition at-rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DefinitionKind {
    /// `@utility`.
    Utility,
    /// `@custom-variant`, or `@variant` with `@slot` outside a style rule.
    Variant,
    /// `@stillwind-variant`, which declares a variant in Stillwind syntax.
    StillwindVariant,
}

/// One definition at-rule of a source.
pub(super) struct Definition<'a> {
    /// The at-rule node.
    pub(super) node: &'a CssNode,
    /// Directive name without `@`, such as `custom-variant`.
    pub(super) directive: &'a str,
    /// Kind of the definition.
    pub(super) kind: DefinitionKind,
    /// Byte range of the name, from the start of the prelude to its first whitespace.
    pub(super) name: Range<usize>,
    /// Byte range of the prelude after the name, without surrounding whitespace. It is
    /// empty when the prelude is only the name.
    pub(super) rest: Range<usize>,
    /// Block of a block definition, or `None` for a statement such as a
    /// `@custom-variant` shorthand.
    pub(super) block: Option<&'a CssBlock>,
}

impl Definition<'_> {
    /// Returns whether the name contains an unescaped `*`, which captures a value.
    pub(super) fn has_capture(&self, text: &str) -> bool {
        let name = &text[self.name.clone()];
        name.char_indices()
            .any(|(index, character)| character == '*' && !name[..index].ends_with('\\'))
    }
}

/// Returns the definitions of `nodes`, the top-level nodes of `text`, in authored order.
pub(super) fn definitions<'a>(nodes: &'a [CssNode], text: &str) -> Vec<Definition<'a>> {
    let mut found = Vec::new();
    collect(nodes, text, &mut found);
    found
}

fn collect<'a>(nodes: &'a [CssNode], text: &str, found: &mut Vec<Definition<'a>>) {
    for node in nodes {
        let CssNodeKind::AtRule {
            name: directive,
            prelude,
            block,
        } = &node.kind
        else {
            continue;
        };
        let kind = match directive.as_str() {
            "utility" => DefinitionKind::Utility,
            "custom-variant" => DefinitionKind::Variant,
            "variant"
                if block
                    .as_ref()
                    .is_some_and(|block| text[block.contents.clone()].contains("@slot")) =>
            {
                DefinitionKind::Variant
            }
            "stillwind-variant" => DefinitionKind::StillwindVariant,
            "theme" | "variant" | "tailwind" => continue,
            name if name.starts_with("stillwind-") => continue,
            _ => {
                if let Some(block) = block {
                    collect(&block.children, text, found);
                }
                continue;
            }
        };
        let header = &text[prelude.clone()];
        let name_end = header.find(char::is_whitespace).unwrap_or(header.len());
        let rest = &header[name_end..];
        let rest_start = prelude.start + name_end + (rest.len() - rest.trim_start().len());
        found.push(Definition {
            node,
            directive,
            kind,
            name: prelude.start..prelude.start + name_end,
            rest: rest_start..rest_start + rest.trim().len(),
            block: block.as_ref(),
        });
    }
}
