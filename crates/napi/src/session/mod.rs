//! Stateful Node host binding. Resolution and I/O remain in JavaScript.
//!
//! [`NativeDesignSystemLoader`] exposes the loader state machine directly: it returns
//! the outstanding CSS requests and accepts the host's answers, so the JavaScript side
//! keeps full control of resolution, package lookup, and reading order. The snapshot
//! only exists after a `Complete` step, and every method that needs it reports
//! [`NativeSessionError::IncompleteCssLoad`] instead of loading anything itself.
//!
//! [`project::NativeProjectSession`] wraps the project session. It receives TypeScript facts the
//! host already produced through its long-lived TS7 service rather than running a
//! checker of its own.

//!
//! `loader` owns CSS load requests and completed snapshots. `project` wraps project
//! publication and reloads. `facts` attaches supplied checker facts to source scans.

mod facts;
mod loader;
mod project;

use facts::ProvidedTypeScriptFacts;
use loader::NativeDesignSystemLoader;
pub use project::NativeSessionError;
