//! Recursive class contributions and their conversion to CSS rule contributions.

use lean_string::LeanStr;
use serde::{Deserialize, Serialize};

use stillwind_source::{Span, Spanned};

use super::{
    AnchorScope, AssetReference, RefDefinition, RefProjection, StyleVariant, ViewTransitionName,
};

/// Branches selected by a source conditional operator or a lookup in a literal table.
/// The source form determines which branches exist throughout parsing, lowering, and
/// rendering. Deserialization rejects an `alternate` on `And` and requires it on
/// `Ternary`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ConditionalBranches<T> {
    /// `test && consequent`.
    And {
        /// Values selected when the test is truthy.
        consequent: T,
    },
    /// `test ? consequent : alternate`.
    Ternary {
        /// Values selected when the test is truthy.
        consequent: T,
        /// Values selected when the test is falsy.
        alternate: T,
    },
    /// `table[test]` for an array or object literal. The test value selects the entry
    /// whose key equals the value converted to a property name. A value that names no
    /// entry selects no values.
    Select {
        /// Entries in source order with unique keys, possibly empty.
        entries: Vec<SelectEntry<T>>,
    },
}

/// One entry of a literal table that a lookup test selects.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SelectEntry<T> {
    /// Property name of the entry. An array element uses its decimal index. The
    /// JavaScript frontend never produces `__proto__`.
    pub key: LeanStr,
    /// Values selected when the lookup reads this entry.
    pub value: T,
}

/// The test outcome that selects one branch of [`ConditionalBranches`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchArm<'a> {
    /// The test is truthy.
    Truthy,
    /// The test is falsy.
    Falsy,
    /// The lookup reads the table entry with `key`.
    Entry {
        /// Property name of the selected entry.
        key: &'a LeanStr,
        /// Position of the entry in source order, smaller than `count`.
        index: usize,
        /// Number of entries in the table.
        count: usize,
    },
}

impl<T> ConditionalBranches<T> {
    /// Returns each branch with the outcome that selects it, in source order: the
    /// truthy branch, then any falsy branch, or the table entries.
    pub fn arms(&self) -> Vec<(BranchArm<'_>, &T)> {
        match self {
            Self::And { consequent } => vec![(BranchArm::Truthy, consequent)],
            Self::Ternary {
                consequent,
                alternate,
            } => vec![
                (BranchArm::Truthy, consequent),
                (BranchArm::Falsy, alternate),
            ],
            Self::Select { entries } => entries
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    (
                        BranchArm::Entry {
                            key: &entry.key,
                            index,
                            count: entries.len(),
                        },
                        &entry.value,
                    )
                })
                .collect(),
        }
    }

    /// Returns every branch in the order of [`Self::arms`].
    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.arms().into_iter().map(|(_, value)| value)
    }

    /// Returns every branch for mutation in the order of [`Self::arms`].
    pub fn values_mut(&mut self) -> Vec<&mut T> {
        match self {
            Self::And { consequent } => vec![consequent],
            Self::Ternary {
                consequent,
                alternate,
            } => vec![consequent, alternate],
            Self::Select { entries } => entries.iter_mut().map(|entry| &mut entry.value).collect(),
        }
    }

    /// Returns branches of `U` by mapping each branch in the order of [`Self::arms`],
    /// without changing the source form or the table keys.
    pub fn map<U>(self, mut f: impl FnMut(T) -> U) -> ConditionalBranches<U> {
        self.map_arms(|_, value| f(value))
    }

    /// Returns branches of `U` by mapping each branch with the outcome that selects it,
    /// in the order of [`Self::arms`]. The source form and the table keys stay the
    /// same.
    pub fn map_arms<U>(self, mut f: impl FnMut(BranchArm<'_>, T) -> U) -> ConditionalBranches<U> {
        match self.try_map_arms(|arm, value| Ok::<_, std::convert::Infallible>(f(arm, value))) {
            Ok(branches) => branches,
            Err(never) => match never {},
        }
    }

    /// Returns branches of `U` by mapping each branch in the order of [`Self::arms`],
    /// without changing the source form or the table keys.
    ///
    /// # Errors
    ///
    /// Returns the first error produced by `f`, in branch order.
    pub fn try_map<U, E>(
        self,
        mut f: impl FnMut(T) -> Result<U, E>,
    ) -> Result<ConditionalBranches<U>, E> {
        self.try_map_arms(|_, value| f(value))
    }

    /// Returns branches of `U` by mapping each branch with the outcome that selects it,
    /// in the order of [`Self::arms`].
    ///
    /// # Errors
    ///
    /// Returns the first error produced by `f`, in branch order.
    pub fn try_map_arms<U, E>(
        self,
        mut f: impl FnMut(BranchArm<'_>, T) -> Result<U, E>,
    ) -> Result<ConditionalBranches<U>, E> {
        Ok(match self {
            Self::And { consequent } => ConditionalBranches::And {
                consequent: f(BranchArm::Truthy, consequent)?,
            },
            Self::Ternary {
                consequent,
                alternate,
            } => ConditionalBranches::Ternary {
                consequent: f(BranchArm::Truthy, consequent)?,
                alternate: f(BranchArm::Falsy, alternate)?,
            },
            Self::Select { entries } => {
                let count = entries.len();
                ConditionalBranches::Select {
                    entries: entries
                        .into_iter()
                        .enumerate()
                        .map(|(index, SelectEntry { key, value })| {
                            let value = f(
                                BranchArm::Entry {
                                    key: &key,
                                    index,
                                    count,
                                },
                                value,
                            )?;
                            Ok(SelectEntry { key, value })
                        })
                        .collect::<Result<_, E>>()?,
                }
            }
        })
    }

    /// Returns the same source form and branches as shared references. Table keys are
    /// cloned.
    pub fn as_ref(&self) -> ConditionalBranches<&T> {
        match self {
            Self::And { consequent } => ConditionalBranches::And { consequent },
            Self::Ternary {
                consequent,
                alternate,
            } => ConditionalBranches::Ternary {
                consequent,
                alternate,
            },
            Self::Select { entries } => ConditionalBranches::Select {
                entries: entries
                    .iter()
                    .map(|entry| SelectEntry {
                        key: entry.key.clone(),
                        value: &entry.value,
                    })
                    .collect(),
            },
        }
    }
}

/// A root expression that preserves mixed class and props contributions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ClassExpression {
    /// `[a, b]`: elements are flattened at runtime in evaluation order.
    Array(
        /// Source elements in evaluation order, possibly empty.
        Vec<Self>,
    ),
    /// `test ? a : b`, `test && a`, or `table[test]`; an empty branch contributes
    /// nothing.
    Conditional {
        /// The guard expression or lookup key, emitted verbatim.
        test: Span,
        /// Contributions selected by the source form, in source order.
        branches: ConditionalBranches<Vec<Self>>,
    },
    /// CSS rules, including their variant wrappers and rule-only nested branches.
    Rule(
        /// Rule subtree permitted beneath a CSS variant.
        Box<RuleExpression>,
    ),
    /// An object literal containing class and custom-property entries.
    Group {
        /// Span of the whole object literal, used for diagnostics.
        span: Span,
        /// Source order is preserved; custom properties and classes may interleave.
        entries: Vec<GroupEntry>,
    },
    /// The `class` key's value, normalized into clsx arguments under the macro
    /// contract.
    ClassValue {
        /// Whole original value, retained as a contribution-order and helper-import
        /// anchor.
        span: Span,
        /// Original expression occurrences in class order. Array literals without
        /// spread are expanded and holes omitted; spread arrays and guards remain
        /// complete expressions. An empty list contributes no class value.
        arguments: Vec<Span>,
    },
}

