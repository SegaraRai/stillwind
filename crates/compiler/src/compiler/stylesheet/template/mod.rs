//! Assembly of the final stylesheet from the design system's output template.
//!
//! [`render_mapped`] combines a design-system template with one producer's rules,
//! generated keyframes, and registration dependencies. It selects the required theme
//! declarations and registrations, then inserts them at the template's slots. The
//! template supplies selectors, layers, and conditions; this module does not choose
//! browser targets or minify values.
//!
//! Document output includes authored rules. Carrier output includes generated support
//! and layer reservations, but omits independent authored rules. Both use the same
//! anonymous-layer identities. [`render`] is the raw-CSS entry point without input
//! metadata; generated artifacts use [`render_mapped`] to preserve their placement,
//! dependencies, and source mappings.
//!
//! [`assemble`] coordinates selection, slot validation, and emission. [`support`]
//! selects and renders generated dependencies; [`emit`] walks the template and
//! preserves placement and source origins. [`authored`] resolves authored CSS for
//! both dependency discovery and emission. The test module checks assembly contracts
//! across selection and placement, including carrier behavior and source mappings.

mod assemble;
mod authored;
mod emit;
mod support;

pub(super) use assemble::render;
pub(crate) use assemble::render_mapped;

#[cfg(test)]
mod tests;
