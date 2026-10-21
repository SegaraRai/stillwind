//! Registries of the places where utility strings resolve: elements, keyframe frames,
//! keyframes uses, and view transitions.
//!
//! Each [`RegistryContext`] has its own utility, variant, and arbitrary-property
//! definitions. A `@stillwind-registry <context>, ... { ... }` block switches the
//! registries that the statements inside it change, as `pushd` and `popd` switch a
//! directory. Statements outside every block change the `default`, `keyframe`, and
//! `view-transition` registries, so those registries start with the same definitions.
//! The `animation` registry starts empty and rejects every arbitrary property, and only
//! blocks that name it add definitions. Blocks may nest, and the innermost block
//! decides the registries of a statement.
//!
//! Statements apply in registration order: statements in `@plugin` answers first, then
//! the other statements, each group in source order. An undefinition removes the
//! matching definitions that precede it in each of its registries, and a definition
//! adds to them in its own layer. Statements in `@plugin` answers therefore cannot
//! remove Stillwind definitions of the package or the stylesheet.
//!
//! [`RegistryStatements::parse`] reads the statements of one stylesheet, and
//! [`RegistryStatements::definitions`] applies them to one registry.
//! [`mask_registry_blocks`] removes the blocks from the text that the parsers of
//! stylesheet-wide metadata read.

use std::ops::Range;
use std::sync::Arc;

use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{
    CssNode, CssNodeKind, extract_directive_blocks, extract_order_statements, parse_structure,
};
use stillwind_diagnostics::CompileDiagnostic;

use super::utilities::parse_utility_definitions;
use super::{
    CustomUtility, CustomVariant, parse_stillwind_utility_definitions,
    parse_stillwind_variant_definitions,
};
use crate::parse_variant_definitions;

const REGISTRY: &str = "@stillwind-registry";

/// At-rules that a `@stillwind-registry` block may contain, without `@`.
const BLOCK_STATEMENTS: &[&str] = &[
    "utility",
    "stillwind-utility",
    "custom-variant",
    "variant",
    "stillwind-variant",
    "stillwind-arbitrary-property",
    "stillwind-undef-utility",
    "stillwind-undef-variant",
    "stillwind-undef-arbitrary-property",
    "stillwind-registry",
];

/// A place where utility strings resolve through their own definitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum RegistryContext {
    /// Serialized as `default`: element utilities, such as the inputs of `style()`.
    Default,
    /// Serialized as `animation`: the options of a keyframes use.
    Animation,
    /// Serialized as `keyframe`: the utilities of one `style.keyframes()` frame.
    Keyframe,
    /// Serialized as `view-transition`: the `group`, `old`, `new`, and `imagePair`
    /// inputs of `style.viewTransition()`.
    ViewTransition,
}

impl RegistryContext {
    /// Every context, in the order of [`Self::index`].
    pub const ALL: [Self; 4] = [
        Self::Default,
        Self::Animation,
        Self::Keyframe,
        Self::ViewTransition,
    ];

    /// The contexts that statements outside every `@stillwind-registry` block change.
    pub const ROOT: [Self; 3] = [Self::Default, Self::Keyframe, Self::ViewTransition];

    /// Returns the name that a `@stillwind-registry` header uses for this context.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Animation => "animation",
            Self::Keyframe => "keyframe",
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

/// A set of contexts, with bit [`RegistryContext::index`] set for each member.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Contexts(u8);

impl Contexts {
    fn of(contexts: impl IntoIterator<Item = RegistryContext>) -> Self {
        Self(
            contexts
                .into_iter()
                .fold(0, |bits, context| bits | (1 << context.index())),
        )
    }

    const fn contains(self, context: RegistryContext) -> bool {
        self.0 & (1 << context.index()) != 0
    }
}

/// One statement that changes registries.
#[derive(Clone, Debug, Eq, PartialEq)]
enum RegistryOperation {
    /// `@stillwind-undef-utility <pattern>;`
    UndefUtility(LeanStr),
    /// `@stillwind-undef-variant <pattern>;`
    UndefVariant(LeanStr),
    /// `@stillwind-undef-arbitrary-property <pattern>;`
    UndefArbitraryProperty(LeanStr),
    /// `@stillwind-arbitrary-property <pattern>;`, which accepts the matching arbitrary
    /// properties again.
    ArbitraryProperty(LeanStr),
    /// A `@stillwind-utility` definition.
    StillwindUtility(Arc<CustomUtility>),
    /// A `@utility` definition.
    Utility(Arc<CustomUtility>),
    /// A `@stillwind-variant` definition.
    StillwindVariant(Arc<CustomVariant>),
    /// A `@custom-variant` or `@variant` definition.
    Variant(Arc<CustomVariant>),
}

/// One statement and the registries that it changes.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Statement {
    contexts: Contexts,
    operation: RegistryOperation,
    /// Byte offset of the directive in the parsed text.
    offset: usize,
    /// Whether an `@plugin` answer contains the statement.
    plugin: bool,
}

