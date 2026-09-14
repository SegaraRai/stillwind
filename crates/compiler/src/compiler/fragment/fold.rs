//! Representation proofs for combining style contributions.
//!
//! [`record_fields`] selects a layout for a known record and consumer demand. [`fold`]
//! reduces adjacent contributions bottom-up when a whole-call layout fails. This module
//! does not render JavaScript or allocate runtime bindings; rejected candidates remain
//! available for runtime composition.

use std::collections::BTreeSet;

use stillwind_ir::{ConditionalKind, FrameworkSpec, StyleFormat};

use super::expression::{Atom, Emitted, Fragment, ValueExpr};

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
/// channel. Inputs must satisfy the macro purity contract. Final props retain runtime
/// field presence and insertion order. Returns `None` for unsupported shapes, special
/// property names, empty-class uncertainty, or incompatible style serialization.
#[allow(clippy::too_many_lines)] // One proof over record fields and their runtime channels.
pub(super) fn record_fields(
    atoms: &[Atom],
    demand: Demand,
    fw: &FrameworkSpec,
) -> Option<Vec<Field>> {
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
    let class_at = classes.first().copied();
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
    if atoms.iter().any(|atom| matches!(atom, Atom::Attribute(p) if p.name == "__proto__" || p.name == fw.class_property)) {
        return None;
    }
    if atoms
        .iter()
        .any(|atom| matches!(atom, Atom::Class(Emitted::Expression(ValueExpr::Source(_)))))
        && (demand == Demand::Props || classes.len() > 1)
    {
        return None;
    }
    if demand == Demand::Props {
        if !classes.is_empty()
            && !classes
                .iter()
                .any(|i| matches!(&atoms[*i], Atom::Class(value) if nonempty_class(value)))
        {
            return None;
        }
        if !custom.is_empty()
            && atoms
                .iter()
                .any(|atom| matches!(atom, Atom::Attribute(p) if p.name == "style"))
        {
            return None;
        }
        if fw.style_format == StyleFormat::String {
            let mut names = BTreeSet::new();
            if custom
                .iter()
                .any(|i| matches!(&atoms[*i], Atom::Custom(p) if !names.insert(&p.name)))
            {
                return None;
            }
        }
    }
    if demand == Demand::Props {
        // Match runtime property insertion order independently of pure evaluations.
        fields.sort_by_key(|field| match field {
            Field::Classes(_) => 0,
            Field::Attribute(_) => 1,
            Field::Custom(_) => 2,
        });
    }
    Some(fields)
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

/// Representation state for a pending record. Opaque classes cannot share a class field.
#[derive(Default)]
struct Channels {
    has_class: bool,
    closed: bool,
}

impl Channels {
    /// A rejected atom must be retried with fresh state before appending it.
    fn append(&mut self, atom: &Atom, fw: &FrameworkSpec) -> bool {
        if self.closed {
            return false;
        }
        match atom {
            Atom::Class(Emitted::Expression(ValueExpr::Source(_))) => {
                self.closed = true;
                !self.has_class
            }
            Atom::Class(_) => {
                self.has_class = true;
                true
            }
            Atom::Attribute(p) => {
                self.closed = p.name == "__proto__" || p.name == fw.class_property;
                true
            }
            Atom::Custom(_) => true,
        }
    }
}

/// Clones and flattens records/sequences into one ordered root candidate. Returns
/// `None` for any branch, opaque props, or style payload; an empty subtree returns an
/// empty atom list. No representation proof is performed here.
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