/// The reason a class expression cannot become a CSS rule expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuleExpressionErrorKind {
    /// An object entry supplies an inline custom property.
    #[error("a custom property cannot be placed under a CSS variant")]
    CustomProperty,
    /// A class value has no CSS rule for the variant to transform.
    #[error("an external class cannot be placed under a CSS variant")]
    ExternalClass,
}

/// A failed class-to-rule conversion with the first incompatible descendant's kind and span.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{reason}")]
pub struct RuleExpressionError {
    /// Why this descendant cannot be used as a CSS rule.
    pub reason: RuleExpressionErrorKind,
    /// Original source range of the incompatible descendant.
    pub span: Span,
}

/// A CSS rule expression whose every descendant may occur under a variant.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RuleExpression {
    /// A utility string or utility template literal.
    Utilities(
        /// Decoded utility text and its original source span.
        Spanned<LeanStr>,
    ),
    /// A source array of rule expressions.
    Array(
        /// Rule elements in source order, possibly empty.
        Vec<Self>,
    ),
    /// A source condition or table lookup selecting rule expressions.
    Conditional {
        /// Original condition expression or lookup key.
        test: Span,
        /// Rules selected by the source form.
        branches: ConditionalBranches<Vec<Self>>,
    },
    /// A CSS variant applied to rule expressions.
    Variant {
        /// Variant selector or at-rule.
        variant: StyleVariant,
        /// Rules under the variant.
        inputs: Vec<Self>,
    },
    /// An object expression containing only rule entries.
    Group {
        /// Whole object span.
        span: Span,
        /// Rule entries in source order.
        inputs: Vec<Self>,
    },
    /// A scope-only use with no receiver attachment.
    AnchorScope {
        /// Scope supplied independently of a receiver or marker attachment.
        scope: AnchorScope,
        /// Whole use expression for diagnostics and source mapping.
        span: Span,
    },
    /// A use with one known receiver identity.
    RefProjection {
        /// The ref this projection applies to.
        definition: RefDefinition,
        /// The receiver expression the use site wrote, in the original source.
        ///
        /// Retained for source locations; static uses need no runtime marker read.
        value: Span,
        /// The CSS role the referenced element plays for this call.
        projection: RefProjection,
    },
    /// A ref projection whose ref is only known at runtime.
    DynamicRefProjection {
        /// The runtime ref value.
        value: Span,
        /// Every statically reachable alternative, used for CSS planning. An empty
        /// finite set produces no CSS and selects from an empty lookup table.
        refs: Vec<RefDefinition>,
        /// The CSS role the projected element plays in each alternative.
        projection: RefProjection,
    },
    /// A view-transition use resolved to one declared transition.
    ViewTransitionUse {
        /// The declared transition selected by the use.
        transition: AssetReference,
        /// Participant naming supplied by this use.
        name: ViewTransitionName,
    },
    /// A view-transition use selected from finite runtime candidates.
    DynamicViewTransitionUse {
        /// The runtime transition receiver.
        value: Span,
        /// Every statically reachable transition declaration. The JavaScript frontend
        /// omits this use when the finite set is empty.
        transitions: Vec<AssetReference>,
        /// Participant naming supplied by this use.
        name: ViewTransitionName,
    },
    /// `style(keyframes("options"))` with a statically known keyframes.
    KeyframesUse {
        /// The declared keyframes being used.
        keyframes: AssetReference,
        /// Literal animation options and their source span, resolved into keyframes
        /// declarations by the compiler.
        options: Spanned<LeanStr>,
    },
    /// A keyframes use whose declaration is only known at runtime.
    DynamicKeyframesUse {
        /// The receiver expression the use site wrote, whose keyframes name selects the
        /// prebuilt class.
        value: Span,
        /// Every statically reachable alternative, used for CSS planning. The
        /// JavaScript frontend omits this use when the finite set is empty.
        keyframes: Vec<AssetReference>,
        /// Literal animation options and their source span, resolved into keyframes
        /// declarations by the compiler.
        options: Spanned<LeanStr>,
    },
}

