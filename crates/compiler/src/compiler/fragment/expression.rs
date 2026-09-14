//! Semantic style contributions before framework keys and style format are selected.
//!
//! [`Fragment::from_emission`] builds an ordered tree from finalized class units.
//! [`Emitted`] retains class guards and property groups; [`Property`] and [`ValueExpr`]
//! carry source provenance and effect proofs. Folding and field selection belong to
//! `fold`; rendering is deferred so rejected candidates never allocate helper imports.

use stillwind_ir::ConditionalKind;
use stillwind_source::Span;

use super::super::expression_render::is_plain;
use super::super::props::{StyleEmission, StyleEmissionPart};
use crate::generated::{Code, code};
use crate::runtime::{RuntimeBindings, RuntimeHelper};

/// A source expression or a runtime call whose binding is allocated only on output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ValueExpr {
    /// An opaque value whose shape and effects have not been proven.
    Source(
        /// Complete expression code; source-derived text retains its occurrence mappings.
        Code,
    ),
    /// A compiler-owned call; arguments retain their evaluation order.
    Call {
        /// True only when source arguments satisfy the frontend's effect-free model.
        /// This is a proof supplied by lowering, not an inspection of the helper body.
        pure: bool,
        /// Runtime operation; its local binding is allocated only during rendering.
        helper: RuntimeHelper,
        /// Complete argument expressions in JavaScript evaluation order, without separators.
        arguments: Vec<Code>,
        /// Original-source UTF-8 span selecting the helper import realm.
        span: Span,
    },
}

impl ValueExpr {
    /// Produces expression code with retained argument mappings.
    /// A deferred call registers its helper at `span`; an opaque source value is cloned
    /// without registering helpers. Call this only after selecting a renderable plan.
    pub(crate) fn render(&self, runtime: &mut RuntimeBindings<'_>) -> Code {
        match self {
            Self::Source(expression) => expression.clone(),
            Self::Call {
                helper,
                arguments,
                span,
                ..
            } => {
                let helper = runtime.require(*helper, *span);
                code![helper, "(", Code::join(arguments.clone(), ","), ")"]
            }
        }
    }
}

impl From<String> for ValueExpr {
    /// Wraps text as an opaque value with no exact source mappings or effect proof.
    fn from(value: String) -> Self {
        Self::Source(value.into())
    }
}

impl From<&str> for ValueExpr {
    /// Copies text into an opaque value with no exact source mappings or effect proof.
    fn from(value: &str) -> Self {
        Self::Source(value.into())
    }
}

/// A source-owned property whose value has not been converted or serialized.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Property {
    /// Decoded property name, without JavaScript quoting.
    pub name: String,
    /// Verbatim UTF-8 expression text equal to the original source slice at `span`.
    pub value: String,
    /// True when the frontend permits reordering under its effect heuristic;
    /// false includes unknown effects. Getter and implicit-coercion effects are excluded.
    pub pure: bool,
    /// Original-source UTF-8 value span identifying this occurrence and its import realm.
    pub span: Span,
}

/// Lowered class contributions with the guards and property groups needed for folding.
/// Source-derived code keeps occurrence mappings; this tree does not yet select
/// symbolic fragment keys or framework props.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Emitted {
    /// Class-name contributions in one leaf.
    Leaf {
        /// Class strings in contribution order, joined with spaces and quoted by the renderer.
        names: Vec<String>,
    },
    /// A dynamic class value or deferred runtime lookup.
    Expression(
        /// Source expression or deferred helper call.
        ValueExpr,
    ),
    /// A JavaScript array expression.
    Array(
        /// Elements in source order; `None` marks an omitted or base-folded contribution.
        Vec<Option<Emitted>>,
    ),
    /// A JavaScript conditional expression.
    Conditional {
        /// Complete test expression with original whitespace and occurrence mappings.
        code: Code,
        /// Effect-free proof for the test alone; it may be omitted only if both arms are empty.
        pure: bool,
        /// Whether serialization uses a ternary or logical conjunction.
        kind: ConditionalKind,
        /// Truthy contributions; `None` contributes no value but does not itself erase the test.
        consequent: Option<Box<Emitted>>,
        /// Falsy contributions for a ternary; `None` for conjunction or an omitted arm.
        alternate: Option<Box<Emitted>>,
    },
    /// An object literal inside a conditional expression.
    Group(
        /// Entries in source order; `None` marks an omitted or base-folded contribution.
        Vec<Option<EmittedEntry>>,
    ),
}

