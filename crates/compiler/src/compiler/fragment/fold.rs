//! Representation and evaluation proofs for combining style contributions.
//!
//! [`record_fields`] selects a layout for a known record and consumer demand.
//! [`fold`] reduces adjacent contributions bottom-up when a whole-call layout fails.
//! This module does not render JavaScript or allocate runtime bindings; rejected
//! candidates remain available for runtime composition.

use std::collections::BTreeSet;

use stillwind_ir::{ConditionalKind, FrameworkSpec, StyleFormat};

use super::expression::{Atom, Emitted, Fragment, ValueExpr};

/// Why a candidate cannot represent this ordered record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Rejection {
    /// Grouping a channel would cross an evaluation in a record with unproven effects.
    EvaluationOrder {
        /// Zero-based index of the first grouped occurrence in the candidate's atom slice.
        before: usize,
        /// Index of an evaluated occurrence in another channel, strictly between the pair.
        between: usize,
        /// Index of the later grouped occurrence; `before < between < after`.
        after: usize,
    },
    /// Value shape, key layout, or framework serialization cannot represent the record.
    Representation,
}

/// The representation accepted by the immediate consumer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Demand {
    /// Symbolic channels that a parent fragment can still compose.
    Fragment,
    /// Final framework keys and object/string style format, with runtime key presence/order.
    Props,
}

/// One output field, referencing source-ordered occurrences in a record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Field {
    /// One field containing ordered class contributions.
    Classes(
        /// Nonempty, increasing indices of class atoms in the candidate record.
        Vec<usize>,
    ),
    /// One field containing ordered custom-property contributions.
    Custom(
        /// Nonempty, increasing indices of custom-property atoms in the candidate record.
        Vec<usize>,
    ),
    /// One ordinary property field.
    Attribute(
        /// Zero-based index of its attribute atom in the candidate record.
        usize,
    ),
}

