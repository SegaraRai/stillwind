//! Composition-order lints over published artifacts.
//!
//! [`composition_lints`] detects competing declarations among unrelated bases and
//! across imported bases. It compares Webref property interference within matching
//! selector and at-rule contexts. Authored document CSS is outside this analysis.

use std::collections::{BTreeMap, BTreeSet};

use stillwind_compiler::CompiledModule;
use stillwind_css::{declaration_ranges, properties_interfere};
use stillwind_diagnostics::{LintFinding, LintKind};
use stillwind_semantic::{IdentityId, ModuleId};
use stillwind_source::{FileId, Span};

/// One declaration a call can contribute, with the variant context it sits under.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DeclarationKey {
    /// Lowercased CSS property name, including custom properties and shorthand forms.
    property: String,
    /// Variant chain the declaration sits under, empty for the unconditional unit.
    context: String,
}

/// Declarations of every call in one artifact, keyed by the call's source offsets.
type CallDeclarations = BTreeMap<(u32, u32), BTreeSet<DeclarationKey>>;

/// Identities that make up one composition graph node.
type CallIdentity = (String, u32, u32);

/// Reports both composition-order rules for one published module.
///
/// Returns no findings when the module has no artifact, composes nothing, or when the
/// other side of an edge has no published artifact yet.
pub(crate) fn composition_lints(
    artifacts: &BTreeMap<ModuleId, CompiledModule>,
    module: &ModuleId,
) -> Vec<LintFinding> {
    let Some(artifact) = artifacts.get(module) else {
        return Vec::new();
    };
    let own = call_declarations(artifact);
    let mut cache: BTreeMap<String, CallDeclarations> = BTreeMap::new();
    let mut bases_by_call: BTreeMap<(u32, u32), BTreeSet<IdentityId>> = BTreeMap::new();
    for edge in &artifact.composition {
        if edge.derived.module_path != module.as_str() {
            continue;
        }
        bases_by_call
            .entry((edge.derived.start, edge.derived.end))
            .or_default()
            .insert(edge.base.clone());
    }
    let graph = composition_graph(artifacts);
    let mut findings = Vec::new();
    for ((start, end), bases) in &bases_by_call {
        // A call with no rules of its own still owns an identity, so the span comes from
        // the identity rather than from a generated class.
        let span = Span::new(FileId(artifact.key.source_file_id), *start, *end);
        let mut unrelated: Vec<(&IdentityId, BTreeSet<DeclarationKey>)> = Vec::new();
        let mut imported: Vec<(&IdentityId, BTreeSet<DeclarationKey>)> = Vec::new();
        for base in bases {
            let Some(declarations) = declarations_of(&mut cache, artifacts, &base.module_path)
            else {
                continue;
            };
            let keys = declarations
                .get(&(base.start, base.end))
                .cloned()
                .unwrap_or_default();
            if keys.is_empty() {
                continue;
            }
            if base.module_path == module.as_str() {
                unrelated.push((base, keys));
            } else {
                imported.push((base, keys));
            }
        }
        let own_keys = own.get(&(*start, *end)).cloned().unwrap_or_default();
        findings.extend(ambiguous_base_order(&graph, &unrelated, span));
        findings.extend(imported_base_interference(
            &own_keys, &unrelated, &imported, module, span,
        ));
    }
    findings.sort_by(|left, right| {
        (left.labels[0].span.start.0, left.code(), left.notes.clone()).cmp(&(
            right.labels[0].span.start.0,
            right.code(),
            right.notes.clone(),
        ))
    });
    findings
}

