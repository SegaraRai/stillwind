//! Semantic style contributions before framework keys and style format are selected.
//!
//! [`Fragment::from_emission`] builds an ordered tree from finalized class units.
//! [`Emitted`] retains class guards and property groups; [`Property`] and [`ValueExpr`]
//! carry source provenance. Folding and field selection belong to [`super::fold`];
//! rendering is deferred so rejected candidates never allocate helper imports.

use stillwind_ir::ConditionalBranches;
use stillwind_source::Span;

use super::super::emission::CLASS_KEY;
use super::super::expression_render::is_plain;
use super::super::props::{StyleEmission, StyleEmissionPart};
use crate::generated::{Code, code, js_string};
use crate::runtime::{RuntimeBindings, RuntimeHelper};

/// A source expression or a deferred compiler-owned value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ValueExpr {
    /// An opaque value whose shape has not been proven.
    Source(
        /// Complete expression code; source-derived text retains its occurrence
        /// mappings.
        Code,
    ),
    /// The marker class read from a compiled ref value.
    ///
    /// A ref is emitted as `{ c: "marker-class" }`, so this read is a class string that
    /// can remain in a class-only expression.
    RefClassRead {
        /// Ref value expression with its source occurrence mappings.
        value: Code,
    },
    /// A compiler-owned call; arguments retain their evaluation order.
    Call {
        /// Runtime operation; its local binding is allocated only during rendering.
        helper: RuntimeHelper,
        /// Complete argument expressions in JavaScript evaluation order, without
        /// separators.
        arguments: Vec<Code>,
        /// Original-source UTF-8 span selecting the helper import realm.
        span: Span,
    },
    /// A table selecting a generated class from a runtime asset value.
    Lookup {
        /// Runtime asset value whose identity selects the class.
        value: Box<Self>,
        /// Property containing the asset identity, without its leading dot.
        key: String,
        /// Table literal mapping candidate identities to generated classes.
        table: Code,
    },
    /// A one-entry style payload whose value a deferred expression produces.
    ///
    /// A view-transition name is written as a style property rather than through a
    /// helper that builds the whole payload, so the entry keeps the deferred call in
    /// one place.
    Entry {
        /// Property name, written as a JavaScript string key.
        property: String,
        /// Property value expression.
        value: Box<Self>,
    },
}

impl ValueExpr {
    /// Produces expression code with retained argument mappings.
    /// A deferred call registers its helper at `span`; an opaque source value is cloned
    /// without registering helpers. Call this only after selecting a renderable plan.
    pub(crate) fn render(&self, runtime: &mut RuntimeBindings<'_>) -> Code {
        match self {
            Self::Source(expression) => expression.clone(),
            Self::RefClassRead { value } => {
                code!["(", value.clone(), ").", CLASS_KEY]
            }
            Self::Call {
                helper,
                arguments,
                span,
                ..
            } => {
                let helper = runtime.require(*helper, *span);
                code![helper, "(", Code::join(arguments.clone(), ","), ")"]
            }
            Self::Lookup { value, key, table } => {
                // The parenthesis keeps the object literal out of statement position and
                // a runtime ref's dot read unambiguous.
                code![
                    "(",
                    table.clone(),
                    ")[(",
                    value.render(runtime),
                    ")",
                    if key.is_empty() {
                        String::new()
                    } else {
                        format!(".{key}")
                    },
                    "]"
                ]
            }
            Self::Entry { property, value } => {
                code!["{", js_string(property), ":", value.render(runtime), "}"]
            }
        }
    }
}

impl From<String> for ValueExpr {
    /// Wraps text as an opaque value with no exact source mappings.
    fn from(value: String) -> Self {
        Self::Source(value.into())
    }
}

impl From<&str> for ValueExpr {
    /// Copies text into an opaque value with no exact source mappings.
    fn from(value: &str) -> Self {
        Self::Source(value.into())
    }
}

