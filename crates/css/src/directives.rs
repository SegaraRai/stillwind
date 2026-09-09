use super::blocks::{strip_blocks, strip_top_level_blocks};
use super::theme::imports::strip_framework_imports;

pub(super) fn strip_framework_directives(css: &str) -> String {
    let css = strip_framework_imports(css);
    let css = strip_blocks(&css, "@theme");
    let css = strip_blocks(&css, "@utility");
    let css = strip_top_level_blocks(&css, "@variant");
    let css = strip_blocks(&css, "@custom-variant");
    let css = strip_blocks(&css, "@stillwind-property");
    let css = strip_blocks(&css, "@stillwind");
    css.lines()
        .filter(|line| {
            let line = line.trim_start();
            !line.starts_with("@custom-variant") && !line.starts_with("@tailwind")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
