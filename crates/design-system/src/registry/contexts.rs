//! Context registries: definitions that apply only where a utility string is evaluated
//! inside a keyframe, a keyframes use, or a view transition.
//!
//! A `@stillwind-registry <context>, ... { ... }` block changes the definitions of the
//! contexts that its header names. Each context starts with the default definitions of
//! every layer, and the blocks apply after them: blocks in `@plugin` answers first,
//! then the other blocks, each group in source order. Inside a block, statements apply
//! in source order. An undefinition removes the matching definitions that precede it in
//! the context, and a definition adds to the context in its own layer.
//!
//! [`split_registry_blocks`] separates the blocks from the text that the default
//! parsers read. [`ContextDefinitions::apply`] applies one block to a context.

use std::ops::Range;
use std::sync::Arc;

use lean_string::LeanStr;

use stillwind_css::{extract_directive_blocks, extract_order_statements};
use stillwind_diagnostics::CompileDiagnostic;

use super::utilities::parse_utility_definitions;
use super::{CustomUtility, CustomVariant, parse_builtin_utility_definitions};
use crate::parse_variant_definitions;

const REGISTRY: &str = "@stillwind-registry";

/// A place where utility strings resolve through definitions other than the default
/// ones.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum RegistryContext {
    /// Serialized as `keyframe`: the utilities of one `style.keyframes()` frame.
    Keyframe,
    /// Serialized as `animation`: the options of a keyframes use.
    Animation,
    /// Serialized as `view-transition`: the `group`, `old`, `new`, and `imagePair`
    /// inputs of `style.viewTransition()`.
    ViewTransition,
}

impl RegistryContext {
    /// Every context, in the order of [`Self::index`].
    pub const ALL: [Self; 3] = [Self::Keyframe, Self::Animation, Self::ViewTransition];

    /// Returns the name that a `@stillwind-registry` header uses for this context.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Keyframe => "keyframe",
            Self::Animation => "animation",
            Self::ViewTransition => "view-transition",
        }
    }

    /// Returns the context that a `@stillwind-registry` header names, or `None` for an
    /// unknown name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|context| context.name() == name)
    }

    /// Returns the position of this context in [`Self::ALL`].
    pub(crate) const fn index(self) -> usize {
        self as usize
    }
}

/// One `@stillwind-registry` block: the contexts it changes and its statements in
/// source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RegistryBlock {
    /// Contexts that the header names, without duplicates.
    contexts: Vec<RegistryContext>,
    /// Whether an `@plugin` answer contains the block.
    plugin: bool,
    /// Statements in source order.
    operations: Vec<RegistryOperation>,
}

impl RegistryBlock {
    /// Whether this block changes `context`.
    pub(crate) fn changes(&self, context: RegistryContext) -> bool {
        self.contexts.contains(&context)
    }
}

/// One statement of a `@stillwind-registry` block.
#[derive(Clone, Debug, Eq, PartialEq)]
enum RegistryOperation {
    /// `@stillwind-undef-utility <pattern>;`
    UndefUtility(LeanStr),
    /// `@stillwind-undef-variant <pattern>;`
    UndefVariant(LeanStr),
    /// `@stillwind-undef-arbitrary-property <pattern>;`
    UndefArbitraryProperty(LeanStr),
    /// `@stillwind-arbitrary-property <pattern>;`, which accepts the matching
    /// arbitrary properties again.
    ArbitraryProperty(LeanStr),
    /// A `@stillwind-utility` definition.
    BuiltinUtility(Arc<CustomUtility>),
    /// A `@utility` definition.
    Utility(Arc<CustomUtility>),
    /// A `@custom-variant` or `@variant` definition.
    Variant(Arc<CustomVariant>),
}

/// Whether the undefinition pattern `pattern` matches the definition name `name`.
///
/// `*` matches every name. A pattern that ends with `*` matches the names that start
/// with the text before the `*`, so `duration-*` matches the definitions `duration-*`
/// and `duration-initial`, and not `-duration-*`. Any other pattern matches only the
/// same name.
fn matches_pattern(pattern: &str, name: &str) -> bool {
    pattern
        .strip_suffix('*')
        .map_or(pattern == name, |prefix| name.starts_with(prefix))
}

/// Whether an arbitrary property pattern accepts or rejects the matching properties.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ArbitraryPropertyRule {
    /// Property pattern as [`matches_pattern`] reads it.
    pattern: LeanStr,
    /// Whether the matching properties are accepted.
    accepted: bool,
}

/// Returns whether `rules` accept the arbitrary property `property`. The last matching
/// rule decides, and a property that no rule matches is accepted.
pub(crate) fn accepts_arbitrary_property(rules: &[ArbitraryPropertyRule], property: &str) -> bool {
    rules
        .iter()
        .rev()
        .find(|rule| matches_pattern(&rule.pattern, property))
        .is_none_or(|rule| rule.accepted)
}

