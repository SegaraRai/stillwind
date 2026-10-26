//! Summarizes the CSS of a completion label for display next to the label.

use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{CssNode, CssNodeKind, parse_structure};

/// Returns a short summary of the generated CSS `css` of one selected completion.
///
/// The summary lists ordinary declarations. It lists custom properties only when there
/// are no ordinary declarations. When the CSS has no declarations, the summary lists
/// its selectors and at-rules instead. The summary has at most 160 characters. A longer
/// summary is cut and ends with an ellipsis. Returns `None` when the CSS has nothing to
/// show.
pub fn completion_detail(css: &str) -> Option<LeanStr> {
    /// Appends the declarations of `nodes` to `ordinary` or `custom`, and their
    /// selectors and at-rules to `wrappers`. `nodes` must refer to byte ranges of
    /// `css`. A declaration that already appears is not added again.
    fn collect(
        css: &str,
        nodes: &[CssNode],
        ordinary: &mut Vec<String>,
        custom: &mut Vec<String>,
        wrappers: &mut Vec<String>,
    ) {
        for node in nodes {
            match &node.kind {
                CssNodeKind::StyleRule { selector, .. } => {
                    let selector = &css[selector.clone()];
                    if selector != "&" {
                        wrappers.push(selector.into());
                    }
                }
                CssNodeKind::AtRule { name, .. } if name == "property" || name == "apply" => {
                    continue;
                }
                CssNodeKind::AtRule { name, prelude, .. } => {
                    wrappers.push(format!("@{name} {}", &css[prelude.clone()]));
                }
                CssNodeKind::Declaration { property, value } => {
                    let property = &css[property.clone()];
                    let output = if property.starts_with("--") {
                        &mut *custom
                    } else {
                        &mut *ordinary
                    };
                    let text = format!("{property}: {};", &css[value.clone()]);
                    if !output.contains(&text) {
                        output.push(text);
                    }
                }
            }
            collect(css, node.children(), ordinary, custom, wrappers);
        }
    }
    let (mut ordinary, mut custom, mut wrappers) = (Vec::new(), Vec::new(), Vec::new());
    collect(
        css,
        &parse_structure(css),
        &mut ordinary,
        &mut custom,
        &mut wrappers,
    );
    let values = if !ordinary.is_empty() {
        ordinary
    } else if !custom.is_empty() {
        custom
    } else {
        wrappers
    };
    if values.is_empty() {
        return None;
    }
    let text = values.join(" ");
    Some(if text.chars().count() > 160 {
        format_args!("{}…", text.chars().take(159).collect::<String>()).to_lean_str()
    } else {
        text.into()
    })
}

#[cfg(test)]
mod tests {
    use super::completion_detail;

    #[test]
    fn completion_details_preserve_declaration_values_and_describe_empty_wrappers() {
        assert_eq!(
            completion_detail("& { --metadata: blue; content: 'a;b'; padding: 1rem; }"),
            Some("content: 'a;b'; padding: 1rem;".into())
        );
        assert_eq!(
            completion_detail("@media (hover: hover) { &:hover {} }"),
            Some("@media (hover: hover) &:hover".into())
        );
        assert_eq!(
            completion_detail(
                "@property --color { syntax: '<color>'; inherits: false; initial-value: red; } & { --paint: red; }"
            ),
            Some("--paint: red;".into())
        );
    }
}