impl Statement {
    /// Whether this statement registers before a statement outside `@plugin` answers at
    /// `offset`.
    fn registers_before(&self, offset: usize) -> bool {
        self.plugin || self.offset < offset
    }
}

/// An undefinition of a registry other than `default` that registers before a
/// definition outside every block, so that definition reaches the registry despite the
/// undefinition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BypassedUndefinition {
    /// Registry that the undefinition changes and the definition reaches.
    pub context: RegistryContext,
    /// Pattern of the undefinition as written.
    pub pattern: LeanStr,
}

/// Whether the pattern `pattern` matches the definition name `name`.
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

/// The definitions of one registry after its statements apply.
#[derive(Debug, Default)]
pub(crate) struct ContextDefinitions {
    /// `@stillwind-utility` definitions in registration order.
    pub(crate) stillwind_utilities: Vec<Arc<CustomUtility>>,
    /// `@stillwind-variant` definitions in registration order.
    pub(crate) stillwind_variants: Vec<Arc<CustomVariant>>,
    /// Plugin and stylesheet utility definitions in registration order.
    pub(crate) utilities: Vec<Arc<CustomUtility>>,
    /// Plugin and stylesheet variant definitions; the last definition of a name wins.
    pub(crate) variants: Vec<Arc<CustomVariant>>,
    /// Arbitrary property rules in registration order.
    pub(crate) arbitrary_properties: Vec<ArbitraryPropertyRule>,
}

impl ContextDefinitions {
    fn apply(&mut self, operation: &RegistryOperation) {
        match operation {
            RegistryOperation::UndefUtility(pattern) => {
                let keep = |utility: &Arc<CustomUtility>| !matches_pattern(pattern, &utility.name);
                self.stillwind_utilities.retain(keep);
                self.utilities.retain(keep);
            }
            RegistryOperation::UndefVariant(pattern) => {
                let keep = |variant: &Arc<CustomVariant>| !matches_pattern(pattern, &variant.name);
                self.stillwind_variants.retain(keep);
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
            RegistryOperation::StillwindUtility(utility) => {
                self.stillwind_utilities.push(Arc::clone(utility));
            }
            RegistryOperation::Utility(utility) => {
                self.utilities.push(Arc::clone(utility));
            }
            RegistryOperation::StillwindVariant(variant) => {
                self.stillwind_variants.push(Arc::clone(variant));
            }
            RegistryOperation::Variant(variant) => {
                self.variants.push(Arc::clone(variant));
            }
        }
    }
}

/// The definition and undefinition statements of one stylesheet, in registration order,
/// with the registries that each statement changes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RegistryStatements {
    statements: Vec<Statement>,
}

