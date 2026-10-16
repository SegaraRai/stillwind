//! Source variants keep ref identity until they are lowered to CSS applications.
//!
//! A [`StyleVariantPlan`] walks the wrappers around one rule contribution. Ordinary
//! prefixes around utility literals are collected into a string and handed to the
//! candidate resolver, which is what makes a utility prefixed by `hover:` behave
//! exactly like the same candidate written as one token. A ref wrapper cannot be
//! spelled as a string: the selector depends on which element declares the ref and
//! where it sits relative to the target, so the wrapper is lowered into
//! [`VariantApplication`] alternatives that carry the generated markers and shared
//! scope conditions.
//!
//! [`StyleVariantPlan::canonical`] is the identity spelling used for class hashing. It
//! records the ref's generated identifiers and the authored prefix, independent of how
//! the source spelled them. This is one part of class identity; module, callsite, and
//! branch information also participate, so equivalent wrappers alone do not guarantee
//! equal class names.
//!
//! [`plan`] builds a [`StyleVariantPlan`] and applies it to compiled utilities,
//! [`prefix`] parses and resolves ordinary variant prefixes, and [`reference`] lowers
//! ref conditions into selector conditions on the declaring element.

mod plan;
mod prefix;
mod reference;

pub(in crate::compiler) use plan::StyleVariantPlan;
pub(in crate::compiler) use prefix::parse_prefix;
