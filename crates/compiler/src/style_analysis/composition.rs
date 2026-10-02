//! Recursive base resolution with source-qualified decisions and shared summaries.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use stillwind_semantic::IdentityId;

use super::{StyleEffect, StyleEffects, StyleSupply, UseId};

/// Resolves supplies against the artifacts available in one coherent revision.
pub struct StyleResolver<'a> {
    registry: Arc<BTreeMap<IdentityId, &'a StyleEffects>>,
    cache: BTreeMap<IdentityId, Summary<'a>>,
}

impl<'a> StyleResolver<'a> {
    /// Registers direct effects. Entries must belong to the same published revision.
    pub fn new(effects: impl IntoIterator<Item = &'a StyleEffects>) -> Self {
        Self {
            registry: Arc::new(
                effects
                    .into_iter()
                    .map(|e| (e.identity.clone(), e))
                    .collect(),
            ),
            cache: BTreeMap::new(),
        }
    }

    /// Resolves a call and its reachable bases. Missing calls and cycles remain unknown.
    pub fn resolve(&mut self, identity: &IdentityId) -> Option<ResolvedStyle<'a>> {
        let root = *self.registry.get(identity)?;
        let summary = self.summary(identity, &mut BTreeSet::new());
        Some(ResolvedStyle {
            root,
            supplies: summary.supplies,
            unknown: summary.unknown,
            registry: self.registry.clone(),
        })
    }

    fn summary(&mut self, identity: &IdentityId, active: &mut BTreeSet<IdentityId>) -> Summary<'a> {
        if let Some(summary) = self.cache.get(identity) {
            return summary.clone();
        }
        if !active.insert(identity.clone()) {
            return Summary {
                unknown: vec![Vec::new()],
                ..Summary::default()
            };
        }
        let Some(effects) = self.registry.get(identity).copied() else {
            active.remove(identity);
            return Summary {
                unknown: vec![Vec::new()],
                ..Summary::default()
            };
        };
        let mut result = Summary::default();
        self.walk(&effects.root, identity, &[], None, active, &mut result);
        active.remove(identity);
        result.supplies.sort_by(|a, b| {
            (&a.owner, &a.supply.site, &a.decisions).cmp(&(&b.owner, &b.supply.site, &b.decisions))
        });
        result.supplies.dedup_by(|a, b| {
            a.owner == b.owner
                && a.supply == b.supply
                && a.decisions == b.decisions
                && a.base == b.base
        });
        self.cache.insert(identity.clone(), result.clone());
        result
    }

    fn walk(
        &mut self,
        node: &'a StyleEffect,
        owner: &IdentityId,
        decisions: &[Decision],
        base: Option<&IdentityId>,
        active: &mut BTreeSet<IdentityId>,
        out: &mut Summary<'a>,
    ) {
        match node {
            StyleEffect::All(nodes) => {
                for node in nodes {
                    self.walk(node, owner, decisions, base, active, out);
                }
            }
            StyleEffect::Branch {
                condition,
                branches,
            } => {
                let mut visit = |node: &'a StyleEffect, arm| {
                    let mut path = decisions.to_vec();
                    path.push(Decision {
                        site: condition.clone(),
                        arm,
                        choices: 2,
                    });
                    self.walk(node, owner, &path, base, active, out);
                };
                visit(branches.consequent(), 1);
                if let Some(alternate) = branches.alternate() {
                    visit(alternate, 0);
                }
            }
            StyleEffect::Choice { site, alternatives } => {
                for (arm, node) in alternatives.iter().enumerate() {
                    let mut path = decisions.to_vec();
                    path.push(Decision {
                        site: site.clone(),
                        arm,
                        choices: alternatives.len(),
                    });
                    self.walk(node, owner, &path, base, active, out);
                }
            }
            StyleEffect::Supply(supply) => out.supplies.push(ResolvedSupply {
                owner: owner.clone(),
                supply,
                decisions: decisions.to_vec(),
                base: base.cloned(),
            }),
            StyleEffect::Base(identity) => {
                let summary = self.summary(identity, active);
                for mut path in summary.unknown {
                    path.extend_from_slice(decisions);
                    if compatible(&path, &path) {
                        out.unknown.push(path);
                    }
                }
                for mut supply in summary.supplies {
                    supply.decisions.extend_from_slice(decisions);
                    supply.base = Some(base.unwrap_or(identity).clone());
                    if compatible(&supply.decisions, &supply.decisions) {
                        out.supplies.push(supply);
                    }
                }
            }
            StyleEffect::Unknown => out.unknown.push(decisions.to_vec()),
        }
    }
}

