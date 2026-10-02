//! Recursive base resolution with source-qualified decisions and shared summaries.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use stillwind_semantic::IdentityId;

use super::{StyleEffect, StyleEffects, StyleSupply, UseId};

/// Resolves style calls against one set of immutable module artifacts.
/// Local and imported bases share the registry and cached summaries. Replace the
/// resolver when any artifact changes so cached supplies retain valid source spans.
pub struct StyleResolver<'a> {
    /// Immutable effects borrowed from the current artifacts, indexed by style identity.
    registry: Arc<BTreeMap<IdentityId, &'a StyleEffects>>,
    /// Summaries retain conditions relative to their original calls.
    cache: BTreeMap<IdentityId, Summary<'a>>,
}

impl<'a> StyleResolver<'a> {
    /// Registers direct effects from the current artifacts and starts an empty cache.
    /// `effects` must contain unique style identities from one coherent revision.
    /// Each effect's source coordinates must still identify its producer's source.
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

    /// Returns the call's direct and inherited supplies, retaining their source guards.
    /// `identity` must identify a style declaration. An unregistered root returns `None`.
    /// Missing bases and cycle edges add unknown paths to a registered root's result.
    /// The method caches summaries for subsequent calls without changing the artifacts.
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

    /// Returns a cached or newly collected summary for `identity`.
    /// `active` contains the current recursion path. New visits are removed before
    /// returning; a missing identity or repeated visit returns one unknown path.
    /// Successfully visited records are cached with their original source conditions.
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

    /// Appends the supplies reachable through `node` to `out`.
    /// `owner` identifies the source call; `decisions` contains enclosing source tests.
    /// `base` identifies the first base used by the caller. Recursive summaries keep
    /// their original owners and acquire the caller's decisions and base identity.
    /// Contradictory inherited paths are omitted. `active` tracks recursion visits.
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
        }
    }
}

/// The supplies and unknown paths visible to one registered style call.
/// Obtain this immutable result through `StyleResolver::resolve` and pass it to
/// `check_style`. The result borrows source records and retains the registry needed
/// to inspect base ancestry independently of the resolver's lifetime.
pub struct ResolvedStyle<'a> {
    /// Original call whose findings are being computed.
    pub(super) root: &'a StyleEffects,
    /// Original supplies with the conditions imposed by every inherited use.
    pub(super) supplies: Vec<ResolvedSupply<'a>>,
    /// Conditions under which unavailable base contributions could occur.
    unknown: Vec<Vec<Decision>>,
    registry: Arc<BTreeMap<IdentityId, &'a StyleEffects>>,
}

impl ResolvedStyle<'_> {
    /// Returns whether any unavailable contribution can coexist with `supply`.
    /// The supply must come from this result. Only source decisions are compared;
    /// the method does not infer implications between CSS conditions.
    pub(super) fn unknown_can_coexist(&self, supply: &ResolvedSupply<'_>) -> bool {
        self.unknown
            .iter()
            .any(|path| compatible(path, &supply.decisions))
    }

    /// Returns whether `from` transitively consumes `to` through registered base edges.
    /// Both identities denote style declarations. Traversal includes every branch,
    /// terminates on cycles, and stops at unregistered producers. Source decisions
    /// must be compared separately when callers need simultaneous contributions.
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

/// Supplies and unavailable paths relative to one original style call.
#[derive(Clone, Default)]
struct Summary<'a> {
    /// Original records with inherited decisions; caller conditions are added when reused.
    supplies: Vec<ResolvedSupply<'a>>,
    /// Guards for base contributions whose records were not available.
    unknown: Vec<Vec<Decision>>,
}

/// One original supply under all conditions required by the consuming call.
#[derive(Clone)]
pub(super) struct ResolvedSupply<'a> {
    /// Call that originally supplied the record.
    pub(super) owner: IdentityId,
    /// Original record before inheritance added conditions.
    pub(super) supply: &'a StyleSupply,
    /// Source decisions required by the original use and consuming bases.
    decisions: Vec<Decision>,
    /// First direct base of the resolved root; absent for its own supplies.
    pub(super) base: Option<IdentityId>,
}