/// The definitions of one context while blocks apply to it.
#[derive(Debug)]
pub(crate) struct ContextDefinitions {
    /// Built-in utility definitions in registration order.
    pub(crate) builtin_utilities: Vec<Arc<CustomUtility>>,
    /// Built-in variant definitions in registration order.
    pub(crate) builtin_variants: Vec<Arc<CustomVariant>>,
    /// Plugin and stylesheet utility definitions in registration order.
    pub(crate) utilities: Vec<Arc<CustomUtility>>,
    /// Plugin and stylesheet variant definitions; the last definition of a name wins.
    pub(crate) variants: Vec<Arc<CustomVariant>>,
    /// Arbitrary property rules in source order.
    pub(crate) arbitrary_properties: Vec<ArbitraryPropertyRule>,
}

impl ContextDefinitions {
    /// Applies the statements of `block` in source order.
    pub(crate) fn apply(&mut self, block: &RegistryBlock) {
        for operation in &block.operations {
            match operation {
                RegistryOperation::UndefUtility(pattern) => {
                    let keep =
                        |utility: &Arc<CustomUtility>| !matches_pattern(pattern, &utility.name);
                    self.builtin_utilities.retain(keep);
                    self.utilities.retain(keep);
                }
                RegistryOperation::UndefVariant(pattern) => {
                    let keep =
                        |variant: &Arc<CustomVariant>| !matches_pattern(pattern, &variant.name);
                    self.builtin_variants.retain(keep);
                    self.variants.retain(keep);
                }
                RegistryOperation::UndefArbitraryProperty(pattern) => {
                    self.arbitrary_properties.push(ArbitraryPropertyRule {
                        pattern: pattern.clone(),
                        accepted: false,
                    });
                }
                RegistryOperation::ArbitraryProperty(pattern) => {
                    self.arbitrary_properties.push(ArbitraryPropertyRule {
                        pattern: pattern.clone(),
                        accepted: true,
                    });
                }
                RegistryOperation::BuiltinUtility(utility) => {
                    self.builtin_utilities.push(Arc::clone(utility));
                }
                RegistryOperation::Utility(utility) => self.utilities.push(Arc::clone(utility)),
                RegistryOperation::Variant(variant) => self.variants.push(Arc::clone(variant)),
            }
        }
    }
}

/// Finds the top-level `@stillwind-registry` blocks of `css` and parses their
/// statements.
///
/// Returns the text that the default parsers read, in which every block is replaced by
/// spaces with its line breaks kept, so offsets keep their meaning. The text is `None`
/// when `css` contains no block. The blocks are ordered as they apply: blocks for which
/// `in_plugin` returns true for the directive offset come first, then the others, each
/// in source order. A block nested in another block is part of the outer block's text
/// and is not parsed.
///
/// # Errors
///
/// Returns [`CompileDiagnostic::UnknownRegistryContext`] for a header that names no
/// context or an unknown context.
pub(crate) fn split_registry_blocks(
    css: &str,
    in_plugin: impl Fn(usize) -> bool,
) -> Result<(Option<String>, Vec<RegistryBlock>), CompileDiagnostic> {
    if !css.contains(REGISTRY) {
        return Ok((None, Vec::new()));
    }
    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut blocks = Vec::new();
    for directive in extract_directive_blocks(css, REGISTRY) {
        if ranges
            .last()
            .is_some_and(|outer| outer.end >= directive.range.end)
        {
            continue;
        }
        let mut contexts = Vec::new();
        for name in directive.header.split(',').map(str::trim) {
            let Some(context) = RegistryContext::from_name(name) else {
                return Err(CompileDiagnostic::UnknownRegistryContext {
                    offset: directive.range.start,
                    context: name.into(),
                });
            };
            if !contexts.contains(&context) {
                contexts.push(context);
            }
        }
        let body = blank_outside(css, &directive.body);
        let plugin = in_plugin(directive.range.start);
        blocks.push(RegistryBlock {
            contexts,
            plugin,
            operations: parse_operations(&body, &in_plugin),
        });
        ranges.push(directive.range);
    }
    blocks.sort_by_key(|block| !block.plugin);
    let masked = blank_where(css, |offset| {
        ranges.iter().any(|range| range.contains(&offset))
    });
    Ok((Some(masked), blocks))
}

