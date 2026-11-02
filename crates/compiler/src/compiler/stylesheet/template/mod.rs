//! Assembly of the final stylesheet from the output template of the design system.
//!
//! [`render_mapped`] combines a design-system template with the rules, generated
//! keyframes, and registration dependencies of one producer. It selects the required
//! theme declarations and registrations and inserts them at the slots of the template.
//! The template supplies selectors, layers, and conditions; this module does not choose
//! browser targets or minify values.
//!
//! Document output includes authored rules. Carrier output includes generated support
//! and layer reservations but omits independent authored rules. Both use the same
//! anonymous-layer identities. [`render`] is the raw-CSS entry point without input
//! metadata. Generated artifacts use [`render_mapped`] to keep their placement,
//! dependencies, and source mappings.
//!
//! Modules:
//!
//! - [`assemble`] coordinates selection, slot validation, and emission.
//! - [`authored`] resolves authored CSS for both dependency discovery and emission.
//! - [`emit`] walks the template and keeps placement and source origins.
//! - [`support`] selects and renders generated dependencies.

mod assemble;
mod authored;
mod emit;
mod support;

pub(super) use assemble::render;
pub(crate) use assemble::{render_css_template, render_mapped};

#[cfg(test)]
mod tests;
