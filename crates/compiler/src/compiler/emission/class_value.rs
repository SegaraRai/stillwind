//! Class and rule expression lowering for one `style()` call.
//!
//! [`CallEmitter::emit_item`] enters the source class tree. Rule descendants are
//! resolved under their CSS variant wrappers; contiguous rules within a JavaScript
//! condition region contribute to one generated class. Root object entries retain
//! their independent props channels. Final framework fields and helper imports are
//! selected later by fragment rendering.

use stillwind_diagnostics::{LintDiagnostic, LintFinding, LintKind};
use stillwind_ir::{
    ClassExpression, ConditionalBranches, GroupEntry, RuleExpression, StyleVariant,
};
use stillwind_source::Span;

use super::super::fragment::{Emitted, EmittedEntry, ValueExpr};
use super::super::literal::LiteralContribution;
use super::super::props::{GuardStep, StyleEmissionPart};
use super::super::style_variants::{StyleVariantPlan, parse_prefix};
use super::super::utilities::policy::check_variant;
use super::super::variants::document_scoped_selector;
use super::{CallEmitter, PendingRegion};
use crate::editor_lint::literal_boundaries;
use crate::generated::js_string;
use crate::runtime::RuntimeHelper;

impl CallEmitter<'_> {
    /// Reports source policies once per authored variant wrapper, even when that
    /// wrapper covers several literals. Container queries have no viewport policy.
    fn report_wrapper_lints(&mut self, variant: &StyleVariant) {
        let prefixes = match variant {
            StyleVariant::Prefix(prefix) => vec![prefix],
            StyleVariant::Ref { condition, scopes } => std::iter::once(&condition.prefix)
                .chain(scopes.iter().map(|scope| &scope.prefix))
                .collect(),
            StyleVariant::Container { .. } => Vec::new(),
        };
        for prefix in prefixes {
            if let Some(variants) = parse_prefix(prefix) {
                let boundaries = literal_boundaries(self.source, prefix);
                let mut offset = 0;
                for variant in variants {
                    let end = offset + variant.raw.len();
                    let span = boundaries
                        .as_ref()
                        .and_then(|boundaries| {
                            Some(Span::new(
                                prefix.span.file,
                                *boundaries.get(offset)?,
                                *boundaries.get(end)?,
                            ))
                        })
                        .unwrap_or(prefix.span);
                    self.lint_findings.extend(check_variant(
                        &variant,
                        self.theme,
                        &self.lint_settings,
                        span,
                    ));
                    offset = end + 1;
                }
            }
        }
        let StyleVariant::Prefix(prefix) = variant else {
            return;
        };
        if !self
            .lint_settings
            .enabled_for_detection(LintKind::RequireElementScope)
        {
            return;
        }
        let Some(variants) = parse_prefix(prefix) else {
            return;
        };
        let theme = self.theme;
        if let Some(selector) = variants
            .iter()
            .find_map(|variant| document_scoped_selector(variant, theme))
        {
            self.lint_findings.push(LintFinding::new(
                LintDiagnostic::DocumentScopedSelector { selector },
                prefix.span,
            ));
        }
    }

    /// Lowers a class item under its enclosing input guards and an empty variant stack.
    /// Appends retained runtime code at `position`; omitted contributions may instead
    /// have joined the base unit or reported diagnostics during lowering.
    pub(super) fn emit_item(
        &mut self,
        expression: &ClassExpression,
        position: u32,
        inherited: &[GuardStep],
    ) {
        let mut wrappers = Vec::new();
        let mut guard = inherited.to_vec();
        if let Some(emitted) = self.emit_expression(expression, &mut guard, &mut wrappers, true) {
            self.parts
                .push(StyleEmissionPart::Class { emitted, position });
        }
    }

    /// Lowers one class expression, tracking the guard path and the active wrappers.
    ///
    /// `fold` is set at the top level of a call: a rule reached without a guard can
    /// join the base unit. Conditional arms collect contiguous rules into guarded class
    /// units. `None` covers base-folded, provably empty, and failed contributions.
    /// Guard and wrapper stacks are restored after nested traversal; callers inspect
    /// diagnostics separately rather than treating absence as a successful empty
    /// expression.
    fn emit_expression(
        &mut self,
        expression: &ClassExpression,
        guard: &mut Vec<GuardStep>,
        wrappers: &mut Vec<StyleVariant>,
        fold: bool,
    ) -> Option<Emitted> {
        self.emit_expression_inner(expression, guard, wrappers, fold)
            .filter(|value| !value.is_empty())
    }

    /// Resolves one class node while preserving guards, variant context, and source mappings.
    /// May allocate CSS/class units or record asset dependencies and diagnostics.
    /// Returns `None` for a base-folded leaf or a lowering failure; empty runtime nodes
    /// are left for `emit_expression` to filter.
    #[allow(clippy::too_many_lines)] // One exhaustive match over the class expression.
    fn emit_expression_inner(
        &mut self,
        expression: &ClassExpression,
        guard: &mut Vec<GuardStep>,
        wrappers: &mut Vec<StyleVariant>,
        fold: bool,
    ) -> Option<Emitted> {
        match expression {
            ClassExpression::Rule(rule) => self.emit_rule_expression(rule, guard, wrappers, fold),
            ClassExpression::Array(elements) => {
                let mut items = Vec::new();
                let mut any = false;
                for element in elements {
                    let item = self.emit_expression(element, guard, wrappers, fold);
                    any |= item.is_some();
                    items.push(item);
                }
                any.then_some(Emitted::Array(items))
            }
            ClassExpression::Conditional { test, branches } => {
                let in_region = self.pending_region.is_some();
                let preceding = self.end_region(guard);
                let test_span = *test;
                let test = self.source_expression(test_span)?;
                let branches = match branches {
                    ConditionalBranches::And { consequent } => {
                        guard.push(GuardStep {
                            test: test_span,
                            when_true: true,
                        });
                        let consequent = self.emit_sequence(consequent, guard, wrappers);
                        guard.pop();
                        ConditionalBranches::And {
                            consequent: consequent.map(Box::new),
                        }
                    }
                    ConditionalBranches::Ternary {
                        consequent,
                        alternate,
                    } => {
                        guard.push(GuardStep {
                            test: test_span,
                            when_true: true,
                        });
                        let consequent = self.emit_sequence(consequent, guard, wrappers);
                        guard.pop();
                        guard.push(GuardStep {
                            test: test_span,
                            when_true: false,
                        });
                        let alternate = self.emit_sequence(alternate, guard, wrappers);
                        guard.pop();
                        ConditionalBranches::Ternary {
                            consequent: consequent.map(Box::new),
                            alternate: alternate.map(Box::new),
                        }
                    }
                };
                let conditional = Some(Emitted::Conditional {
                    code: test,
                    branches,
                });
                if in_region {
                    self.begin_region();
                }
                match (preceding, conditional) {
                    (Some(preceding), Some(conditional)) => {
                        Some(Emitted::Array(vec![Some(preceding), Some(conditional)]))
                    }
                    (None, conditional) => conditional,
                    (Some(preceding), None) => Some(preceding),
                }
            }
            ClassExpression::Group { entries, .. } => {
                let mut items = Vec::new();
                let mut any = false;
                for entry in entries {
                    let item = match entry {
                        GroupEntry::Class(expression) => self
                            .emit_expression(expression, guard, wrappers, fold)
                            .map(EmittedEntry::Class),
                        GroupEntry::Attribute(attribute) => {
                            let property = self.render_attribute(attribute)?;
                            Some(EmittedEntry::Attribute(property))
                        }
                        GroupEntry::CustomProperty(property) => {
                            let custom_property = self.render_custom_property(property)?;
                            Some(EmittedEntry::CustomProperty(custom_property))
                        }
                    };
                    any |= item.is_some();
                    items.push(item);
                }
                any.then_some(Emitted::Group(items))
            }
            ClassExpression::ClassValue { span, arguments } => {
                if self
                    .lint_settings
                    .enabled_for_detection(LintKind::NoClassProperty)
                {
                    self.lint_findings
                        .push(LintFinding::new(LintDiagnostic::ClassProperty, *span));
                }
                if arguments.is_empty() {
                    return None;
                }
                let expression = ValueExpr::Call {
                    helper: RuntimeHelper::Clsx,
                    arguments: arguments
                        .iter()
                        .map(|argument| self.source_expression(*argument))
                        .collect::<Option<Vec<_>>>()?,
                    span: *span,
                };
                Some(Emitted::Expression(expression))
            }
        }
    }

    /// Resolves a rule subtree under the active JavaScript guards and CSS variants.
    /// Returns the retained runtime class contribution, or `None` when the rule folds
    /// into a region/base unit, contributes nothing, or fails resolution. Diagnostics
    /// are appended separately for failures. Resolved literals record their
    /// declarations before class merging; transition and ref uses record their semantic
    /// requirements. The caller supplies original source spans, and successful
    /// traversal restores temporary guards and wrappers before returning.
    #[allow(clippy::too_many_lines)] // One exhaustive match over rule expressions.
    fn emit_rule_expression(
        &mut self,
        expression: &RuleExpression,
        guard: &mut Vec<GuardStep>,
        wrappers: &mut Vec<StyleVariant>,
        fold: bool,
    ) -> Option<Emitted> {
        let emitted = match expression {
            RuleExpression::Utilities(value) => {
                let plan = StyleVariantPlan::resolve(
                    self.identifiers,
                    wrappers,
                    self.theme,
                    self.diagnostics,
                )?;
                let mut contribution = LiteralContribution::resolve(
                    value,
                    &plan,
                    self.theme,
                    &self.lint_settings,
                    self.diagnostics,
                    self.lint_findings,
                );
                if let Some(boundaries) = literal_boundaries(self.source, value) {
                    for utility in &mut contribution.utilities {
                        for span in &mut utility.source.variant_spans {
                            let start = (span.start.0 - value.span.start.0) as usize;
                            let end = (span.end.0 - value.span.start.0) as usize;
                            if let (Some(&start), Some(&end)) =
                                (boundaries.get(start), boundaries.get(end))
                            {
                                *span = Span::new(value.span.file, start, end);
                            }
                        }
                        if let Some(span) = utility.source.span {
                            let start = (span.start.0 - value.span.start.0) as usize;
                            let end = (span.end.0 - value.span.start.0) as usize;
                            if let (Some(&start), Some(&end)) =
                                (boundaries.get(start), boundaries.get(end))
                            {
                                utility.source.span = Some(Span::new(value.span.file, start, end));
                            }
                        }
                    }
                }
                self.record_properties(&contribution, false, value.span, guard, None, None);
                self.contribute_rule(contribution, guard, fold)
            }
            RuleExpression::Array(elements) => {
                let items = elements
                    .iter()
                    .map(|element| self.emit_rule_expression(element, guard, wrappers, fold))
                    .collect::<Vec<_>>();
                items
                    .iter()
                    .any(Option::is_some)
                    .then_some(Emitted::Array(items))
            }
            RuleExpression::Conditional { test, branches } => {
                let in_region = self.pending_region.is_some();
                let preceding = self.end_region(guard);
                let code = self.source_expression(*test)?;
                let mut emit_arm = |values: &[RuleExpression], when_true| {
                    guard.push(GuardStep {
                        test: *test,
                        when_true,
                    });
                    let value = self.emit_rule_sequence(values, guard, wrappers);
                    guard.pop();
                    value.map(Box::new)
                };
                let branches = match branches {
                    ConditionalBranches::And { consequent } => ConditionalBranches::And {
                        consequent: emit_arm(consequent, true),
                    },
                    ConditionalBranches::Ternary {
                        consequent,
                        alternate,
                    } => ConditionalBranches::Ternary {
                        consequent: emit_arm(consequent, true),
                        alternate: emit_arm(alternate, false),
                    },
                };
                if in_region {
                    self.begin_region();
                }
                let conditional = Emitted::Conditional { code, branches };
                Some(match preceding {
                    Some(preceding) => Emitted::Array(vec![Some(preceding), Some(conditional)]),
                    None => conditional,
                })
            }
            RuleExpression::Variant { variant, inputs } => {
                self.report_wrapper_lints(variant);
                wrappers.push(variant.clone());
                let items = inputs
                    .iter()
                    .map(|input| self.emit_rule_expression(input, guard, wrappers, fold))
                    .collect::<Vec<_>>();
                wrappers.pop();
                if !items.iter().any(Option::is_some) {
                    None
                } else if items.len() == 1 {
                    items.into_iter().next().flatten()
                } else {
                    Some(Emitted::Array(items))
                }
            }
            RuleExpression::Group { inputs, .. } => {
                let items = inputs
                    .iter()
                    .map(|input| {
                        self.emit_rule_expression(input, guard, wrappers, fold)
                            .map(EmittedEntry::Class)
                    })
                    .collect::<Vec<_>>();
                items
                    .iter()
                    .any(Option::is_some)
                    .then_some(Emitted::Group(items))
            }
            RuleExpression::AnchorScope { scope, span } => {
                self.emit_anchor_scope(scope, *span, guard, wrappers, fold)
            }
            RuleExpression::RefProjection {
                definition,
                value,
                projection,
            } => self.emit_ref_projection(definition, *value, projection, guard, wrappers, fold),
            RuleExpression::DynamicRefProjection {
                value,
                refs,
                projection,
            } => self.emit_dynamic_ref_projection(value, refs, projection, guard, wrappers),
            RuleExpression::ViewTransitionUse { transition, name } => {
                self.emit_view_transition_expression(transition, name, guard, wrappers, fold)
            }
            RuleExpression::DynamicViewTransitionUse {
                value,
                transitions,
                name,
            } => self.emit_dynamic_view_transition_expression(
                value,
                transitions,
                name,
                guard,
                wrappers,
            ),
            RuleExpression::KeyframesUse { keyframes, options } => {
                self.emit_keyframes_use(keyframes, options, guard, wrappers, fold)
            }
            RuleExpression::DynamicKeyframesUse {
                value,
                keyframes,
                options,
            } => self.emit_dynamic_keyframes_use(value, keyframes, options, guard, wrappers),
        };
        emitted.filter(|value| !value.is_empty())
    }

    /// Collects all rule contributions in one conditional arm into contiguous regions.
    /// Returns their runtime classes and values in encounter order, or `None` when the
    /// arm has no retained contribution.
    fn emit_rule_sequence(
        &mut self,
        expressions: &[RuleExpression],
        guard: &mut Vec<GuardStep>,
        wrappers: &mut Vec<StyleVariant>,
    ) -> Option<Emitted> {
        let outer = self.begin_region();
        let mut items = Vec::new();
        for expression in expressions {
            if let Some(item) = self.emit_rule_expression(expression, guard, wrappers, false) {
                items.push(item);
            }
        }
        let trailing = self.end_region(guard);
        self.pending_region = outer;
        if let Some(class) = trailing {
            items.push(class);
        }
        match items.len() {
            0 => None,
            1 => items.pop(),
            _ => Some(Emitted::Array(items.into_iter().map(Some).collect())),
        }
    }

    /// Lowers one branch with the supplied JavaScript conditions and CSS variants.
    /// Consecutive rules share a class region. Nested JavaScript conditions separate
    /// those regions.
    ///
    /// Returns the remaining classes and values in encounter order. Empty groups are
    /// removed and groups with one item are simplified under the macro purity contract.
    /// Failures add diagnostics. Previously allocated contributions may remain.
    fn emit_sequence(
        &mut self,
        expressions: &[ClassExpression],
        guard: &mut Vec<GuardStep>,
        wrappers: &mut Vec<StyleVariant>,
    ) -> Option<Emitted> {
        let outer = self.begin_region();
        let mut items = Vec::new();
        for expression in expressions {
            if let Some(item) = self.emit_expression(expression, guard, wrappers, false) {
                items.push(item);
            }
        }
        let trailing = self.end_region(guard);
        self.pending_region = outer;
        if let Some(class) = trailing {
            items.push(class);
        }
        match items.len() {
            0 => None,
            1 => items.pop(),
            _ => Some(Emitted::Array(items.into_iter().map(Some).collect())),
        }
    }

    /// Lowers all possible candidates for the receiver at `value`.
    /// Each candidate must pair a unique canonical identity token with its resolved
    /// rules. `key` is the identity field in the compiled receiver (`c`, `k`, or `v`).
    /// `guard` lists the enclosing JavaScript conditions.
    ///
    /// With one candidate, the rules join the current region or base without reading
    /// the receiver. With multiple candidates, each gets a leaf unit and the returned
    /// expression looks up the selected class.
    ///
    /// `None` means either that the single candidate was merged or that lowering failed.
    /// Failures add diagnostics and keep earlier allocations. Callers must check the
    /// diagnostics to distinguish these outcomes.
    pub(super) fn emit_candidate_units(
        &mut self,
        value: &Span,
        key: &str,
        candidates: Vec<(String, LiteralContribution)>,
        guard: &[GuardStep],
    ) -> Option<Emitted> {
        if candidates.len() == 1 {
            let (_, contribution) = candidates.into_iter().next()?;
            return self.contribute_rule(contribution, guard, true);
        }
        let mut table = Vec::new();
        for (identity, contribution) in candidates {
            let class = match self.units.leaf(contribution, guard) {
                Ok(class) => class,
                Err(failure) => {
                    self.reject_name(failure, self.call.span);
                    return None;
                }
            };
            let Emitted::Leaf { names } = class else {
                return None;
            };
            let key = js_string(&identity);
            let key = if identity == "__proto__" {
                format!("[{key}]")
            } else {
                key
            };
            table.push(format!("{key}:{}", js_string(&names.join(" "))));
        }
        Some(Emitted::Expression(ValueExpr::Lookup {
            value: Box::new(ValueExpr::Source(self.source_expression(*value)?)),
            key: key.to_owned(),
            table: format!("{{{}}}", table.join(",")).into(),
        }))
    }

    /// Adds resolved rules under the JavaScript conditions in `guard`.
    /// If a region is pending, the rules join it regardless of `fold`. Otherwise, `fold`
    /// allows the rules to join the base only when `guard` is empty.
    ///
    /// Returns an expression for class names that must still be added at runtime, or
    /// for a generated leaf. `None` means that the rules were merged or that allocation
    /// or emission failed. Failures add diagnostics.
    pub(super) fn contribute_rule(
        &mut self,
        mut contribution: LiteralContribution,
        guard: &[GuardStep],
        fold: bool,
    ) -> Option<Emitted> {
        if let Some(region) = &mut self.pending_region {
            let mut names = Vec::new();
            names.append(&mut contribution.markers);
            names.append(&mut contribution.external);
            names.append(&mut contribution.plain);
            region.rules.append_rules(contribution);
            return (!names.is_empty()).then_some(Emitted::Leaf { names });
        }
        if fold && guard.is_empty() {
            self.units.fold(contribution);
            return None;
        }
        match self.units.leaf(contribution, guard) {
            Ok(leaf) => Some(leaf),
            Err(failure) => {
                self.reject_name(failure, self.call.span);
                None
            }
        }
    }

    /// Allocates a class for the collected rules under the conditions in `guard`.
    /// The caller must reserve `order` first and extract any marker or external class
    /// names from `contribution` before calling this function.
    ///
    /// Returns a generated leaf. Returns `None` if the region has no rules or allocation
    /// or emission fails. Failures add diagnostics.
    pub(super) fn finish_region(
        &mut self,
        contribution: LiteralContribution,
        guard: &[GuardStep],
        order: u32,
    ) -> Option<Emitted> {
        if contribution.utilities.is_empty() && contribution.view_transitions.is_empty() {
            return None;
        }
        match self.units.leaf_at(contribution, guard, order) {
            Ok(class) => Some(class),
            Err(failure) => {
                self.reject_name(failure, self.call.span);
                None
            }
        }
    }

    /// Starts a rule region and reserves its source order before visiting children.
    /// Returns the previously pending region for the caller to restore, if any.
    pub(super) fn begin_region(&mut self) -> Option<PendingRegion> {
        let region = PendingRegion {
            order: self.units.reserve_branch(),
            rules: LiteralContribution::default(),
        };
        self.pending_region.replace(region)
    }

    /// Ends the pending contiguous rule region before another region begins. Returns
    /// its generated class expression when it contributed rules and allocation
    /// succeeded; `None` also covers the absence of a pending region.
    pub(super) fn end_region(&mut self, guard: &[GuardStep]) -> Option<Emitted> {
        let region = self.pending_region.take()?;
        self.finish_region(region.rules, guard, region.order)
    }
}