/// Supplies visible to one call, including their inherited conditions and origins.
pub struct ResolvedStyle<'a> {
    pub(super) root: &'a StyleEffects,
    pub(super) supplies: Vec<ResolvedSupply<'a>>,
    pub(super) unknown: Vec<Vec<Decision>>,
    registry: Arc<BTreeMap<IdentityId, &'a StyleEffects>>,
}

impl ResolvedStyle<'_> {
    /// Whether an unavailable contribution could be present with this supply.
    pub(super) fn unknown_can_coexist(&self, supply: &ResolvedSupply<'_>) -> bool {
        self.unknown
            .iter()
            .any(|path| compatible(path, &supply.decisions))
    }

    /// Whether `from` transitively consumes `to` as a base, ignoring branch outcomes.
    pub(super) fn reaches(&self, from: &IdentityId, to: &IdentityId) -> bool {
        let mut pending = vec![from.clone()];
        let mut seen = BTreeSet::new();
        while let Some(identity) = pending.pop() {
            if !seen.insert(identity.clone()) {
                continue;
            }
            if let Some(effects) = self.registry.get(&identity) {
                let mut bases = Vec::new();
                collect_bases(&effects.root, &mut bases);
                for base in bases {
                    if base == to {
                        return true;
                    }
                    pending.push(base.clone());
                }
            }
        }
        false
    }
}

#[derive(Clone, Default)]
struct Summary<'a> {
    supplies: Vec<ResolvedSupply<'a>>,
    unknown: Vec<Vec<Decision>>,
}

/// One original supply under all conditions required by the consuming call.
#[derive(Clone)]
pub(super) struct ResolvedSupply<'a> {
    pub owner: IdentityId,
    pub supply: &'a StyleSupply,
    pub decisions: Vec<Decision>,
    pub base: Option<IdentityId>,
}

impl ResolvedSupply<'_> {
    pub fn coexists(&self, other: &Self) -> bool {
        compatible(&self.decisions, &other.decisions)
    }
    pub fn same_supply(&self, other: &Self) -> bool {
        self.owner == other.owner && self.supply == other.supply
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct Decision {
    site: UseId,
    arm: usize,
    choices: usize,
}

fn compatible(a: &[Decision], b: &[Decision]) -> bool {
    !a.iter()
        .any(|a| b.iter().any(|b| a.site == b.site && a.arm != b.arm))
}

fn collect_bases<'a>(node: &'a StyleEffect, out: &mut Vec<&'a IdentityId>) {
    match node {
        StyleEffect::All(nodes)
        | StyleEffect::Choice {
            alternatives: nodes,
            ..
        } => {
            for node in nodes {
                collect_bases(node, out);
            }
        }
        StyleEffect::Branch { branches, .. } => {
            collect_bases(branches.consequent(), out);
            if let Some(alt) = branches.alternate() {
                collect_bases(alt, out);
            }
        }
        StyleEffect::Base(identity) => out.push(identity),
        _ => {}
    }
}

