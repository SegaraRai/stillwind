//! Source variants keep their ref identity until they are lowered to CSS applications.
//!
//! A [`StyleVariantPlan`] walks the wrappers around one rule contribution. Ordinary
//! prefixes around utility literals are collected into a string and passed to the
//! candidate resolver, so a utility prefixed by `hover:` behaves exactly like the same
//! candidate written as one token. A ref wrapper cannot be spelled as a string, because
//! its selector depends on which element declares the ref and where that element is
//! relative to the target. The wrapper is therefore lowered into
//! [`VariantApplication`](stillwind_design_system::VariantApplication) alternatives
//! that carry the generated markers and shared scope conditions.
//!
//! [`StyleVariantPlan::canonical`] is the identity spelling used for class hashing. It
//! records the generated identifiers of the ref and the authored prefix, independent of
//! how the source spelled them. It is only one part of class identity: module, call
//! site, and branch information also participate, so equivalent wrappers alone do not
//! guarantee equal class names.
//!
//! Modules:
//!
//! - [`plan`] builds a [`StyleVariantPlan`] and applies it to compiled utilities.
//! - [`prefix`] parses and resolves ordinary variant prefixes.
//! - [`mod@reference`] lowers ref conditions into selector conditions on the declaring
//!   element.

mod plan;
mod prefix;
mod reference;

pub(in crate::compiler) use plan::StyleVariantPlan;
pub(in crate::compiler) use prefix::{collect_variant_and_applications, parse_prefix};
