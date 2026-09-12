//! CSS-defined styling semantics, caller-driven loading, and immutable snapshots.
//!
//! The crate owns everything that can be expressed by the project's CSS: theme tokens,
//! the utility, variant and property registries, import expansion, and candidate
//! evaluation. A host supplies the CSS buffers through [`DesignSystemLoader`] and
//! receives a [`DesignSystemSnapshot`]. The loader never reads a file, resolves a
//! specifier, or interprets a source name as a path; those decisions stay in the host
//! (`cli`, Vite, or an editor).
//!
//! Two prefixed custom-property namespaces exist and must stay distinct:
//! Stillwind-owned identifiers use the configurable `--sw-` prefix, and the
//! Tailwind-compatibility namespace uses the separately configurable `--tw-` prefix.
//! [`DesignSystemOptions`] validates that they differ. Both are substituted into
//! generated text in one pass wherever that text is rewritten, which is the compiler's
//! own entry point rather than anything in this crate.
//!
//! A snapshot is immutable after construction and clones an `Arc`, so a compile or
//! editor request pins one clone for its whole duration instead of observing two
//! revisions. Reloading CSS builds a new snapshot.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

mod evaluation;
mod loading;
mod options;
mod output;
mod registry;
mod snapshot;
mod theme;

pub use evaluation::*;
pub use loading::*;
pub use options::*;
pub use registry::*;
pub use snapshot::*;
pub use theme::*;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "../../test-support/css.rs"]
mod css_fixture;

#[cfg(test)]
#[path = "../../test-support/defaults.rs"]
mod test_defaults;