/// Plans output fields for `atoms` under the consumer's representation and framework.
///
/// Returned indices address this exact atom slice and preserve order within each
/// channel. Cross-channel reordering is permitted only when every atom is proven
/// pure; otherwise evaluated occurrences keep source order. Final props also retain
/// runtime field presence and insertion order. The input is not modified.
///
/// # Errors
///
/// Returns an evaluation-order obstruction when grouping crosses another evaluated
/// channel in an unproven record. Other unsupported shapes, special property names,
/// empty-class uncertainty, or incompatible style serialization return `Representation`.
#[allow(clippy::too_many_lines)] // One proof over record fields and their runtime channels.
pub(super) fn record_fields(
    atoms: &[Atom],
    demand: Demand,
    fw: &FrameworkSpec,
) -> Result<Vec<Field>, Rejection> {
    let pure = atoms.iter().all(Atom::is_pure);
    let classes = atoms
        .iter()
        .enumerate()
        .filter_map(|(i, atom)| matches!(atom, Atom::Class(_)).then_some(i))
        .collect::<Vec<_>>();
    let custom = atoms
        .iter()
        .enumerate()
        .filter_map(|(i, atom)| matches!(atom, Atom::Custom(_)).then_some(i))
        .collect::<Vec<_>>();
    let class_at = classes
        .iter()
        .copied()
        .find(|i| atoms[*i].evaluates())
        .or_else(|| classes.first().copied());
    let custom_at = custom.first().copied();
    let mut fields = Vec::new();
    for (i, atom) in atoms.iter().enumerate() {
        if Some(i) == class_at {
            fields.push(Field::Classes(classes.clone()));
        }
        if Some(i) == custom_at {
            fields.push(Field::Custom(custom.clone()));
        }
        if matches!(atom, Atom::Attribute(_)) {
            fields.push(Field::Attribute(i));
        }
    }
    let expected = atoms
        .iter()
        .enumerate()
        .filter_map(|(i, atom)| atom.evaluates().then_some(i))
        .collect::<Vec<_>>();
    let actual = fields
        .iter()
        .flat_map(|field| match field {
            Field::Classes(items) | Field::Custom(items) => items.clone(),
            Field::Attribute(i) => vec![*i],
        })
        .filter(|i| atoms[*i].evaluates())
        .collect::<Vec<_>>();
    if !pure && expected != actual {
        for group in [&classes, &custom] {
            let evaluations = group
                .iter()
                .copied()
                .filter(|i| atoms[*i].evaluates())
                .collect::<Vec<_>>();
            for pair in evaluations.windows(2) {
                if let Some(between) =
                    (pair[0] + 1..pair[1]).find(|i| atoms[*i].evaluates() && !group.contains(i))
                {
                    return Err(Rejection::EvaluationOrder {
                        before: pair[0],
                        between,
                        after: pair[1],
                    });
                }
            }
        }
        return Err(Rejection::Representation);
    }
    if atoms.iter().any(|atom| matches!(atom, Atom::Attribute(p) if p.name == "__proto__" || p.name == fw.class_property)) {
        return Err(Rejection::Representation);
    }
    if atoms
        .iter()
        .any(|atom| matches!(atom, Atom::Class(Emitted::Expression(ValueExpr::Source(_)))))
        && (demand == Demand::Props || classes.len() > 1)
    {
        return Err(Rejection::Representation);
    }
    if demand == Demand::Props {
        // Materialization inserts class before ordinary props and style after
        // them. A literal must preserve both this key order and evaluation order.
        let first_attribute = atoms
            .iter()
            .position(|atom| matches!(atom, Atom::Attribute(_)));
        let last_attribute = atoms
            .iter()
            .rposition(|atom| matches!(atom, Atom::Attribute(_)));
        if (!pure
            && (class_at
                .zip(first_attribute)
                .is_some_and(|(class, attr)| class > attr)
                || custom_at
                    .zip(last_attribute)
                    .is_some_and(|(custom, attr)| custom < attr)))
            || (!classes.is_empty()
                && !classes
                    .iter()
                    .any(|i| matches!(&atoms[*i], Atom::Class(value) if nonempty_class(value))))
        {
            return Err(Rejection::Representation);
        }
        if !custom.is_empty()
            && atoms
                .iter()
                .any(|atom| matches!(atom, Atom::Attribute(p) if p.name == "style"))
        {
            return Err(Rejection::Representation);
        }
        if fw.style_format == StyleFormat::String {
            let mut names = BTreeSet::new();
            if custom
                .iter()
                .any(|i| matches!(&atoms[*i], Atom::Custom(p) if !names.insert(&p.name)))
            {
                return Err(Rejection::Representation);
            }
        }
    }
    if pure && demand == Demand::Props {
        // Match runtime property insertion order independently of pure evaluations.
        fields.sort_by_key(|field| match field {
            Field::Classes(_) => 0,
            Field::Attribute(_) => 1,
            Field::Custom(_) => 2,
        });
    }
    Ok(fields)
}

/// Reduces children before combining adjacent record contributions.
///
/// Flattens nested sequences, drops empty records inside sequences, and groups
/// compatible adjacent atoms. Branch tests, opaque props, and style payloads remain
/// boundaries. Failed combinations stay separate; the result still requires a
/// representation proof when rendered and never materializes framework props here.
pub(super) fn fold(fragment: Fragment, fw: &FrameworkSpec) -> Fragment {
    match fragment {
        Fragment::Sequence(children) => {
            let mut output = Vec::new();
            let mut pending = Vec::new();
            let mut channels = Channels::default();
            for child in children {
                let child = fold(child, fw);
                let children = match child {
                    Fragment::Sequence(children) => children,
                    other => vec![other],
                };
                for child in children {
                    if let Fragment::Record(atoms) = child {
                        for atom in atoms {
                            if !channels.append(&atom, fw) {
                                if !pending.is_empty() {
                                    output.push(Fragment::Record(std::mem::take(&mut pending)));
                                }
                                channels = Channels::default();
                                channels.append(&atom, fw);
                            }
                            pending.push(atom);
                        }
                    } else {
                        if !pending.is_empty() {
                            output.push(Fragment::Record(std::mem::take(&mut pending)));
                        }
                        output.push(child);
                        channels = Channels::default();
                    }
                }
            }
            if !pending.is_empty() {
                output.push(Fragment::Record(pending));
            }
            if output.len() == 1 {
                output.remove(0)
            } else {
                Fragment::Sequence(output)
            }
        }
        Fragment::Branch {
            test,
            kind,
            consequent,
            alternate,
        } => Fragment::Branch {
            test,
            kind,
            consequent: Box::new(fold(*consequent, fw)),
            alternate: Box::new(fold(*alternate, fw)),
        },
        other => other,
    }
}

