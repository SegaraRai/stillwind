//! Representation proofs for combining style contributions.
//!
//! [`record_fields`] selects a layout for a known record and consumer demand. [`fold`]
//! reduces adjacent contributions bottom-up when a whole-call layout fails. This module
//! does not render JavaScript or allocate runtime bindings; rejected candidates remain
//! available for runtime composition.

use std::collections::BTreeSet;

use lean_string::LeanString;

use stillwind_ir::{FrameworkSpec, StyleFormat};
use stillwind_source::JoinText;

use super::expression::{Atom, Emitted, Fragment, ValueExpr};

/// The representation accepted by the immediate consumer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Demand {
    /// Symbolic channels that a parent fragment can still compose.
    Fragment,
    /// Final framework keys and object/string style format.
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
}

/// Plans a shared field layout for a record under the requested consumer.
///
/// Inputs must satisfy the macro purity contract and the class-atom invariants of
/// [`Fragment::from_emission`]. Returned indices address this exact slice and preserve
/// contribution order within each channel. Props layouts group class and
/// custom-property fields in that order. Dynamic class fields can remain present when
/// empty.
///
/// Returns `None` for an opaque source class sharing a field with another class. Props
/// demand additionally rejects duplicate custom names in string style format. An empty
/// record yields an empty layout.
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
    for i in 0..atoms.len() {
        if Some(i) == class_at {
            fields.push(Field::Classes(classes.clone()));
        }
        if Some(i) == custom_at {
            fields.push(Field::Custom(custom.clone()));
        }
    }
    if atoms
        .iter()
        .any(|atom| matches!(atom, Atom::Class(Emitted::Expression(ValueExpr::Source(_)))))
        && classes.len() > 1
    {
        return None;
    }
    if demand == Demand::Props && fw.style_format == StyleFormat::String {
        let mut names = BTreeSet::new();
        if custom
            .iter()
            .any(|i| matches!(&atoms[*i], Atom::Custom(p) if !names.insert(&p.name)))
        {
            return None;
        }
    }
    if demand == Demand::Props {
        // Match runtime property insertion order independently of pure evaluations.
        fields.sort_by_key(|field| match field {
            Field::Classes(_) => 0,
            Field::Custom(_) => 1,
        });
    }
    Some(fields)
}

/// Combines adjacent contributions and moves class-only branches into class values.
///
/// Requires the macro purity contract and the class-atom invariants established by
/// [`Fragment::from_emission`]. Preserves contribution order within channels and the
/// conditions selecting retained values. Opaque props and whole-style payloads remain
/// boundaries. The result still needs [`record_fields`] validation for its consumer,
/// because grouping alone does not establish a valid final-props layout.
pub(super) fn fold(fragment: Fragment) -> Fragment {
    match fragment {
        Fragment::Sequence(children) => {
            let mut output = Vec::new();
            let mut pending = Vec::new();
            let mut channels = Channels::default();
            for child in children {
                let child = fold(child);
                let children = match child {
                    Fragment::Sequence(children) => children,
                    other => vec![other],
                };
                for child in children {
                    if let Fragment::Record(atoms) = child {
                        for atom in atoms {
                            if !channels.append(&atom) {
                                if !pending.is_empty() {
                                    output.push(Fragment::Record(std::mem::take(&mut pending)));
                                }
                                channels = Channels::default();
                                channels.append(&atom);
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
        Fragment::Branch { test, branches } => {
            let branches = branches.map(|value| Box::new(fold(*value)));
            let classes = branches.as_ref().try_map(|value| {
                class_branch(value)
                    .map(|class| Some(Box::new(class)))
                    .ok_or(())
            });
            if let Ok(branches) = classes {
                Fragment::Record(vec![Atom::Class(Emitted::Conditional {
                    code: test,
                    branches,
                })])
            } else {
                Fragment::Branch { test, branches }
            }
        }
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
    fn append(&mut self, atom: &Atom) -> bool {
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
pub(super) fn static_class(value: &Emitted) -> Option<LeanString> {
    if let Emitted::Leaf { names } = value {
        Some(names.join_text(" "))
    } else {
        None
    }
}

/// Extracts an ordered class value from a record or sequence of records.
///
/// Requires class atoms with the invariants established by [`Fragment::from_emission`].
/// Returns `None` for other fragment nodes, non-class atoms, or a top-level opaque
/// source class atom. Those opaque values cannot be moved into clsx because fragment
/// collection ignores nonstrings that clsx can interpret. Empty input yields an empty
/// class array. Retained values are cloned without normalization or runtime type
/// checks.
fn class_branch(fragment: &Fragment) -> Option<Emitted> {
    let atoms = known_record(fragment)?;
    let mut values = atoms
        .into_iter()
        .map(|atom| match atom {
            Atom::Class(Emitted::Expression(ValueExpr::Source(_))) | Atom::Custom(_) => None,
            Atom::Class(value) => Some(Some(value)),
        })
        .collect::<Option<Vec<_>>>()?;
    if values.len() == 1 {
        values.pop().flatten()
    } else {
        Some(Emitted::Array(values))
    }
}
