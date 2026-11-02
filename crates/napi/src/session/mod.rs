//! Stateful Node host binding. Resolution and I/O stay in JavaScript.
//!
//! [`NativeDesignSystemLoader`] exposes the loader state machine directly. It returns
//! the outstanding CSS requests and accepts the answers of the host, so JavaScript
//! keeps full control of resolution, package lookup, and reading order. The snapshot
//! exists only after a `Complete` step, and every method that needs it reports
//! [`NativeSessionError::IncompleteCssLoad`] instead of loading anything itself.
//!
//! [`NativeProjectSession`](project::NativeProjectSession) wraps the project session.
//! It receives the TypeScript facts that the long-lived TS7 service of the host has
//! produced, and runs no checker of its own.
//!
//! Modules:
//!
//! - [`facts`] attaches supplied checker facts to source scans.
//! - [`loader`] owns CSS load requests and completed snapshots.
//! - [`project`] wraps project publication and reloads.

mod facts;
mod loader;
mod project;

pub use project::NativeSessionError;

use facts::ProvidedTypeScriptFacts;
use loader::NativeDesignSystemLoader;