/// Returns an original-source UTF-8 byte anchor for sorting a call's contributions.
///
/// Conditionals use their test, variants and groups use their enclosing span, and
/// arrays use their first element or zero when empty. This establishes initial
/// contribution order; folding separately proves whether evaluations may move.
pub(super) fn expression_position(expression: &ClassExpression) -> u32 {
    match expression {
        ClassExpression::Rule(rule) => rule_position(rule),
        ClassExpression::Array(elements) => elements.first().map_or(0, expression_position),
        ClassExpression::Conditional { test, .. } => test.start.0,
        ClassExpression::Group { span, .. } | ClassExpression::ClassValue { span, .. } => {
            span.start.0
        }
    }
}

/// Returns the original UTF-8 byte anchor of a rule for contribution ordering. An empty
/// rule array uses zero.
fn rule_position(expression: &RuleExpression) -> u32 {
    match expression {
        RuleExpression::Utilities(value) => value.span.start.0,
        RuleExpression::Array(elements) => elements.first().map_or(0, rule_position),
        RuleExpression::Conditional { test, .. } => test.start.0,
        RuleExpression::Variant { variant, .. } => match variant {
            StyleVariant::Prefix(prefix) | StyleVariant::Container { query: prefix, .. } => {
                prefix.span.start.0
            }
            StyleVariant::Ref { condition, .. } => condition.prefix.span.start.0,
        },
        RuleExpression::AnchorScope { span, .. } | RuleExpression::Group { span, .. } => {
            span.start.0
        }
        RuleExpression::RefProjection { value, .. }
        | RuleExpression::DynamicRefProjection { value, .. }
        | RuleExpression::DynamicViewTransitionUse { value, .. }
        | RuleExpression::DynamicKeyframesUse { value, .. } => value.start.0,
        RuleExpression::ViewTransitionUse { transition, .. } => transition.span.start.0,
        RuleExpression::KeyframesUse { keyframes, .. } => keyframes.span.start.0,
    }
}
