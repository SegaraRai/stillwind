//! Definition recovery for editor queries when an authored stylesheet is incomplete.

use lean_string::LeanString;

use stillwind_css::{CssNode, CssNodeKind, find_css_functions, parse_structure};

/// Extracts imports and design-system definitions for an editor-only CSS load.
///
/// Authored rules and output slots are omitted. Missing block terminators are restored
/// from the structure tree, and incomplete statements are skipped. The result is a
/// best-effort definition source. Callers must keep ordinary compilation and lint on
/// the original buffers and may still encounter invalid definition headers or imports.
pub fn editor_css_source(css: &str) -> LeanString {
    let mut output = LeanString::new();
    for node in parse_structure(css) {
        collect(css, &node, &mut output);
    }
    output
}

/// Appends imports and definitions from `node` to `output`, flattening `@layer`
/// wrappers. The node must have ranges into the original `css`. Authored rules, output
/// slots, and templates contribute no text.
fn collect(css: &str, node: &CssNode, output: &mut LeanString) {
    let CssNodeKind::AtRule {
        name,
        prelude,
        block,
    } = &node.kind
    else {
        return;
    };
    if matches!(name.as_str(), "stillwind-slot" | "stillwind-css-template") {
        return;
    }
    if name == "layer" {
        for child in node.children() {
            collect(css, child, output);
        }
        return;
    }
    if !(name.starts_with("stillwind-")
        || matches!(
            name.as_str(),
            "theme" | "utility" | "custom-variant" | "variant" | "import"
        ))
    {
        return;
    }
    if let Some(block) = block {
        output.push('@');
        output.push_str(name);
        output.push(' ');
        output.push_str(&css[prelude.clone()]);
        output.push('{');
        for child in &block.children {
            render(css, child, output);
        }
        output.push('}');
    } else if css[node.range.clone()].ends_with(';') {
        output.push_str(&css[node.range.clone()]);
    }
}

/// Appends a node inside a retained definition to `output`, restoring its block
/// terminator. The node must have ranges into the original `css`. Declarations with
/// unclosed theme or custom-property references are omitted. At-rule statements without
/// a semicolon are also omitted.
fn render(css: &str, node: &CssNode, output: &mut LeanString) {
    match &node.kind {
        CssNodeKind::StyleRule { selector, block } => {
            output.push_str(&css[selector.clone()]);
            output.push('{');
            for child in &block.children {
                render(css, child, output);
            }
            output.push('}');
        }
        CssNodeKind::AtRule {
            name,
            prelude,
            block: Some(block),
        } => {
            output.push('@');
            output.push_str(name);
            output.push(' ');
            output.push_str(&css[prelude.clone()]);
            output.push('{');
            for child in &block.children {
                render(css, child, output);
            }
            output.push('}');
        }
        CssNodeKind::Declaration { property, value } => {
            let value = &css[value.clone()];
            if find_css_functions(value, &["theme", "--theme", "var"], 0)
                .iter()
                .any(|call| call.end.is_none())
            {
                return;
            }
            output.push_str(&css[property.clone()]);
            output.push(':');
            output.push_str(value);
            output.push(';');
        }
        CssNodeKind::AtRule { .. } => {
            if css[node.range.clone()].ends_with(';') {
                output.push_str(&css[node.range.clone()]);
            }
        }
    }
}
