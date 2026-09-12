//! Stylesheet assembly.
//!
//! [`render`] exposes the two entry points (document and carrier), [`template`] walks the design
//! system's output template to place layers, theme tokens, and utility slots, and
//! [`authored_directives`] expands `@apply` and `@variant` in user CSS. [`properties`] decides which
//! `@property` registrations are needed, [`keyframes`] extracts authored frames, and [`blocks`]
//! provides the byte-level at-rule scanning those passes rely on. All of them operate on CSS text
//! and return CSS text; nothing here mutates the design system.

pub(super) mod authored_directives;
pub(super) mod blocks;
pub(super) mod keyframes;
pub(super) mod properties;
pub(super) mod render;
pub(super) mod template;