/// Whether the required supplies can occur while every excluded supply is absent.
/// Each excluded guard is a conjunction, so absence requires at least one of its
/// decisions to select a different arm. Search retains finite decision domains.
pub(super) fn can_omit(required: &[&ResolvedSupply<'_>], excluded: &[&ResolvedSupply<'_>]) -> bool {
    let decisions = required
        .iter()
        .flat_map(|s| s.decisions.clone())
        .collect::<Vec<_>>();
    let mut pending = vec![(0, decisions)];
    while let Some((index, decisions)) = pending.pop() {
        if !compatible(&decisions, &decisions) {
            continue;
        }
        if index == excluded.len() {
            return true;
        }
        let guard = &excluded[index].decisions;
        if !compatible(&decisions, guard) {
            pending.push((index + 1, decisions));
            continue;
        }
        for step in guard {
            if decisions.iter().any(|known| known.site == step.site) {
                continue;
            }
            for arm in 0..step.choices {
                if arm == step.arm {
                    continue;
                }
                let mut path = decisions.clone();
                path.push(Decision {
                    site: step.site.clone(),
                    arm,
                    choices: step.choices,
                });
                pending.push((index + 1, path));
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use stillwind_diagnostics::LintKind;
    use stillwind_ir::ConditionalBranches;
    use stillwind_semantic::AssetKind;
    use stillwind_source::{FileId, ModuleId, Span};

    use super::*;
    use crate::style_analysis::{
        DeclarationValue, RuleContext, RuleSubject, StyleSupply, SupplyKind, check_style,
    };

    fn identity(module: &str) -> IdentityId {
        IdentityId::new(AssetKind::Style, ModuleId::from(module), 0, 100, "")
    }

    fn site(module: &str) -> UseId {
        UseId {
            module: ModuleId::from(module),
            span: Span::new(FileId(1), 10, 20),
        }
    }

    fn supply(module: &str, kind: SupplyKind) -> StyleEffect {
        StyleEffect::Supply(StyleSupply {
            site: site(module),
            ordinal: 0,
            kind,
        })
    }

    fn effects(module: &str, root: StyleEffect) -> StyleEffects {
        StyleEffects {
            identity: identity(module),
            span: Span::new(FileId(1), 0, 100),
            root,
        }
    }

    fn name(module: &str) -> StyleEffect {
        supply(
            module,
            SupplyKind::Declaration {
                property: "view-transition-name".into(),
                value: DeclarationValue::Css(module.into()),
                important: false,
                provider: false,
                gate: None,
                context: RuleContext {
                    selector: "&".into(),
                    at_rules: Vec::new(),
                    subject: RuleSubject::Target,
                },
            },
        )
    }

    fn branch(module: &str, yes: StyleEffect, no: StyleEffect) -> StyleEffect {
        StyleEffect::Branch {
            condition: site(module),
            branches: ConditionalBranches::Ternary {
                consequent: Box::new(yes),
                alternate: Box::new(no),
            },
        }
    }

    fn transition() -> StyleEffect {
        supply(
            "root",
            SupplyKind::Transition {
                identity: IdentityId::new(
                    AssetKind::ViewTransition,
                    ModuleId::from("transition"),
                    0,
                    10,
                    "vt",
                ),
                context: RuleContext {
                    selector: "&".into(),
                    at_rules: Vec::new(),
                    subject: RuleSubject::Target,
                },
            },
        )
    }

    #[test]
    fn equal_offsets_in_different_modules_do_not_make_conditions_exclusive() {
        let a = effects("a", branch("a", name("a"), StyleEffect::All(Vec::new())));
        let b = effects("b", branch("b", StyleEffect::All(Vec::new()), name("b")));
        let root = effects(
            "root",
            StyleEffect::All(vec![
                StyleEffect::Base(a.identity.clone()),
                StyleEffect::Base(b.identity.clone()),
                transition(),
            ]),
        );
        let mut resolver = StyleResolver::new([&a, &b, &root]);
        let findings = check_style(&resolver.resolve(&root.identity).unwrap());
        assert!(
            findings
                .iter()
                .any(|f| f.kind() == LintKind::SingleViewTransitionName)
        );
    }

    #[test]
    fn finite_candidates_are_exclusive_at_one_use() {
        let a = effects("a", name("a"));
        let b = effects("b", name("b"));
        let root = effects(
            "root",
            StyleEffect::All(vec![
                transition(),
                StyleEffect::Choice {
                    site: site("root"),
                    alternatives: vec![
                        StyleEffect::Base(a.identity.clone()),
                        StyleEffect::Base(b.identity.clone()),
                    ],
                },
            ]),
        );
        let mut resolver = StyleResolver::new([&a, &b, &root]);
        assert!(check_style(&resolver.resolve(&root.identity).unwrap()).is_empty());
    }

    #[test]
    fn unavailable_bases_only_hide_absence_in_compatible_branches() {
        let reference = IdentityId::new(AssetKind::Ref, ModuleId::from("ref"), 0, 10, "ref");
        let provider = supply(
            "root",
            SupplyKind::Provider {
                reference,
                name: "ref".into(),
            },
        );
        let root = effects(
            "root",
            branch(
                "root",
                provider.clone(),
                StyleEffect::Base(identity("missing")),
            ),
        );
        let mut resolver = StyleResolver::new([&root]);
        assert_eq!(
            check_style(&resolver.resolve(&root.identity).unwrap())
                .iter()
                .map(stillwind_diagnostics::LintFinding::kind)
                .collect::<Vec<_>>(),
            vec![LintKind::RequireRefAttachment]
        );
        let root = effects(
            "root",
            StyleEffect::All(vec![provider, StyleEffect::Base(identity("missing"))]),
        );
        let mut resolver = StyleResolver::new([&root]);
        assert!(check_style(&resolver.resolve(&root.identity).unwrap()).is_empty());
    }
}
