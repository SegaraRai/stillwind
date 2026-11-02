//! Name allocation: the generator contract, its injection point, and the memo of one
//! compilation.
//!
//! A host supplies [`Identifiers`] to a compilation. Each name family has its own
//! generator method, because each family has different context: a utility class belongs
//! to a call and a branch, a declaration name belongs to a module and a binding, and an
//! anonymous layer belongs to a template and a position. Every method also receives the
//! untruncated hash of that context, so a generator may use the hash alone without
//! reimplementing the identity rule. A method that returns [`GeneratedName::Default`]
//! keeps the compiler's own name.
//!
//! [`IdentifierContext`] is the allocation of one compilation. It asks the host
//! generator first and uses the compiler's own name second, and it memoizes the result
//! per family, identity, and label. A helper that asks twice therefore never observes
//! two calls of an impure generator and never produces two names for one declaration. A
//! rejection is memoized too, so one compilation reports it once.
//!
//! Modules:
//!
//! - [`allocation`] resolves requests, memoizes their names, and records rejected and
//!   detached names.
//! - [`contract`] defines host generators and their compilation inputs.

mod allocation;
mod contract;

pub use allocation::{IdentifierContext, RejectedIdentifier, RejectedNames};
pub use contract::{GeneratedName, GeneratorSet, IdentifierGenerator, Identifiers};

pub(crate) use allocation::DetachedName;