/// Parses the statements of one block body. `body` has the length of the whole text,
/// with everything outside the body replaced by spaces.
fn parse_operations(body: &str, in_plugin: &impl Fn(usize) -> bool) -> Vec<RegistryOperation> {
    let mut operations: Vec<(usize, RegistryOperation)> = Vec::new();
    for (directive, operation) in [
        (
            "@stillwind-undef-utility",
            RegistryOperation::UndefUtility as fn(LeanStr) -> RegistryOperation,
        ),
        ("@stillwind-undef-variant", RegistryOperation::UndefVariant),
        (
            "@stillwind-undef-arbitrary-property",
            RegistryOperation::UndefArbitraryProperty,
        ),
        (
            "@stillwind-arbitrary-property",
            RegistryOperation::ArbitraryProperty,
        ),
    ] {
        operations.extend(
            extract_order_statements(body, directive)
                .into_iter()
                .map(|(offset, pattern)| (offset, operation(pattern))),
        );
    }
    operations.extend(
        parse_builtin_utility_definitions(body)
            .into_iter()
            .filter_map(|utility| {
                Some((
                    utility.builtin_order?,
                    RegistryOperation::BuiltinUtility(Arc::new(utility)),
                ))
            }),
    );
    operations.extend(
        parse_utility_definitions(body)
            .into_iter()
            .map(|(offset, mut utility)| {
                utility.plugin = in_plugin(offset);
                (offset, RegistryOperation::Utility(Arc::new(utility)))
            }),
    );
    operations.extend(
        parse_variant_definitions(body)
            .into_iter()
            .map(|(offset, variant)| (offset, RegistryOperation::Variant(Arc::new(variant)))),
    );
    operations.sort_by_key(|(offset, _)| *offset);
    operations
        .into_iter()
        .map(|(_, operation)| operation)
        .collect()
}

/// Returns `css` with every character outside `keep` replaced by spaces, keeping line
/// breaks.
fn blank_outside(css: &str, keep: &Range<usize>) -> String {
    blank_where(css, |offset| !keep.contains(&offset))
}

/// Returns `css` with each character at an offset for which `blank` returns true replaced
/// by as many spaces as its UTF-8 length. Line breaks are kept, so the result has the
/// byte length and line positions of `css`.
fn blank_where(css: &str, blank: impl Fn(usize) -> bool) -> String {
    let mut output = String::with_capacity(css.len());
    for (offset, character) in css.char_indices() {
        if character == '\n' || !blank(offset) {
            output.push(character);
        } else {
            output.extend(std::iter::repeat_n(' ', character.len_utf8()));
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocks(css: &str) -> (Option<String>, Vec<RegistryBlock>) {
        split_registry_blocks(css, |_| false).unwrap()
    }

    #[test]
    fn masks_blocks_and_keeps_offsets() {
        let css = "@utility a { color: red; }\n@stillwind-registry keyframe, animation {\n  @utility a { color: blue; }\n}\n@utility b { color: green; }";
        let (masked, blocks) = blocks(css);
        let masked = masked.unwrap();
        assert_eq!(masked.len(), css.len());
        assert_eq!(parse_utility_definitions(&masked).len(), 2);
        assert_eq!(masked.find("@utility b"), css.find("@utility b"));
        let [block] = blocks.as_slice() else {
            panic!("one block: {blocks:?}");
        };
        assert_eq!(
            block.contexts,
            [RegistryContext::Keyframe, RegistryContext::Animation]
        );
        assert!(matches!(
            block.operations.as_slice(),
            [RegistryOperation::Utility(utility)] if utility.body.contains("blue")
        ));
    }

    #[test]
    fn keeps_statement_order_and_rejects_unknown_contexts() {
        let (_, blocks) = blocks(
            "@stillwind-registry animation { @stillwind-undef-utility *; @stillwind-utility duration-* { animation-duration: --value(integer); } @stillwind-undef-arbitrary-property *; @stillwind-arbitrary-property animation-*; }",
        );
        assert!(matches!(
            blocks[0].operations.as_slice(),
            [
                RegistryOperation::UndefUtility(all),
                RegistryOperation::BuiltinUtility(_),
                RegistryOperation::UndefArbitraryProperty(_),
                RegistryOperation::ArbitraryProperty(animation),
            ] if all == "*" && animation == "animation-*"
        ));
        assert!(matches!(
            split_registry_blocks("@stillwind-registry motion { }", |_| false),
            Err(CompileDiagnostic::UnknownRegistryContext { context, .. }) if context == "motion"
        ));
        assert!(matches!(
            split_registry_blocks("@stillwind-registry { }", |_| false),
            Err(CompileDiagnostic::UnknownRegistryContext { context, .. }) if context.is_empty()
        ));
    }

    #[test]
    fn matches_undefinition_patterns() {
        assert!(matches_pattern("*", "-translate-*"));
        assert!(matches_pattern("duration-*", "duration-*"));
        assert!(matches_pattern("duration-*", "duration-initial"));
        assert!(!matches_pattern("translate-*", "-translate-*"));
        assert!(!matches_pattern("duration", "duration-*"));
        let rules = [
            ArbitraryPropertyRule {
                pattern: "*".into(),
                accepted: false,
            },
            ArbitraryPropertyRule {
                pattern: "animation-*".into(),
                accepted: true,
            },
        ];
        assert!(accepts_arbitrary_property(&rules, "animation-delay"));
        assert!(!accepts_arbitrary_property(&rules, "color"));
        assert!(accepts_arbitrary_property(&[], "color"));
    }
}
