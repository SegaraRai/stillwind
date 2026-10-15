//! Restores authored CSS URLs at resource references in display output.

use cssparser::serialize_string;
use lean_string::{LeanString, ToLeanString};

use stillwind_css::relative_css_urls;
use stillwind_design_system::DesignSystemSnapshot;

/// Restores authored specifiers at CSS URL references that match resources in
/// `snapshot`.
///
/// The replacement uses CSS string escaping and preserves the reference's string or
/// `url()` form. Other text and unknown references remain unchanged. No resources are
/// loaded, and relative specifiers are returned as authored rather than resolved paths.
pub(crate) fn preview_resource_urls(
    mut css: LeanString,
    snapshot: &DesignSystemSnapshot,
) -> LeanString {
    for reference in relative_css_urls(&css).into_iter().rev() {
        let Some(resource) = snapshot
            .sources()
            .resources
            .iter()
            .find(|resource| resource.placeholder == reference.specifier)
        else {
            continue;
        };
        let mut value = LeanString::new();
        let _ = serialize_string(&resource.specifier, &mut value);
        let value = if reference.string {
            value
        } else {
            format_args!("url({value})").to_lean_string()
        };
        css.replace_range(reference.range, &value);
    }
    css
}