impl TryFrom<ClassExpression> for RuleExpression {
    type Error = RuleExpressionError;

    /// Returns the rule subtree of a root expression when every descendant can
    /// contribute CSS under a variant.
    ///
    /// # Errors
    ///
    /// Returns the reason and source range of the first incompatible descendant.
    fn try_from(expression: ClassExpression) -> Result<Self, Self::Error> {
        match expression {
            ClassExpression::Rule(rule) => Ok(*rule),
            ClassExpression::Array(elements) => Ok(Self::Array(
                elements
                    .into_iter()
                    .map(Self::try_from)
                    .collect::<Result<_, _>>()?,
            )),
            ClassExpression::Conditional { test, branches } => Ok(Self::Conditional {
                test,
                branches: branches
                    .try_map(|values| values.into_iter().map(Self::try_from).collect())?,
            }),
            ClassExpression::Group { span, entries } => Ok(Self::Group {
                span,
                inputs: entries
                    .into_iter()
                    .map(|entry| match entry {
                        GroupEntry::Class(expression) => Self::try_from(*expression),
                        GroupEntry::CustomProperty(property) => Err(RuleExpressionError {
                            reason: RuleExpressionErrorKind::CustomProperty,
                            span: property.name.span,
                        }),
                    })
                    .collect::<Result<_, _>>()?,
            }),
            ClassExpression::ClassValue { span, .. } => Err(RuleExpressionError {
                reason: RuleExpressionErrorKind::ExternalClass,
                span,
            }),
        }
    }
}

impl From<RuleExpression> for ClassExpression {
    /// Wraps a rule subtree as one root class contribution.
    fn from(rule: RuleExpression) -> Self {
        Self::Rule(Box::new(rule))
    }
}

/// One source-ordered entry of an object literal class expression.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum GroupEntry {
    /// An entry contributing classes. Boxed to keep the enum small.
    Class(
        /// Class expression supplied by this object entry.
        Box<ClassExpression>,
    ),
    /// A custom CSS property evaluated at runtime and emitted into inline style.
    CustomProperty(
        /// Authored custom-property key and source value.
        CustomPropertyEntry,
    ),
}

/// An inline CSS custom property with an expression evaluated at the call site.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomPropertyEntry {
    /// Decoded CSS property name, beginning with `--`. The JavaScript frontend
    /// selects this entry only after checking the prefix.
    pub name: Spanned<LeanStr>,
    /// Original expression whose value is converted to CSS text.
    pub value: Span,
    /// Decoded CSS text when a literal can be converted without evaluating user
    /// code. When present, rendering uses this value instead of evaluating `value`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub static_value: Option<LeanStr>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialization_requires_the_branches_of_the_source_operator() {
        assert!(
            serde_json::from_str::<ConditionalBranches<Vec<u8>>>(
                r#"{ "kind": "and", "consequent": [1], "alternate": [2] }"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<ConditionalBranches<Vec<u8>>>(
                r#"{ "kind": "ternary", "consequent": [1] }"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<ConditionalBranches<Vec<u8>>>(
                r#"{ "kind": "and", "consequent": [1] }"#
            )
            .is_ok()
        );
    }
}
