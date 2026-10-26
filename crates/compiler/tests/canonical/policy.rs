//! Tests for options of the `prefer-canonical-utilities` rule whose defaults differ
//! from Tailwind's canonicalization.

use stillwind_compiler::{CompilerOptions, DesignSystemQuery};
use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_diagnostics::{CanonicalUtilitiesOverride, LintOverrides};
use stillwind_source::{FileId, SourceFile};
use stillwind_test_support::expand_css;

#[test]
fn ancestor_migrations_respect_the_specificity_policy() {
    let source =
        SourceFile::new(FileId(1), None, None, expand_css("@import 'stillwind';")).unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &source,
        "theme.css".into(),
        DesignSystemOptions::default(),
    )
    .unwrap();
    for preserve in [true, false, true] {
        let query = DesignSystemQuery::new(
            snapshot.clone(),
            CompilerOptions {
                lint: LintOverrides {
                    prefer_canonical_utilities: Some(CanonicalUtilitiesOverride {
                        preserve_specificity: Some(preserve),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        assert_eq!(
            query.canonical_candidate("[p_&]:flex").unwrap().as_deref(),
            if preserve { None } else { Some("in-[p]:flex") }
        );
        assert_eq!(
            query
                .canonical_candidate("[&:nth-child(odd)]:flex")
                .unwrap()
                .as_deref(),
            Some("odd:flex")
        );
    }
}

#[test]
fn internal_prefixes_keep_the_same_canonical_replacements() {
    let source =
        SourceFile::new(FileId(1), None, None, expand_css("@import 'stillwind';")).unwrap();
    for prefix in ["--tw-", "--compat-"] {
        let options = CompilerOptions {
            tailwind_prefix: prefix.into(),
            ..Default::default()
        };
        let snapshot =
            DesignSystemSnapshot::from_source(&source, "theme.css".into(), options.design_system())
                .unwrap();
        let query = DesignSystemQuery::new(snapshot, options);
        assert_eq!(
            query
                .canonical_candidate("tracking-[0.025em]")
                .unwrap()
                .as_deref(),
            Some("tracking-wide")
        );
        assert_eq!(
            query
                .canonical_candidates(&["text-sm".into(), "leading-7".into()])
                .unwrap(),
            ["text-sm/7"]
        );
    }
}

#[test]
fn arbitrary_compaction_keeps_runtime_variables_and_literal_content() {
    let source =
        SourceFile::new(FileId(1), None, None, expand_css("@import 'stillwind';")).unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &source,
        "theme.css".into(),
        DesignSystemOptions::default(),
    )
    .unwrap();
    let query = DesignSystemQuery::new(snapshot, CompilerOptions::default());
    for raw in [
        "w-[var(--unknown,_10px)]",
        "w-(--my_width,10px)",
        "content-['a_b']",
        "bg-[url('/a_b.png')]",
        r"content-['a\_b']",
    ] {
        let canonical = query
            .canonical_candidate(raw)
            .unwrap()
            .unwrap_or_else(|| raw.into());
        assert!(
            query.hover_candidate(&canonical).unwrap().is_some(),
            "{canonical}"
        );
        assert!(
            query.canonical_candidate(&canonical).unwrap().is_none(),
            "{canonical}"
        );
        if raw.contains("my_width") {
            assert!(canonical.contains("--my_width"));
        }
        if raw.contains("unknown") {
            assert!(canonical.contains("--unknown,10px"));
        }
        if raw.contains("url(") {
            assert!(canonical.contains("/a_b.png"));
        }
    }
}

#[test]
fn stacked_variants_keep_the_subject_of_an_attribute_selector() {
    let source =
        SourceFile::new(FileId(1), None, None, expand_css("@import 'stillwind';")).unwrap();
    let snapshot = DesignSystemSnapshot::from_source(
        &source,
        "theme.css".into(),
        DesignSystemOptions::default(),
    )
    .unwrap();
    let query = DesignSystemQuery::new(snapshot, CompilerOptions::default());
    for raw in [
        "focus:[[data-visible]&]:flex",
        "focus:[[data-visible]]:flex",
        "focus:[&:is([data-visible])]:flex",
    ] {
        assert_eq!(
            query.canonical_candidate(raw).unwrap().as_deref(),
            Some("focus:data-visible:flex"),
            "{raw}"
        );
    }
}
