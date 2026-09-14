//! Registry audits over the vendored CSS, parity fixtures, and Stillwind fixtures.
//!
//! The regular test checks that negative-value definitions state their sign explicitly.
//! The ignored, opt-in audit checks built-in reachability from parity fixtures; normal
//! test runs do not enforce that exhaustive coverage requirement.

use super::*;
use crate::test_defaults::BUILTIN_REGISTRY_CSS;

#[test]
fn builtin_negative_value_definitions_author_their_sign_explicitly() {
    for (file, css) in REGISTRY_FILES {
        assert!(
            !css.contains("--inset()"),
            "{file} must express keyword domains in CSS instead of a Rust inset helper"
        );
        for utility in crate::parse_builtin_utility_definitions(css) {
            if utility.name.starts_with('-')
                && (utility.body.contains("--value(") || utility.body.contains("--value-map("))
            {
                assert!(
                    utility.body.contains("* -1"),
                    "negative utility {} in {file} must author its sign in CSS: {}",
                    utility.name,
                    utility.body
                );
            }
        }
    }
}

#[test]
#[ignore = "manual exhaustive built-in registry reachability audit"]
#[allow(clippy::too_many_lines)]
fn audits_builtin_registry_reachability_from_parity_fixtures() {
    assert_eq!(
        REGISTRY_FILES
            .iter()
            .map(|(_, css)| *css)
            .collect::<String>(),
        BUILTIN_REGISTRY_CSS,
        "audit registry must follow the production registry exactly"
    );
    let utilities = registry_utilities();
    let variants = registry_variants();
    let mut matched_utilities = BTreeSet::new();
    let mut resolved_utilities = BTreeSet::new();
    let mut used_utilities = BTreeSet::new();
    let mut matched_variants = BTreeSet::new();
    let mut resolved_variants = BTreeSet::new();
    let mut used_variants = BTreeSet::new();
    let fixture_root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/tailwind-v4/cases");
    let mut fixture_paths = fs::read_dir(fixture_root)
        .expect("parity fixture directory")
        .map(|entry| entry.expect("fixture entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .collect::<Vec<_>>();
    fixture_paths.sort();
    for project in ["runtime-if-fallback", "runtime-if-native"] {
        fixture_paths.push(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../fixtures/projects/{project}/stillwind.css")),
        );
    }

    let mut candidate_count = 0_usize;
    let mut skipped_scope_candidates = 0_usize;
    let mut theme_parse_failures = 0_usize;
    let mut candidate_parse_failures = 0_usize;
    for path in fixture_paths {
        let input = fs::read_to_string(&path).expect("registry fixture");
        let applications = stillwind_css::extract_named_statements(&input, "@apply");
        // Project @apply inputs also produce checked-in compiler output through
        // fixtures:check, so reachability witnesses exercise real transformations.
        let cases = if path.extension().is_some_and(|extension| extension == "css") {
            vec![(
                input.as_str(),
                applications
                    .iter()
                    .flat_map(|(_, candidates)| candidates.split_whitespace())
                    .collect(),
            )]
        } else {
            parity_cases(&input)
        };
        for (case_index, (input, candidates)) in cases.into_iter().enumerate() {
            // `prefix(...)` import modifiers are explicitly unsupported, and the scope
            // directives select sources outside a resolved stylesheet.
            if ["@source", "@config", "@plugin", "@reference", "prefix("]
                .iter()
                .any(|directive| input.contains(directive))
            {
                skipped_scope_candidates += candidates.len();
                continue;
            }
            let source =
                SourceFile::new(FileId(1), format!("{}#{case_index}", path.display()), input)
                    .unwrap();
            let Ok(theme) = parse_fixture_theme(&source) else {
                theme_parse_failures += 1;
                continue;
            };
            for raw in candidates {
                let Some(candidate) = parse_candidate(raw) else {
                    candidate_parse_failures += 1;
                    continue;
                };
                candidate_count += 1;
                for variant in &candidate.variants {
                    if let VariantKind::Named(value) = &variant.kind {
                        for (index, (_, definition)) in variants.iter().enumerate() {
                            let (matched, resolved) =
                                resolve_registry_variant(definition, value, &theme);
                            if matched {
                                matched_variants.insert(index);
                            }
                            if resolved.is_some() {
                                resolved_variants.insert(index);
                            }
                        }
                        let baseline =
                            resolve_registry_variant_branches(&variants, None, value, &theme);
                        for excluded in 0..variants.len() {
                            let without = resolve_registry_variant_branches(
                                &variants,
                                Some(excluded),
                                value,
                                &theme,
                            );
                            if without != baseline {
                                used_variants.insert(excluded);
                            }
                        }
                    }
                }
                let Utility::Named { name, modifier } = &candidate.utility else {
                    continue;
                };
                let lookup = if candidate.negative {
                    format!("-{name}")
                } else {
                    name.clone()
                };
                let matched = matched_registry_utilities(&utilities, &lookup);
                matched_utilities.extend(matched.iter().map(|(index, _)| *index));
                resolved_utilities.extend(matched.iter().filter_map(|(index, definition)| {
                    resolve_css_utility_body(definition, &lookup, modifier.as_ref(), &theme)
                        .map(|_| *index)
                }));
                let baseline = resolve_css_utility_branches(
                    matched.iter().map(|(_, definition)| *definition),
                    &lookup,
                    modifier.as_ref(),
                    &theme,
                );
                for (excluded, _) in &matched {
                    let without = resolve_css_utility_branches(
                        matched
                            .iter()
                            .filter(|(index, _)| index != excluded)
                            .map(|(_, definition)| *definition),
                        &lookup,
                        modifier.as_ref(),
                        &theme,
                    );
                    if without != baseline {
                        used_utilities.insert(*excluded);
                    }
                }
            }
        }
    }

    let never_matched_utilities = utilities
        .iter()
        .enumerate()
        .filter(|(index, _)| !matched_utilities.contains(index))
        .map(|(_, (label, _))| label.as_str())
        .collect::<Vec<_>>();
    let unresolved_utilities = utilities
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            matched_utilities.contains(index) && !resolved_utilities.contains(index)
        })
        .map(|(_, (label, _))| label.as_str())
        .collect::<Vec<_>>();
    let non_contributing_utilities = utilities
        .iter()
        .enumerate()
        .filter(|(index, _)| resolved_utilities.contains(index) && !used_utilities.contains(index))
        .map(|(_, (label, _))| label.as_str())
        .collect::<Vec<_>>();
    let never_matched_variants = variants
        .iter()
        .enumerate()
        .filter(|(index, _)| !matched_variants.contains(index))
        .map(|(_, (label, _))| label.as_str())
        .collect::<Vec<_>>();
    let unresolved_variants = variants
        .iter()
        .enumerate()
        .filter(|(index, _)| matched_variants.contains(index) && !resolved_variants.contains(index))
        .map(|(_, (label, _))| label.as_str())
        .collect::<Vec<_>>();
    let shadowed_variants = variants
        .iter()
        .enumerate()
        .filter(|(index, _)| resolved_variants.contains(index) && !used_variants.contains(index))
        .map(|(_, (label, _))| label.as_str())
        .collect::<Vec<_>>();
    let mut duplicate_utilities = HashMap::<(&str, &str), Vec<&str>>::new();
    for (label, utility) in &utilities {
        duplicate_utilities
            .entry((&utility.name, &utility.body))
            .or_default()
            .push(label);
    }
    let mut duplicate_utility_groups = duplicate_utilities
        .into_values()
        .filter(|labels| labels.len() > 1)
        .map(|labels| labels.join(" = "))
        .collect::<Vec<_>>();
    duplicate_utility_groups.sort();
    eprintln!(
        "registry audit: {candidate_count} candidates, {skipped_scope_candidates} out-of-scope candidates, {theme_parse_failures} theme parse failures, {candidate_parse_failures} candidate parse failures, {}/{} utility definitions contribute, {}/{} variant definitions contribute",
        used_utilities.len(),
        utilities.len(),
        used_variants.len(),
        variants.len()
    );
    eprintln!(
        "utility definitions never matched ({}):\n{}",
        never_matched_utilities.len(),
        never_matched_utilities.join("\n")
    );
    eprintln!(
        "utility definitions matched but never resolved ({}):\n{}",
        unresolved_utilities.len(),
        unresolved_utilities.join("\n")
    );
    eprintln!(
        "utility definitions resolved but never contributed ({}):\n{}",
        non_contributing_utilities.len(),
        non_contributing_utilities.join("\n")
    );
    eprintln!(
        "exact duplicate utility groups ({}):\n{}",
        duplicate_utility_groups.len(),
        duplicate_utility_groups.join("\n")
    );
    eprintln!(
        "variant definitions never matched ({}):\n{}",
        never_matched_variants.len(),
        never_matched_variants.join("\n")
    );
    eprintln!(
        "variant definitions matched but never resolved ({}):\n{}",
        unresolved_variants.len(),
        unresolved_variants.join("\n")
    );
    eprintln!(
        "variant definitions resolved but never contributed ({}):\n{}",
        shadowed_variants.len(),
        shadowed_variants.join("\n")
    );
    assert!(
        never_matched_utilities.is_empty(),
        "every utility definition must have fixture coverage"
    );
    assert!(
        unresolved_utilities.is_empty(),
        "every matched utility definition must resolve in a fixture"
    );
    assert!(
        duplicate_utility_groups.is_empty(),
        "exact duplicate utility definitions must be removed"
    );
    assert!(
        never_matched_variants.is_empty()
            && unresolved_variants.is_empty()
            && shadowed_variants.is_empty(),
        "every variant definition must contribute in a fixture"
    );
}