/// Reports the first declaration slot two unrelated bases compete for.
fn ambiguous_base_order(
    graph: &BTreeMap<CallIdentity, BTreeSet<CallIdentity>>,
    bases: &[(&IdentityId, BTreeSet<DeclarationKey>)],
    span: Span,
) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    for (index, (left, left_keys)) in bases.iter().enumerate() {
        for (right, right_keys) in &bases[index + 1..] {
            let left_key = identity_key(left);
            let right_key = identity_key(right);
            if left_key == right_key
                || reaches(graph, &left_key, &right_key)
                || reaches(graph, &right_key, &left_key)
            {
                continue;
            }
            let Some((left_declaration, right_declaration)) =
                first_interference(left_keys, right_keys)
            else {
                continue;
            };
            let mut finding = LintFinding::new(
                LintKind::AmbiguousBaseOrder {
                    properties: describe(&left_declaration, &right_declaration),
                },
                span,
            );
            finding.notes.push(format!(
                "`{}` and `{}` are independent bases{}",
                left.module_path,
                right.module_path,
                context_note(&left_declaration)
            ));
            findings.push(finding);
            break;
        }
    }
    findings
}

/// Reports the first declaration slot an imported base competes for with the call's rules.
fn imported_base_interference(
    own: &BTreeSet<DeclarationKey>,
    same_module: &[(&IdentityId, BTreeSet<DeclarationKey>)],
    imported: &[(&IdentityId, BTreeSet<DeclarationKey>)],
    module: &ModuleId,
    span: Span,
) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    for (base, keys) in imported {
        let mut competing = own.clone();
        for (other, other_keys) in same_module {
            if other.module_path == base.module_path
                && other.start == base.start
                && other.end == base.end
            {
                continue;
            }
            competing.extend(other_keys.iter().cloned());
        }
        for (other, other_keys) in imported {
            if std::ptr::eq(*other, *base) {
                continue;
            }
            competing.extend(other_keys.iter().cloned());
        }
        let Some((base_declaration, competing_declaration)) = first_interference(keys, &competing)
        else {
            continue;
        };
        if base.module_path == module.as_str() {
            continue;
        }
        let mut finding = LintFinding::new(
            LintKind::ImportedBaseInterference {
                module: base.module_path.clone(),
                properties: describe(&base_declaration, &competing_declaration),
            },
            span,
        );
        finding.notes.push(format!(
            "the overlap has matching selector and at-rule contexts{}",
            context_note(&base_declaration)
        ));
        findings.push(finding);
        break;
    }
    findings
}

fn context_note(key: &DeclarationKey) -> String {
    if key.context.is_empty() {
        String::new()
    } else {
        format!(" under `{}`", key.context)
    }
}

/// The first pair of declarations from two calls that competes for one slot.
///
/// Sets iterate in sorted order, so the reported pair is deterministic. Properties interfere
/// when they are equal or when one is a shorthand of the other, which is the relation the
/// `css` crate resolves from Webref's longhand data.
fn first_interference(
    left: &BTreeSet<DeclarationKey>,
    right: &BTreeSet<DeclarationKey>,
) -> Option<(DeclarationKey, DeclarationKey)> {
    for left_key in left {
        for right_key in right {
            if left_key.context == right_key.context
                && properties_interfere(&left_key.property, &right_key.property)
            {
                return Some((left_key.clone(), right_key.clone()));
            }
        }
    }
    None
}

/// Renders one competing pair for a message: the property alone when both sides name it,
/// otherwise both names.
fn describe(left: &DeclarationKey, right: &DeclarationKey) -> String {
    if left.property == right.property {
        left.property.clone()
    } else {
        format!("{} and {}", left.property, right.property)
    }
}

fn identity_key(identity: &IdentityId) -> CallIdentity {
    (identity.module_path.clone(), identity.start, identity.end)
}

/// Every composition edge of every published artifact.
fn composition_graph(
    artifacts: &BTreeMap<ModuleId, CompiledModule>,
) -> BTreeMap<CallIdentity, BTreeSet<CallIdentity>> {
    let mut graph: BTreeMap<CallIdentity, BTreeSet<CallIdentity>> = BTreeMap::new();
    for artifact in artifacts.values() {
        for edge in &artifact.composition {
            let derived = identity_key(&edge.derived);
            graph
                .entry(derived)
                .or_default()
                .insert(identity_key(&edge.base));
            graph.entry(identity_key(&edge.base)).or_default();
        }
    }
    graph
}