impl RegistryStatements {
    /// Reads the definition and undefinition statements of `css`, including those
    /// inside `@stillwind-registry` blocks. `in_plugin` returns true for the byte
    /// offsets of `@plugin` answers, which register before the other statements.
    ///
    /// The plugin flag of a `@utility` definition records `in_plugin` for its offset.
    ///
    /// # Errors
    ///
    /// Returns [`CompileDiagnostic::UnknownRegistryContext`] for a block header that
    /// names no context or an unknown context, and
    /// [`CompileDiagnostic::InvalidRegistryStatement`] for a block that contains
    /// anything other than definitions, undefinitions, arbitrary-property statements,
    /// and nested blocks.
    pub(crate) fn parse(
        css: &str,
        in_plugin: impl Fn(usize) -> bool,
    ) -> Result<Self, CompileDiagnostic> {
        let scopes = registry_scopes(css)?;
        let contexts_at = |offset: usize| {
            scopes
                .iter()
                .filter(|scope| scope.body.contains(&offset))
                .min_by_key(|scope| scope.body.len())
                .map_or(Contexts::of(RegistryContext::ROOT), |scope| scope.contexts)
        };
        let mut positioned: Vec<(usize, RegistryOperation)> = Vec::new();
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
            if css.contains(directive) {
                positioned.extend(
                    extract_order_statements(css, directive)
                        .into_iter()
                        .map(|(offset, pattern)| (offset, operation(pattern))),
                );
            }
        }
        positioned.extend(
            parse_stillwind_utility_definitions(css)
                .into_iter()
                .filter_map(|utility| {
                    Some((
                        utility.position?,
                        RegistryOperation::StillwindUtility(Arc::new(utility)),
                    ))
                }),
        );
        positioned.extend(parse_utility_definitions(css).into_iter().map(
            |(offset, mut utility)| {
                utility.plugin = in_plugin(offset);
                (offset, RegistryOperation::Utility(Arc::new(utility)))
            },
        ));
        positioned.extend(
            parse_stillwind_variant_definitions(css)
                .into_iter()
                .filter_map(|variant| {
                    Some((
                        variant.position?,
                        RegistryOperation::StillwindVariant(Arc::new(variant)),
                    ))
                }),
        );
        positioned.extend(
            parse_variant_definitions(css)
                .into_iter()
                .map(|(offset, variant)| (offset, RegistryOperation::Variant(Arc::new(variant)))),
        );
        positioned.sort_by_key(|(offset, _)| (!in_plugin(*offset), *offset));
        Ok(Self {
            statements: positioned
                .into_iter()
                .map(|(offset, operation)| Statement {
                    contexts: contexts_at(offset),
                    operation,
                    offset,
                    plugin: in_plugin(offset),
                })
                .collect(),
        })
    }

    /// Applies the statements that change `context`, in registration order, to the
    /// initial definitions of `context`. The `animation` registry starts with a rule
    /// that rejects every arbitrary property; the other registries start empty.
    pub(crate) fn definitions(&self, context: RegistryContext) -> ContextDefinitions {
        let mut definitions = ContextDefinitions::default();
        if context == RegistryContext::Animation {
            definitions
                .arbitrary_properties
                .push(ArbitraryPropertyRule {
                    pattern: "*".into(),
                    accepted: false,
                });
        }
        for statement in self
            .statements
            .iter()
            .filter(|statement| statement.contexts.contains(context))
        {
            definitions.apply(&statement.operation);
        }
        definitions
    }

    /// Whether a `@stillwind-undef-utility` statement that changes `context` matches
    /// the utility name `name`.
    pub(crate) fn undefines_utility(&self, context: RegistryContext, name: &str) -> bool {
        self.statements.iter().any(|statement| {
            statement.contexts.contains(context)
                && matches!(
                    &statement.operation,
                    RegistryOperation::UndefUtility(pattern) if matches_pattern(pattern, name)
                )
        })
    }

    /// Returns the undefinition that a definition outside every block and outside
    /// `@plugin` answers bypasses. The definition has the name `name`, defines a
    /// utility when `utility` is true and a variant otherwise, and starts at `offset`.
    ///
    /// For `keyframe` and then `view-transition`, the last undefinition of that
    /// registry whose pattern matches `name` is checked. It is returned when it
    /// registers before the definition and does not change `default`. A later
    /// undefinition removes the definition again, and an undefinition that also changes
    /// `default` removes the earlier definitions there too, so neither is bypassed.
    pub(crate) fn bypassed_undefinition(
        &self,
        utility: bool,
        name: &str,
        offset: usize,
    ) -> Option<BypassedUndefinition> {
        [RegistryContext::Keyframe, RegistryContext::ViewTransition]
            .into_iter()
            .find_map(|context| {
                let (statement, pattern) = self
                    .statements
                    .iter()
                    .filter(|statement| statement.contexts.contains(context))
                    .filter_map(|statement| {
                        let ((RegistryOperation::UndefUtility(pattern), true)
                        | (RegistryOperation::UndefVariant(pattern), false)) =
                            (&statement.operation, utility)
                        else {
                            return None;
                        };
                        matches_pattern(pattern, name).then_some((statement, pattern))
                    })
                    .max_by_key(|(statement, _)| (!statement.plugin, statement.offset))?;
                (statement.registers_before(offset)
                    && !statement.contexts.contains(RegistryContext::Default))
                .then(|| BypassedUndefinition {
                    context,
                    pattern: pattern.clone(),
                })
            })
    }
}