/// Compatibility state for a pending sequence of record atoms.
/// A failed append closes that pending record; the caller must reset this state
/// before retrying the rejected atom.
#[derive(Default)]
struct Channels {
    /// Whether an evaluated class (index 0) or custom-property (index 1) channel was seen.
    seen: [bool; 2],
    /// Last evaluated channel; `None` initially and after an ordinary attribute.
    last: Option<usize>,
    /// An opaque class or special attribute closes the record to subsequent atoms.
    special: bool,
    /// At least one accepted evaluation lacks an effect-free proof.
    effectful: bool,
    /// Grouping accepted pure channels requires reordering, forbidding later effects.
    reordered: bool,
}

impl Channels {
    /// Tests and records one occurrence against the pending layout.
    /// On `true`, the caller may append the atom. On `false`, discard this state:
    /// rejection is not transactional and may already have set the closing flag.
    fn append(&mut self, atom: &Atom, fw: &FrameworkSpec) -> bool {
        if self.special {
            return false;
        }
        if matches!(atom, Atom::Class(Emitted::Expression(ValueExpr::Source(_)))) {
            self.special = true;
            return !self.seen[0];
        }
        let effectful = self.effectful || !atom.is_pure();
        if effectful && self.reordered {
            return false;
        }
        let channel = match atom {
            Atom::Class(_) if atom.evaluates() => Some(0),
            Atom::Class(_) => return true,
            Atom::Custom(_) => Some(1),
            Atom::Attribute(p) => {
                self.special = p.name == "__proto__" || p.name == fw.class_property;
                None
            }
        };
        if let Some(channel) = channel {
            if self.seen[channel] && self.last != Some(channel) {
                if effectful {
                    return false;
                }
                self.reordered = true;
            }
            self.seen[channel] = true;
        }
        self.effectful = effectful;
        self.last = channel;
        true
    }
}

/// Clones and flattens records/sequences into one ordered root candidate.
/// Returns `None` for any branch, opaque props, or style payload; an empty subtree
/// returns an empty atom list. No representation or effect proof is performed here.
pub(super) fn known_record(fragment: &Fragment) -> Option<Vec<Atom>> {
    match fragment {
        Fragment::Record(atoms) => Some(atoms.clone()),
        Fragment::Sequence(children) => children
            .iter()
            .map(known_record)
            .collect::<Option<Vec<_>>>()
            .map(|records| records.into_iter().flatten().collect()),
        _ => None,
    }
}

/// Returns a static leaf's names joined by one space, without JavaScript quoting.
/// Returns `None` for every other node, including arrays or branches of static leaves.
pub(super) fn static_class(value: &Emitted) -> Option<String> {
    if let Emitted::Leaf { names } = value {
        Some(names.join(" "))
    } else {
        None
    }
}

/// Conservatively proves a class field cannot disappear for any branch selection.
/// Nonempty leaves, arrays containing a proven value, and ternaries with both arms
/// proven qualify; opaque values and logical conjunctions remain unproven.
fn nonempty_class(value: &Emitted) -> bool {
    match value {
        Emitted::Leaf { names } => names.iter().any(|name| !name.is_empty()),
        Emitted::Array(items) => items.iter().flatten().any(nonempty_class),
        Emitted::Conditional {
            kind: ConditionalKind::Ternary,
            consequent,
            alternate,
            ..
        } => {
            consequent.as_deref().is_some_and(nonempty_class)
                && alternate.as_deref().is_some_and(nonempty_class)
        }
        _ => false,
    }
}
