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
//! [`DesignSystemOptions`] validates that they differ and provides
//! [`DesignSystemOptions::rewrite_generated`] for substituting the canonical markers.
//! The compiler chooses which generated strings to pass through that operation; loading
//! a design system does not rewrite every authored value to the configured prefixes.
//!
//! Runtime-conditional syntax is shared by value and variant resolution through
//! [`is_runtime_condition`] and [`runtime_if_arguments`]; the compiler chooses its lowering.
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

mod conditionals;
mod evaluation;
mod loading;
mod options;
mod output;
mod registry;
mod snapshot;
mod theme;

pub use conditionals::{is_runtime_condition, runtime_condition_value, runtime_if_arguments};
pub use evaluation::*;
pub use loading::*;
pub use options::*;
pub use output::parse_output_template;
pub use registry::*;
pub use snapshot::*;
pub use theme::*;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod test_support;