/// The body of one `@stillwind-registry` block and the contexts that its header names.
struct Scope {
    /// Byte range inside the braces.
    body: Range<usize>,
    contexts: Contexts,
}

/// Returns the bodies of every `@stillwind-registry` block of `css`, including nested
/// blocks, and checks their headers and statements.
fn registry_scopes(css: &str) -> Result<Vec<Scope>, CompileDiagnostic> {
    let mut scopes = Vec::new();
    if !css.contains(REGISTRY) {
        return Ok(scopes);
    }
    for block in extract_directive_blocks(css, REGISTRY) {
        let start = block.range.start;
        let nodes = parse_structure(&css[block.range.clone()]);
        for node in &nodes {
            collect_scope(css, start, node, &mut scopes)?;
        }
    }
    Ok(scopes)
}

/// Records the scope of the `@stillwind-registry` node `node`, whose ranges are
/// relative to `base`, then the scopes of the blocks nested in it.
fn collect_scope(
    css: &str,
    base: usize,
    node: &CssNode,
    scopes: &mut Vec<Scope>,
) -> Result<(), CompileDiagnostic> {
    let CssNodeKind::AtRule {
        prelude,
        block: Some(block),
        ..
    } = &node.kind
    else {
        return Ok(());
    };
    let offset = base + node.range.start;
    let mut contexts = Vec::new();
    for name in css[base + prelude.start..base + prelude.end]
        .split(',')
        .map(str::trim)
    {
        let Some(context) = RegistryContext::from_name(name) else {
            return Err(CompileDiagnostic::UnknownRegistryContext {
                offset,
                context: name.into(),
            });
        };
        contexts.push(context);
    }
    scopes.push(Scope {
        body: base + block.contents.start..base + block.contents.end,
        contexts: Contexts::of(contexts),
    });
    for child in &block.children {
        let name = match &child.kind {
            CssNodeKind::AtRule { name, .. } if BLOCK_STATEMENTS.contains(&name.as_str()) => name,
            CssNodeKind::AtRule { name, .. } => {
                return Err(CompileDiagnostic::InvalidRegistryStatement {
                    offset: base + child.range.start,
                    statement: format_args!("@{name}").to_lean_str(),
                });
            }
            _ => {
                return Err(CompileDiagnostic::InvalidRegistryStatement {
                    offset: base + child.range.start,
                    statement: css[base + child.range.start..base + child.range.end]
                        .trim()
                        .into(),
                });
            }
        };
        if name == "stillwind-registry" {
            collect_scope(css, base, child, scopes)?;
        }
    }
    Ok(())
}

