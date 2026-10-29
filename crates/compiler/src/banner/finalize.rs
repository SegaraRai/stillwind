//! Selects the notices of a rendered stylesheet and adds them before its body.

use std::collections::BTreeSet;

use lean_string::LeanStr;

use super::tailwind::TAILWIND_BANNER;
use crate::MappedStylesheet;

const STILLWIND: &str = concat!(
    "/*! stillwind v",
    env!("CARGO_PKG_VERSION"),
    " | MIT License | https://stillwind.dev */"
);

/// Returns the notices to add before `stylesheet.css`.
///
/// The list starts with the Stillwind and Tailwind notices. Each other notice that a
/// nonempty range of `stylesheet.banners` requires follows once, in lexical order.
/// The list is empty when the body contains only whitespace. Notices keep their
/// original text.
pub fn stylesheet_banners(stylesheet: &MappedStylesheet) -> Vec<LeanStr> {
    if stylesheet.css.trim().is_empty() {
        return Vec::new();
    }
    let mut selected: BTreeSet<_> = stylesheet
        .banners
        .iter()
        .filter(|(range, _)| !range.is_empty())
        .map(|(_, banner)| banner.as_str())
        .collect();
    selected.remove(STILLWIND);
    selected.remove(TAILWIND_BANNER);
    [STILLWIND, TAILWIND_BANNER]
        .into_iter()
        .chain(selected)
        .map(LeanStr::from)
        .collect()
}

/// Adds the notices from [`stylesheet_banners`] before the body, one notice per line.
///
/// The generated ranges in `mappings` and `segments` move by the byte length of the
/// added text, so the notices have no source mapping. The returned `banners` is empty.
/// Pending `keyframes` and the other fields remain unchanged.
pub fn finalize_stylesheet(mut stylesheet: MappedStylesheet) -> MappedStylesheet {
    let banners = stylesheet_banners(&stylesheet);
    let mut css = String::new();
    for banner in banners {
        css.push_str(&banner);
        css.push('\n');
    }
    let offset = css.len();
    css.push_str(&stylesheet.css);
    stylesheet.css = css;
    stylesheet.banners.clear();
    for mapping in &mut stylesheet.mappings {
        mapping.generated.start += offset;
        mapping.generated.end += offset;
    }
    for (range, _) in &mut stylesheet.segments {
        range.start += offset;
        range.end += offset;
    }
    stylesheet
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stillwind_css::CssBanner;
    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot, resolve_utility};
    use stillwind_source::{CssSourceName, FileId};
    use stillwind_test_support::css_source;

    use super::*;
    use crate::{CompilerOptions, Identifiers, StylesheetPlacement, StylesheetRenderer};

    #[test]
    fn notices_follow_selected_assets_applications_and_inline_values() {
        let source = css_source(
            FileId(1),
            "\
            @stillwind-banner { /*! global */ }\n\
            @theme asset { @stillwind-banner { /*! used */ } --asset-a: url(a.svg); }\n\
            @theme asset { @stillwind-banner { /*! unused */ } --asset-b: url(b.svg); }\n\
            @theme inline { @stillwind-banner { /*! inline */ } --color-brand: red; }\n\
            @utility icon { @stillwind-banner { /*! utility */ } background:var(--asset-a); }\n\
            @utility color-* { color:--value(--color-*); }\n\
            :root { @stillwind-slot theme; }\n\
            .applied { @apply icon; }\n\
            @stillwind-slot utilities;",
        )
        .unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &source,
            CssSourceName::from("test.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        let output = StylesheetRenderer::new(
            &snapshot,
            &CompilerOptions::default(),
            &Identifiers::default(),
        )
        .render_mapped_stylesheet(&MappedStylesheet::default(), StylesheetPlacement::Document)
        .unwrap()
        .stylesheet;
        let comments = stylesheet_banners(&output);
        assert!(comments.iter().any(|text| text == "/*! used */"));
        assert!(comments.iter().any(|text| text == "/*! utility */"));
        assert!(comments.iter().any(|text| text == "/*! global */"));
        assert!(!comments.iter().any(|text| text == "/*! unused */"));
        let resolved = resolve_utility("color-brand", snapshot.theme())
            .unwrap()
            .unwrap();
        assert!(
            resolved
                .banners
                .iter()
                .any(|banner| banner.as_str() == "/*! inline */")
        );
        let published = finalize_stylesheet(output);
        assert!(published.css.starts_with(STILLWIND));
        assert!(!published.css.contains("@stillwind-banner"));
        assert_eq!(published.css.matches("/*! used */").count(), 1);
    }

    #[test]
    fn empty_output_has_no_notices_and_prefixes_leave_sources_unmapped() {
        assert_eq!(finalize_stylesheet(MappedStylesheet::default()).css, "");
        let source = Arc::new(css_source(FileId(1), ".日本{color:red}").unwrap());
        let mut output = MappedStylesheet::default();
        output.text(source.text(), Some((source.clone(), source.full_span())));
        let comment = CssBanner::new("/*! 作者\n notice */").unwrap();
        output.banners.push((0..output.css.len(), comment));
        let output = finalize_stylesheet(output);
        assert_eq!(
            &output.css[output.mappings[0].generated.clone()],
            source.text()
        );
        assert!(output.mappings[0].generated.start > STILLWIND.len());
    }
}