impl ResolvedSupply<'_> {
    /// Returns whether the two supplies require no conflicting source decisions.
    pub(super) fn coexists(&self, other: &Self) -> bool {
        compatible(&self.decisions, &other.decisions)
    }
    /// Returns whether both records describe the same original contribution.
    /// Different inherited paths and direct base identities do not change its origin.
    pub(super) fn same_supply(&self, other: &Self) -> bool {
        self.owner == other.owner && self.supply == other.supply
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
/// One selected arm of a source occurrence with a finite selection domain.
struct Decision {
    /// Module-qualified source occurrence; all uses of a site share its domain.
    site: UseId,
    /// Zero-based selected arm, smaller than `choices`.
    arm: usize,
    /// Number of possible arms; source branches use two.
    choices: usize,
}

/// Returns whether no cross-pair requires different arms of the same occurrence.
/// Each path must already be consistent; comparing a path with itself checks that
/// precondition. Different source sites remain independent even at equal byte offsets.
fn compatible(a: &[Decision], b: &[Decision]) -> bool {
    !a.iter()
        .any(|a| b.iter().any(|b| a.site == b.site && a.arm != b.arm))
}

/// Appends references to every base edge below `node` in traversal order.
/// Both source branches and all finite alternatives contribute; repeated edges remain.
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
        StyleEffect::Supply(_) => {}
    }
}