/// Returns `css` with every top-level `@stillwind-registry` block replaced by spaces,
/// or `None` when `css` contains no block. Line breaks are kept, so offsets and line
/// positions keep their meaning.
pub(crate) fn mask_registry_blocks(css: &str) -> Option<String> {
    if !css.contains(REGISTRY) {
        return None;
    }
    let ranges = extract_directive_blocks(css, REGISTRY)
        .into_iter()
        .map(|block| block.range)
        .collect::<Vec<_>>();
    let mut output = String::with_capacity(css.len());
    for (offset, character) in css.char_indices() {
        if character == '\n' || !ranges.iter().any(|range| range.contains(&offset)) {
            output.push(character);
        } else {
            output.extend(std::iter::repeat_n(' ', character.len_utf8()));
        }
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn statements(css: &str) -> RegistryStatements {
        RegistryStatements::parse(css, |_| false).unwrap()
    }

    fn utility_bodies(definitions: &ContextDefinitions) -> Vec<&str> {
        definitions
            .stillwind_utilities
            .iter()
            .chain(&definitions.utilities)
            .map(|utility| utility.body.as_str())
            .collect()
    }

    #[test]
    fn root_statements_change_every_registry_except_animation() {
        let statements = statements(
            "@utility a { color: red; }
@stillwind-registry keyframe, animation {
  @stillwind-undef-utility a;
  @utility a { color: blue; }
}
@utility b { color: green; }",
        );
        assert_eq!(
            utility_bodies(&statements.definitions(RegistryContext::Default)),
            ["color: red;", "color: green;"]
        );
        assert_eq!(
            utility_bodies(&statements.definitions(RegistryContext::Keyframe)),
            ["color: blue;", "color: green;"]
        );
        assert_eq!(
            utility_bodies(&statements.definitions(RegistryContext::Animation)),
            ["color: blue;"]
        );
        assert_eq!(
            utility_bodies(&statements.definitions(RegistryContext::ViewTransition)),
            ["color: red;", "color: green;"]
        );
    }

    #[test]
    fn nested_blocks_switch_registries_and_root_undefinitions_apply_in_order() {
        let statements = statements(
            "@utility a-* { color: --value([color]); }
@stillwind-undef-utility a-*;
@utility a-x { color: red; }
@stillwind-registry keyframe {
  @stillwind-registry default { @utility d { color: blue; } }
  @utility k { color: green; }
}",
        );
        assert_eq!(
            utility_bodies(&statements.definitions(RegistryContext::Default)),
            ["color: red;", "color: blue;"]
        );
        assert_eq!(
            utility_bodies(&statements.definitions(RegistryContext::Keyframe)),
            ["color: red;", "color: green;"]
        );
        assert!(statements.undefines_utility(RegistryContext::Default, "a-y"));
        assert!(!statements.undefines_utility(RegistryContext::Animation, "a-y"));
    }

    #[test]
    fn plugin_statements_register_before_the_others() {
        let css = "@stillwind-undef-utility p; @utility p { color: red; }";
        let plugin_start = css.find("@utility").unwrap();
        let statements = RegistryStatements::parse(css, |offset| offset >= plugin_start).unwrap();
        assert_eq!(
            utility_bodies(&statements.definitions(RegistryContext::Default)),
            [] as [&str; 0]
        );
    }

    #[test]
    fn rejects_unknown_contexts_and_statements_that_registries_cannot_hold() {
        for (css, expected) in [
            ("@stillwind-registry motion { }", "motion"),
            ("@stillwind-registry { }", ""),
            (
                "@stillwind-registry keyframe { @stillwind-registry other { } }",
                "other",
            ),
        ] {
            assert!(
                matches!(
                    RegistryStatements::parse(css, |_| false),
                    Err(CompileDiagnostic::UnknownRegistryContext { context, .. }) if context == expected
                ),
                "{css}"
            );
        }
        for (css, expected) in [
            (
                "@stillwind-registry keyframe { @theme { --x: 1; } }",
                "@theme",
            ),
            (
                "@stillwind-registry default { .a { color: red; } }",
                ".a { color: red; }",
            ),
        ] {
            assert!(
                matches!(
                    RegistryStatements::parse(css, |_| false),
                    Err(CompileDiagnostic::InvalidRegistryStatement { statement, .. }) if statement == expected
                ),
                "{css}"
            );
        }
    }

    #[test]
    fn animation_starts_closed_to_arbitrary_properties() {
        let statements = statements(
            "@stillwind-registry animation { @stillwind-arbitrary-property animation-*; }",
        );
        let rules = statements
            .definitions(RegistryContext::Animation)
            .arbitrary_properties;
        assert!(accepts_arbitrary_property(&rules, "animation-delay"));
        assert!(!accepts_arbitrary_property(&rules, "color"));
        assert!(accepts_arbitrary_property(
            &statements
                .definitions(RegistryContext::Default)
                .arbitrary_properties,
            "color"
        ));
    }

    #[test]
    fn masks_blocks_and_keeps_offsets() {
        let css = "@utility a { color: red; }\n@stillwind-registry keyframe {\n  @utility 日本 { color: blue; }\n}\n@utility b { color: green; }";
        let masked = mask_registry_blocks(css).unwrap();
        assert_eq!(masked.len(), css.len());
        assert_eq!(parse_utility_definitions(&masked).len(), 2);
        assert_eq!(masked.find("@utility b"), css.find("@utility b"));
        assert_eq!(mask_registry_blocks("@utility a { }"), None);
    }

    #[test]
    fn matches_undefinition_patterns() {
        assert!(matches_pattern("*", "-translate-*"));
        assert!(matches_pattern("duration-*", "duration-*"));
        assert!(matches_pattern("duration-*", "duration-initial"));
        assert!(!matches_pattern("translate-*", "-translate-*"));
        assert!(!matches_pattern("duration", "duration-*"));
    }
}