/// One entry of an object literal inside a class expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum EmittedEntry {
    /// An individual custom property awaiting value conversion.
    CustomProperty(
        /// Decoded custom-property name and unconverted source value.
        Property,
    ),
    /// An ordinary property awaiting serialization.
    Attribute(
        /// Decoded property name, source value, and original value span.
        Property,
    ),
    /// A nested class expression.
    Class(
        /// Class-expression subtree to materialize within the object.
        Emitted,
    ),
}

/// One contribution to a record. Variants retain independent runtime channels.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Atom {
    /// A contribution to ordered class concatenation.
    Class(
        /// Class-only subtree or an opaque value requiring runtime interpretation.
        Emitted,
    ),
    /// An ordinary property, including an authored `style` attribute.
    Attribute(
        /// One occurrence; repeated names retain their evaluation order.
        Property,
    ),
    /// A custom property merged independently of ordinary `style` and style payloads.
    Custom(
        /// One occurrence; repeated names are resolved by the consumer's representation.
        Property,
    ),
}

impl Emitted {
    /// Returns whether all evaluated children and guards have effect-free proofs.
    /// Opaque `Source` values remain unproven even if their code has source mappings;
    /// helper calls use the proof supplied by lowering.
    pub(crate) fn is_pure(&self) -> bool {
        match self {
            Self::Leaf { .. } => true,
            Self::Expression(ValueExpr::Call { pure, .. }) => *pure,
            Self::Expression(ValueExpr::Source(_)) => false,
            Self::Array(items) => items.iter().flatten().all(Self::is_pure),
            Self::Conditional {
                pure,
                consequent,
                alternate,
                ..
            } => {
                *pure
                    && consequent
                        .iter()
                        .chain(alternate)
                        .all(|item| item.is_pure())
            }
            Self::Group(entries) => entries.iter().flatten().all(|entry| match entry {
                EmittedEntry::Class(value) => value.is_pure(),
                EmittedEntry::Attribute(p) | EmittedEntry::CustomProperty(p) => p.pure,
            }),
        }
    }

    /// Returns whether the subtree may be removed without losing contributions or effects.
    /// A conditional qualifies only when its test is proven pure and both arms qualify.
    /// Dynamic expressions and property entries are never treated as empty here.
    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Self::Leaf { names } => names.iter().all(String::is_empty),
            Self::Array(items) => items.iter().flatten().all(Self::is_empty),
            Self::Conditional {
                pure,
                consequent,
                alternate,
                ..
            } => {
                *pure
                    && consequent
                        .iter()
                        .chain(alternate)
                        .all(|item| item.is_empty())
            }
            Self::Group(entries) => entries
                .iter()
                .flatten()
                .all(|entry| matches!(entry, EmittedEntry::Class(value) if value.is_empty())),
            Self::Expression(_) => false,
        }
    }
}

impl Atom {
    /// Returns the property proof or recursively checks the class subtree.
    pub(super) fn is_pure(&self) -> bool {
        match self {
            Self::Class(value) => value.is_pure(),
            Self::Attribute(p) | Self::Custom(p) => p.pure,
        }
    }

    /// Returns whether the record-order proof must account for this occurrence.
    /// Only static class leaves have no evaluation boundary; this is independent of
    /// purity, since a pure read must not move across an effectful write.
    pub(super) fn evaluates(&self) -> bool {
        !matches!(self, Self::Class(Emitted::Leaf { .. }))
    }
}