/// Returns whether all `required` supplies can occur while every `excluded` supply is absent.
/// All supplies must use consistent finite domains for shared source occurrences.
/// An empty excluded list accepts any consistent required path. An unconditional
/// excluded supply makes omission impossible. Search selects a different arm for
/// at least one decision in each excluded guard without changing the records.
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

    use super::super::test_support as f;
    use super::super::{
        DeclarationValue, RuleContext, RuleSubject, StyleSupply, SupplyKind, check_style,
    };
    use super::*;

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

    #[test]
    fn missing_roots_empty_styles_and_unavailable_bases_remain_distinct() {
        let empty = f::effects("empty", StyleEffect::All(Vec::new()));
        let root = f::effects("root", StyleEffect::Base(f::identity("missing")));
        let mut resolver = StyleResolver::new([&empty, &root]);
        assert!(resolver.resolve(&f::identity("missing")).is_none());
        let empty = resolver.resolve(empty.identity()).unwrap();
        assert!(empty.supplies.is_empty());
        assert!(empty.unknown.is_empty());
        let root = resolver.resolve(root.identity()).unwrap();
        assert!(root.supplies.is_empty());
        assert_eq!(root.unknown, vec![Vec::new()]);
    }

    #[test]
    fn recursive_resolution_keeps_origins_and_the_first_direct_base() {
        let base = f::effects(
            "base",
            f::supply("base", 1, f::declaration("padding", f::context("&"))),
        );
        let middle = f::effects("middle", StyleEffect::Base(base.identity.clone()));
        let root = f::effects(
            "root",
            f::branch(
                "root",
                5,
                StyleEffect::Base(middle.identity.clone()),
                StyleEffect::All(Vec::new()),
            ),
        );
        let mut resolver = StyleResolver::new([&root, &middle, &base]);
        for _ in 0..2 {
            let result = resolver.resolve(root.identity()).unwrap();
            assert_eq!(result.supplies.len(), 1);
            let inherited = &result.supplies[0];
            assert_eq!(inherited.owner, base.identity);
            assert_eq!(inherited.supply.site, f::site("base", 1));
            assert_eq!(inherited.base.as_ref(), Some(&middle.identity));
            assert_eq!(
                inherited.decisions,
                vec![Decision {
                    site: f::site("root", 5),
                    arm: 1,
                    choices: 2
                }]
            );
        }
    }

    #[test]
    fn reachability_includes_all_arms_and_terminates_at_cycles_and_missing_records() {
        let a = f::effects(
            "a",
            f::branch(
                "a",
                1,
                StyleEffect::Base(f::identity("b")),
                StyleEffect::Choice {
                    site: f::site("a", 2),
                    alternatives: vec![
                        StyleEffect::Base(f::identity("c")),
                        StyleEffect::Base(f::identity("missing")),
                    ],
                },
            ),
        );
        let b = f::effects("b", StyleEffect::Base(a.identity.clone()));
        let c = f::effects("c", StyleEffect::All(Vec::new()));
        let mut resolver = StyleResolver::new([&a, &b, &c]);
        let result = resolver.resolve(a.identity()).unwrap();
        assert!(!result.unknown.is_empty());
        for target in [
            &a.identity,
            &b.identity,
            &c.identity,
            &f::identity("missing"),
        ] {
            assert!(result.reaches(&a.identity, target));
        }
        assert!(!result.reaches(&a.identity, &f::identity("unrelated")));
        assert!(!result.reaches(&f::identity("missing"), &a.identity));
    }

    #[test]
    fn decision_compatibility_and_original_supply_identity_are_independent() {
        let yes = Decision {
            site: f::site("a", 1),
            arm: 1,
            choices: 2,
        };
        let no = Decision {
            arm: 0,
            ..yes.clone()
        };
        let independent = Decision {
            site: f::site("b", 1),
            ..no.clone()
        };
        assert!(compatible(&[], std::slice::from_ref(&yes)));
        assert!(compatible(
            std::slice::from_ref(&yes),
            std::slice::from_ref(&independent)
        ));
        assert!(!compatible(
            std::slice::from_ref(&yes),
            std::slice::from_ref(&no)
        ));
        assert!(!compatible(
            &[yes.clone(), no.clone()],
            &[yes.clone(), no.clone()]
        ));
        let record = StyleSupply {
            site: f::site("a", 2),
            kind: f::declaration("padding", f::context("&")),
            ordinal: 0,
        };
        let a = ResolvedSupply {
            owner: f::identity("a"),
            supply: &record,
            decisions: vec![yes],
            base: None,
        };
        let b = ResolvedSupply {
            decisions: vec![no],
            base: Some(f::identity("middle")),
            ..a.clone()
        };
        assert!(a.same_supply(&b));
        assert!(!a.coexists(&b));
        let other = ResolvedSupply {
            owner: f::identity("b"),
            ..a.clone()
        };
        assert!(!a.same_supply(&other));
        assert!(a.coexists(&other));
    }

    #[test]
    fn omission_search_honors_required_paths_and_complete_finite_coverage() {
        let record = StyleSupply {
            site: f::site("root", 2),
            kind: f::declaration("view-transition-class", f::context("&")),
            ordinal: 0,
        };
        let plain = ResolvedSupply {
            owner: f::identity("root"),
            supply: &record,
            decisions: Vec::new(),
            base: None,
        };
        let arms = (0..3)
            .map(|arm| ResolvedSupply {
                decisions: vec![Decision {
                    site: f::site("root", 1),
                    arm,
                    choices: 3,
                }],
                ..plain.clone()
            })
            .collect::<Vec<_>>();
        assert!(can_omit(&[], &[]));
        assert!(!can_omit(&[], &[&plain]));
        assert!(can_omit(&[&plain], &[&arms[0], &arms[1]]));
        assert!(!can_omit(&[&plain], &[&arms[0], &arms[1], &arms[2]]));
        assert!(!can_omit(&[&arms[0]], &[&arms[0]]));
        assert!(can_omit(&[&arms[0]], &[&arms[1]]));
        assert!(!can_omit(&[&arms[0], &arms[1]], &[]));
    }

    #[test]
    fn contradictory_inherited_decisions_are_discarded() {
        let record = f::effects(
            "base",
            f::branch(
                "shared",
                1,
                f::supply("base", 2, f::declaration("padding", f::context("&"))),
                StyleEffect::Base(f::identity("missing")),
            ),
        );
        let root = f::effects(
            "root",
            f::branch(
                "shared",
                1,
                StyleEffect::All(Vec::new()),
                StyleEffect::Base(record.identity.clone()),
            ),
        );
        let mut resolver = StyleResolver::new([&record, &root]);
        let result = resolver.resolve(root.identity()).unwrap();
        assert!(result.supplies.is_empty());
        assert_eq!(result.unknown.len(), 1);
    }
}
