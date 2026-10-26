//! Replaces several utilities of one source container with one utility that produces
//! the same CSS, such as `w-4 h-4` with `size-4`.
//!
//! A group contains the candidates that have the same variants and the same `!`. Only
//! candidates in the same group are replaced together. A proposal is a utility that may
//! replace two or more candidates of a group. A proposal is accepted only when the
//! group produces the same CSS after the replacement.
//!
//! `group` splits the candidates of a container into groups and returns the result of
//! [`crate::DesignSystemQuery::canonical_candidates`]. `partition` divides a group into
//! parts whose candidates cannot affect each other. `component` accepts proposals for
//! one part. `proposals` lists the proposals for a group. `compare` compares the CSS of
//! two sets of candidates.

mod compare;
mod component;
mod group;
mod partition;
mod proposals;