/// A source-owned property whose value has not been converted or serialized.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Property {
    /// Preconverted CSS text for literal custom-property values.
    pub static_value: Option<String>,
    /// Compiler-generated expression already producing CSS text. Used when
    /// `static_value` is absent, before falling back to source-value conversion.
    pub computed_value: Option<ValueExpr>,
    /// Decoded property name, without JavaScript quoting.
    pub name: String,
    /// Verbatim UTF-8 expression text equal to the original source slice at `span`.
    pub value: String,
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
        /// Contributions selected by the source operator; `None` contributes no value.
        branches: ConditionalBranches<Option<Box<Emitted>>>,
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
    /// A whole-style value selected under the same source branch as its class.
    StylePayload(ValueExpr),
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
    /// Returns whether the subtree contributes no value under the macro purity
    /// contract. Conditions whose arms are both empty may be discarded. Dynamic
    /// expressions and property entries are never treated as empty here.
    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Self::Leaf { names } => names.iter().all(String::is_empty),
            Self::Array(items) => items.iter().flatten().all(Self::is_empty),
            Self::Conditional { branches, .. } => branches
                .consequent()
                .iter()
                .chain(branches.alternate().into_iter().flatten())
                .all(|item| item.is_empty()),
            Self::Group(entries) => entries
                .iter()
                .flatten()
                .all(|entry| matches!(entry, EmittedEntry::Class(value) if value.is_empty())),
            Self::Expression(_) => false,
        }
    }
}

/// A composable value before symbolic fragments become framework props. Sequences and
/// records initially retain contribution order. Folding may group channels under the
/// macro contract but keeps guards, opaque operations, and occurrence origins.
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
        /// Values selected by the source operator.
        branches: ConditionalBranches<Box<Self>>,
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
    /// repeated positions keep their emission order and all retained code keeps its
    /// mappings.
    pub(super) fn from_emission(emission: &StyleEmission) -> Self {
        let mut children = Vec::new();
        if !emission.static_names.is_empty() {
            children.push(Self::Record(vec![Atom::Class(Emitted::Leaf {
                names: emission.static_names.clone(),
            })]));
        }
        children.push(Self::from_parts(&emission.parts));
        Self::Sequence(children)
    }

    /// Returns a sequence of branch-local contributions sorted by original source
    /// position without moving them across their selecting test.
    fn from_parts(parts: &[StyleEmissionPart]) -> Self {
        let mut parts = parts.iter().collect::<Vec<_>>();
        parts.sort_by_key(|part| match part {
            StyleEmissionPart::Conditional { position, .. }
            | StyleEmissionPart::Class { position, .. }
            | StyleEmissionPart::Attributes { position, .. }
            | StyleEmissionPart::CustomProperties { position, .. }
            | StyleEmissionPart::Base { position, .. } => *position,
        });
        Self::Sequence(
            parts
                .into_iter()
                .map(|part| match part {
                    StyleEmissionPart::Conditional { test, branches, .. } => Self::Branch {
                        test: test.clone(),
                        branches: branches
                            .as_ref()
                            .map(|parts| Box::new(Self::from_parts(parts))),
                    },
                    StyleEmissionPart::Class { emitted, .. } => Self::from_class(emitted),
                    StyleEmissionPart::Attributes { property, .. } => {
                        Self::Record(vec![Atom::Attribute(property.clone())])
                    }
                    StyleEmissionPart::CustomProperties { property, .. } => {
                        Self::Record(vec![Atom::Custom(property.clone())])
                    }
                    StyleEmissionPart::Base { expression, .. } => {
                        Self::FromProps(expression.clone())
                    }
                })
                .collect(),
        )
    }

    /// Returns the fragment structure of a class contribution, using an empty record
    /// for a provably empty value. Class-only trees remain one atom so their guard is
    /// never duplicated across fields.
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
                        EmittedEntry::StylePayload(value) => Self::StylePayload(value.clone()),
                    })
                    .collect(),
            ),
            Emitted::Array(items) => {
                Self::Sequence(items.iter().flatten().map(Self::from_class).collect())
            }
            Emitted::Conditional {
                code: test,
                branches,
                ..
            } => Self::Branch {
                test: test.clone(),
                branches: branches.as_ref().map(|value| {
                    Box::new(
                        value
                            .as_deref()
                            .map_or_else(|| Self::Record(Vec::new()), Self::from_class),
                    )
                }),
            },
            Emitted::Leaf { .. } | Emitted::Expression(_) => {
                Self::Record(vec![Atom::Class(value.clone())])
            }
        }
    }
}