/// Whether `from` reaches `to` through composition edges.
fn reaches(
    graph: &BTreeMap<CallIdentity, BTreeSet<CallIdentity>>,
    from: &CallIdentity,
    to: &CallIdentity,
) -> bool {
    let mut pending = vec![from.clone()];
    let mut seen = BTreeSet::new();
    while let Some(node) = pending.pop() {
        if !seen.insert(node.clone()) {
            continue;
        }
        for next in graph.get(&node).into_iter().flatten() {
            if next == to {
                return true;
            }
            pending.push(next.clone());
        }
    }
    false
}

fn declarations_of<'a>(
    cache: &'a mut BTreeMap<String, CallDeclarations>,
    artifacts: &BTreeMap<ModuleId, CompiledModule>,
    module_path: &str,
) -> Option<&'a CallDeclarations> {
    if !cache.contains_key(module_path) {
        let artifact = artifacts.get(&ModuleId::from(module_path))?;
        cache.insert(module_path.to_owned(), call_declarations(artifact));
    }
    cache.get(module_path)
}

/// Declarations each call of one artifact contributes, per call span.
fn call_declarations(artifact: &CompiledModule) -> CallDeclarations {
    let mut result = CallDeclarations::new();
    for class in &artifact.classes {
        let selector = format!(".{}", class.class_name);
        let mut keys = BTreeSet::new();
        for rule in &artifact.css.rules {
            let Some(body) = rule.syntax.body.as_deref() else {
                continue;
            };
            if !mentions(&rule.syntax.prelude, &selector) && !mentions(body, &selector) {
                continue;
            }
            let mut context = selector_context(&rule.syntax.prelude, &selector);
            if context == "&" {
                context.clear();
            }
            collect_keys(body, &context, &selector, &mut keys);
        }
        if !keys.is_empty() {
            result.insert((class.call_span.start.0, class.call_span.end.0), keys);
        }
    }
    result
}

/// Whether one selector occurrence of `selector` appears in `text`.
fn mentions(text: &str, selector: &str) -> bool {
    let mut from = 0;
    while let Some(index) = text[from..].find(selector) {
        let start = from + index;
        let end = start + selector.len();
        let next = text[end..].chars().next();
        if !next.is_some_and(|next| next.is_ascii_alphanumeric() || next == '-' || next == '_') {
            return true;
        }
        from = end;
    }
    false
}

/// One emitted selector or at-rule header with the call's class represented as `&`.
fn selector_context(prelude: &str, selector: &str) -> String {
    if prelude.trim_start().starts_with('@') {
        return prelude.trim().to_owned();
    }

    let mut output = String::with_capacity(prelude.len());
    let mut remaining = prelude;
    while let Some(index) = remaining.find(selector) {
        let end = index + selector.len();
        let next = remaining[end..].chars().next();
        output.push_str(&remaining[..index]);
        if next.is_some_and(|next| next.is_ascii_alphanumeric() || next == '-' || next == '_') {
            output.push_str(selector);
        } else {
            output.push('&');
        }
        remaining = &remaining[end..];
    }
    output.push_str(remaining);
    output.trim().to_owned()
}

/// Collects declaration keys from one rule body, tracking nested contexts.
fn collect_keys(
    fragment: &str,
    context: &str,
    selector: &str,
    keys: &mut BTreeSet<DeclarationKey>,
) {
    for range in declaration_ranges(fragment) {
        let text = fragment[range].trim();
        if text.is_empty() {
            continue;
        }
        if let Some(open) = text.find('{') {
            let prelude = text[..open].trim();
            let body = text[open + 1..].trim_end().trim_end_matches('}');
            let nested_context = selector_context(prelude, selector);
            let nested = if context.is_empty() {
                nested_context
            } else {
                format!("{context} {nested_context}")
            };
            collect_keys(body, &nested, selector, keys);
            continue;
        }
        let Some((name, _)) = text.split_once(':') else {
            continue;
        };
        let property = name.trim().to_ascii_lowercase();
        if property.is_empty() {
            continue;
        }
        keys.insert(DeclarationKey {
            property,
            context: context.to_owned(),
        });
    }
}

