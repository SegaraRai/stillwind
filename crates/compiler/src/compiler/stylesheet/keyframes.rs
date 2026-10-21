//! Extraction of authored keyframes for the emitted theme block.
//!
//! Only the keyframes the used tokens ask for are emitted, and each requested rule is copied
//! verbatim, including its whitespace and nested frames. The name comparison is exact and anchored
//! at `@keyframes`, so a request for `spin` does not select `spin-fast`. A rule whose braces never
//! close contributes nothing rather than a truncated rule that would change the animation.

use lean_string::{LeanStr, LeanString};

use stillwind_css::{CssNodeKind, parse_structure};

/// Copies the named `@keyframes` rules out of `css`, in the order the names are given.
///
/// A name that does not appear is skipped silently: a theme may reference a keyframe declared by
/// the host's own CSS, and that is not an error here.
pub(in crate::compiler) fn extract_authored_keyframes(css: &str, names: &[LeanStr]) -> LeanString {
    let mut output = LeanString::new();
    for node in parse_structure(css) {
        if let CssNodeKind::AtRule {
            name,
            prelude,
            block: Some(block),
        } = node.kind
            && name == "keyframes"
            && block.close.is_some()
            && names
                .iter()
                .any(|name| name.as_str() == &css[prelude.clone()])
        {
            output.push_str(&css[node.range]);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_requested_frames_and_preserves_non_frame_css() {
        let css = "a{}@keyframes spin {from{x:0}to{x:1}}b{}@keyframes fade {to{x:0}}";
        assert_eq!(
            extract_authored_keyframes(css, &["fade".into(), "missing".into()]),
            "@keyframes fade {to{x:0}}"
        );

        assert_eq!(
            extract_authored_keyframes("@keyframes x {", &["x".into()]),
            ""
        );
    }
}
