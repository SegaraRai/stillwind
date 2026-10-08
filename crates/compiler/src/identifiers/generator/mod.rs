//! Name allocation: the generator contract, its injection point, and one compilation's memo.
//!
//! [`Identifiers`] is what a host supplies to a compilation. Each family has its own generator
//! method, because each one is given different context: a utility class belongs to a call and a
//! branch, a declaration name belongs to a module and a binding, and an anonymous layer belongs to
//! a template and a position. Every method is also given the hash of that context, untruncated, so
//! a generator may use the hash alone without reimplementing the identity rule. A method that
//! returns [`GeneratedName::Builtin`] keeps the compiler's own name.
//!
//! [`IdentifierContext`] is one compilation's allocation: the host generator first, the compiler's
//! own name second, memoized per family, identity, and label. A helper asked twice therefore cannot
//! observe two calls of an impure generator and cannot produce two names for one declaration, and
//! a rejection is memoized as a rejection, so one compilation reports it once.
//!
//! `contract` defines host generators and their compilation inputs. `allocation`
//! resolves requests, memoizes their names, and records rejected and detached names.

mod allocation;
mod contract;

pub use allocation::{IdentifierContext, RejectedIdentifier, RejectedNames};
pub use contract::{GeneratedName, GeneratorSet, IdentifierGenerator, Identifiers};

pub(crate) use allocation::DetachedName;