#[cfg(test)]
mod tests {
    use stillwind_compiler::{
        CompilationDependencies, GeneratedClass, GeneratedRule, ModuleArtifactKey,
        ModuleCssArtifact, RuleKey,
    };
    use stillwind_css::CssRule;
    use stillwind_ir::StyleDependency;
    use stillwind_semantic::AssetKind;
    use stillwind_source::{FileId, Fingerprint};

    use super::*;

    /// One artifact with the given calls and composition edges.
    ///
    /// Each call is `(start, end, class name, rule body)`; each edge is
    /// `(derived module, derived index, base module, base index)`.
    fn artifact(
        module: &str,
        calls: &[(u32, u32, &str, &str)],
        edges: &[(&str, usize, &str, usize)],
    ) -> CompiledModule {
        let classes = calls
            .iter()
            .map(|(start, end, class, _)| GeneratedClass {
                class_name: (*class).to_owned(),
                call_span: Span::new(FileId(1), *start, *end),
                guard_path: Vec::new(),
            })
            .collect();
        let rules = calls
            .iter()
            .map(|(_, _, class, body)| GeneratedRule {
                keyframes: false,
                required_properties: Vec::new(),
                key: RuleKey(Fingerprint::of(body)),
                syntax: CssRule {
                    range: 0..0,
                    prelude: format!(".{class}"),
                    body: Some((*body).to_owned()),
                },
            })
            .collect();
        let composition = edges
            .iter()
            .map(
                |(derived_module, derived, base_module, base)| StyleDependency {
                    derived: identity(derived_module, calls[*derived]),
                    base: identity(base_module, calls[*base]),
                },
            )
            .collect();
        CompiledModule {
            composition,
            resources: Vec::new(),
            key: ModuleArtifactKey {
                compiler_version: "test".to_owned(),
                compiler_options: Fingerprint::of("options"),
                analysis_revision: Fingerprint::of("analysis"),
                design_system: Fingerprint::of("design"),
                source_revision: Fingerprint::of(module),
                module_id: ModuleId::from(module),
                source_file_id: 1,
                frontend_kind: "test".to_owned(),
                adapter_mode: "class".to_owned(),
                semantic_revision: None,
            },
            code_transforms: Vec::new(),
            css: ModuleCssArtifact { rules },
            classes,
            symbols: Vec::new(),
            rejected_names: Vec::new(),
            diagnostics: Vec::new(),
            lint_findings: Vec::new(),
            dependencies: CompilationDependencies::default(),
        }
    }

    fn identity(module: &str, call: (u32, u32, &str, &str)) -> IdentityId {
        IdentityId::new(AssetKind::Style, module, call.0, call.1, "")
    }

    fn artifacts(modules: &[(&str, CompiledModule)]) -> BTreeMap<ModuleId, CompiledModule> {
        modules
            .iter()
            .map(|(module, artifact)| (ModuleId::from(*module), artifact.clone()))
            .collect()
    }

    fn codes(findings: &[LintFinding]) -> String {
        findings
            .iter()
            .map(|finding| format!("{} {:?}", finding.code(), finding.kind))
            .collect::<Vec<_>>()
            .join("; ")
    }