/// A composable value before symbolic fragments become framework props.
/// Sequences and records initially retain contribution order. Folding may group
/// effect-free channels but keeps guards, opaque operations, and occurrence origins.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Fragment {
    /// Contributions eligible for one field-layout proof.
    Record(
        /// Ordered occurrences, including repeated names; empty means no contribution.
        Vec<Atom>,
    ),
    /// Composition of values that need not fit one record.
    Sequence(
        /// Children in evaluation order; nested sequences can be flattened.
        Vec<Self>,
    ),
    /// A guard shared by all channels in the selected arm.
    Branch {
        /// Complete test expression with original occurrence mappings.
        test: Code,
        /// Ternary or logical conjunction; the latter ignores `alternate`.
        kind: ConditionalKind,
        /// Value selected when the test is truthy.
        consequent: Box<Self>,
        /// Value selected when a ternary test is falsy; empty for conjunction.
        alternate: Box<Self>,
    },
    /// A materialized props value requiring runtime conversion to symbolic channels.
    FromProps(
        /// Complete source expression; an opaque folding boundary.
        Code,
    ),
    /// A whole-style payload distinct from an ordinary `style` property.
    StylePayload(
        /// Payload expression retained as an opaque folding boundary.
        ValueExpr,
    ),
}

impl Fragment {
    /// Builds a tree from finalized class units without allocating helper bindings.
    /// Static names precede a stable sort of the dynamic parts by source position;
    /// repeated positions keep their emission order and all retained code keeps its mappings.
    pub(super) fn from_emission(emission: &StyleEmission) -> Self {
        let mut parts = emission.parts.iter().collect::<Vec<_>>();
        parts.sort_by_key(|part| match part {
            StyleEmissionPart::Class { position, .. }
            | StyleEmissionPart::Attributes { position, .. }
            | StyleEmissionPart::CustomProperties { position, .. }
            | StyleEmissionPart::Base { position, .. }
            | StyleEmissionPart::Style { position, .. } => *position,
        });
        let mut children = Vec::new();
        if !emission.static_names.is_empty() {
            children.push(Self::Record(vec![Atom::Class(Emitted::Leaf {
                names: emission.static_names.clone(),
            })]));
        }
        children.extend(parts.into_iter().map(|part| match part {
            StyleEmissionPart::Class { emitted, .. } => Self::from_class(emitted),
            StyleEmissionPart::Attributes { property, .. } => {
                Self::Record(vec![Atom::Attribute(property.clone())])
            }
            StyleEmissionPart::CustomProperties { property, .. } => {
                Self::Record(vec![Atom::Custom(property.clone())])
            }
            StyleEmissionPart::Base { expression, .. } => Self::FromProps(expression.clone()),
            StyleEmissionPart::Style { expression, .. } => Self::StylePayload(expression.clone()),
        }));
        Self::Sequence(children)
    }

    /// Converts mixed-channel groups to fragment structure, omitting provably empty values.
    /// Class-only trees remain one atom so their guard is never duplicated across fields.
    fn from_class(value: &Emitted) -> Self {
        if value.is_empty() {
            return Self::Record(Vec::new());
        }
        // Class-only trees stay intact, so tests are not distributed over channels.
        if is_plain(value) {
            return Self::Record(vec![Atom::Class(value.clone())]);
        }
        match value {
            Emitted::Group(entries) => Self::Sequence(
                entries
                    .iter()
                    .flatten()
                    .map(|entry| match entry {
                        EmittedEntry::Class(value) => Self::from_class(value),
                        EmittedEntry::Attribute(value) => {
                            Self::Record(vec![Atom::Attribute(value.clone())])
                        }
                        EmittedEntry::CustomProperty(value) => {
                            Self::Record(vec![Atom::Custom(value.clone())])
                        }
                    })
                    .collect(),
            ),
            Emitted::Array(items) => {
                Self::Sequence(items.iter().flatten().map(Self::from_class).collect())
            }
            Emitted::Conditional {
                code: test,
                kind,
                consequent,
                alternate,
                ..
            } => Self::Branch {
                test: test.clone(),
                kind: *kind,
                consequent: Box::new(
                    consequent
                        .as_deref()
                        .map_or_else(|| Self::Record(Vec::new()), Self::from_class),
                ),
                alternate: Box::new(
                    alternate
                        .as_deref()
                        .map_or_else(|| Self::Record(Vec::new()), Self::from_class),
                ),
            },
            Emitted::Leaf { .. } | Emitted::Expression(_) => {
                Self::Record(vec![Atom::Class(value.clone())])
            }
        }
    }
}