    #[test]
    fn imported_base_declarations_are_compared() {
        let app = artifact(
            "app.tsx",
            &[(0, 10, "sw_d", "padding: 16px;")],
            &[("app.tsx", 0, "base.tsx", 0)],
        );
        let base = artifact("base.tsx", &[(0, 10, "sw_b", "padding: 40px;")], &[]);
        let published = artifacts(&[("app.tsx", app), ("base.tsx", base)]);
        let findings = composition_lints(&published, &ModuleId::from("app.tsx"));
        assert_eq!(findings.len(), 1, "{}", codes(&findings));
        assert_eq!(findings[0].code(), "imported-base-interference");
    }

    #[test]
    fn independent_bases_with_one_property_are_reported() {
        let app = artifact(
            "app.tsx",
            &[
                (0, 10, "sw_a", "padding: 40px;"),
                (20, 30, "sw_b", "padding: 16px;"),
                (40, 50, "sw_c", "color: red;"),
            ],
            &[("app.tsx", 2, "app.tsx", 0), ("app.tsx", 2, "app.tsx", 1)],
        );
        let published = artifacts(&[("app.tsx", app)]);
        let findings = composition_lints(&published, &ModuleId::from("app.tsx"));
        assert_eq!(findings.len(), 1, "{}", codes(&findings));
        assert_eq!(findings[0].code(), "ambiguous-base-order");
        assert_eq!(findings[0].labels[0].span.start.0, 40);
    }

    #[test]
    fn an_ancestor_base_is_ordered_and_stays_silent() {
        let app = artifact(
            "app.tsx",
            &[
                (0, 10, "sw_a", "padding: 40px;"),
                (20, 30, "sw_b", "padding: 16px;"),
                (40, 50, "sw_c", "color: red;"),
            ],
            &[
                ("app.tsx", 1, "app.tsx", 0),
                ("app.tsx", 2, "app.tsx", 0),
                ("app.tsx", 2, "app.tsx", 1),
            ],
        );
        let published = artifacts(&[("app.tsx", app)]);
        let findings = composition_lints(&published, &ModuleId::from("app.tsx"));
        assert!(findings.is_empty(), "{}", codes(&findings));
    }

    #[test]
    fn a_shorthand_and_a_longhand_interfere() {
        let app = artifact(
            "app.tsx",
            &[
                (0, 10, "sw_a", "padding: 40px;"),
                (20, 30, "sw_b", "padding-top: 16px;"),
                (40, 50, "sw_c", "color: red;"),
            ],
            &[("app.tsx", 2, "app.tsx", 0), ("app.tsx", 2, "app.tsx", 1)],
        );
        let published = artifacts(&[("app.tsx", app)]);
        let findings = composition_lints(&published, &ModuleId::from("app.tsx"));
        assert_eq!(findings.len(), 1, "{}", codes(&findings));
        let message = findings[0].kind.to_string();
        assert!(message.contains("padding"), "{message}");
        assert!(message.contains("padding-top"), "{message}");
    }

    #[test]
    fn unrelated_properties_stay_silent() {
        let app = artifact(
            "app.tsx",
            &[
                (0, 10, "sw_a", "padding: 40px;"),
                (20, 30, "sw_b", "margin: 16px;"),
                (40, 50, "sw_c", "color: red;"),
            ],
            &[("app.tsx", 2, "app.tsx", 0), ("app.tsx", 2, "app.tsx", 1)],
        );
        let published = artifacts(&[("app.tsx", app)]);
        let findings = composition_lints(&published, &ModuleId::from("app.tsx"));
        assert!(findings.is_empty(), "{}", codes(&findings));
    }

    #[test]
    fn variant_contexts_do_not_interfere() {
        let app = artifact(
            "app.tsx",
            &[
                (0, 10, "sw_a", "margin: 1px;"),
                (
                    20,
                    30,
                    "sw_b",
                    "@media (hover: hover) { .sw_b:hover { margin: 2px; } }",
                ),
                (40, 50, "sw_c", "color: red;"),
            ],
            &[("app.tsx", 2, "app.tsx", 0), ("app.tsx", 2, "app.tsx", 1)],
        );
        let published = artifacts(&[("app.tsx", app)]);
        let findings = composition_lints(&published, &ModuleId::from("app.tsx"));
        assert!(findings.is_empty(), "{}", codes(&findings));
    }

    #[test]
    fn expanded_base_and_state_selectors_do_not_interfere() {
        let mut app = artifact(
            "app.tsx",
            &[
                (0, 10, "sw_a", "padding: 3px;"),
                (20, 30, "sw_b", "padding: 1px;"),
                (40, 50, "sw_c", "padding: 2px;"),
                (60, 70, "sw_d", "color: red;"),
            ],
            &[
                ("app.tsx", 3, "app.tsx", 0),
                ("app.tsx", 3, "app.tsx", 1),
                ("app.tsx", 3, "app.tsx", 2),
            ],
        );
        app.css.rules[1].syntax.prelude = ".sw_b:disabled".to_owned();
        app.css.rules[2].syntax.prelude = ".sw_c[data-type=\"icon\"]".to_owned();
        let published = artifacts(&[("app.tsx", app)]);

        let findings = composition_lints(&published, &ModuleId::from("app.tsx"));

        assert!(findings.is_empty(), "{}", codes(&findings));
    }

    #[test]
    fn equal_expanded_selectors_interfere_across_generated_classes() {
        let mut app = artifact(
            "app.tsx",
            &[
                (0, 10, "sw_a", "padding: 3px;"),
                (20, 30, "sw_b", "padding: 1px;"),
                (40, 50, "sw_c", "color: red;"),
            ],
            &[("app.tsx", 2, "app.tsx", 0), ("app.tsx", 2, "app.tsx", 1)],
        );
        app.css.rules[0].syntax.prelude = ".sw_a:disabled".to_owned();
        app.css.rules[1].syntax.prelude = ".sw_b:disabled".to_owned();
        let published = artifacts(&[("app.tsx", app)]);

        let findings = composition_lints(&published, &ModuleId::from("app.tsx"));

        assert_eq!(findings.len(), 1, "{}", codes(&findings));
        assert_eq!(findings[0].code(), "ambiguous-base-order");
    }

    #[test]
    fn selector_prefixes_are_part_of_the_expanded_context() {
        let mut app = artifact(
            "app.tsx",
            &[
                (0, 10, "sw_a", "padding: 3px;"),
                (20, 30, "sw_b", "padding: 1px;"),
                (40, 50, "sw_c", "color: red;"),
            ],
            &[("app.tsx", 2, "app.tsx", 0), ("app.tsx", 2, "app.tsx", 1)],
        );
        app.css.rules[1].syntax.prelude = ":where(:focus) .sw_b".to_owned();
        let published = artifacts(&[("app.tsx", app)]);

        let findings = composition_lints(&published, &ModuleId::from("app.tsx"));

        assert!(findings.is_empty(), "{}", codes(&findings));
    }

    #[test]
    fn equal_selectors_inside_at_rules_share_a_context() {
        let mut app = artifact(
            "app.tsx",
            &[
                (0, 10, "sw_a", ".sw_a:hover { padding: 3px; }"),
                (20, 30, "sw_b", ".sw_b:hover { padding: 1px; }"),
                (40, 50, "sw_c", "color: red;"),
            ],
            &[("app.tsx", 2, "app.tsx", 0), ("app.tsx", 2, "app.tsx", 1)],
        );
        app.css.rules[0].syntax.prelude = "@media (hover: hover)".to_owned();
        app.css.rules[1].syntax.prelude = "@media (hover: hover)".to_owned();
        let published = artifacts(&[("app.tsx", app)]);

        let findings = composition_lints(&published, &ModuleId::from("app.tsx"));

        assert_eq!(findings.len(), 1, "{}", codes(&findings));
        assert_eq!(findings[0].code(), "ambiguous-base-order");
    }
}
