//! Resolves asset candidates and preserves why an expression cannot be lowered.

use oxc::ast::ast::Expression;
use oxc::span::GetSpan;
use oxc::syntax::operator::LogicalOperator;

use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintDiagnostic, LintFinding};
use stillwind_ir::{ClassExpression, ConditionalBranches};
use stillwind_semantic::{AssetKind, CheckedValue, ExportedValue, IdentityId};

use crate::state::MacroExtractor;

/// The reason an expression cannot supply declaration identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResolutionFailure {
    /// The value is known and cannot supply the requested asset family. Every branch of
    /// a conditional expression has this result.
    Unsupported,
    /// The checker has not provided usable, finite candidates for at least one value.
    MissingFacts,
}

impl<'a> MacroExtractor<'a> {
    /// Reports a failed resolution of `expression`.
    ///
    /// `Unsupported` adds the `unsupported` lint, and the caller lowers the input as
    /// empty or false. `MissingFacts` adds the fatal `missing` diagnostic because
    /// lowering cannot choose a runtime form.
    pub(crate) fn report_resolution_failure(
        &mut self,
        expression: &Expression<'a>,
        failure: ResolutionFailure,
        unsupported: LintDiagnostic,
        missing: CompileDiagnostic,
    ) {
        if failure == ResolutionFailure::MissingFacts {
            self.diagnostics.push(CompileError::new(
                missing,
                self.source_span(expression.span()),
            ));
        } else {
            self.lint_findings.push(LintFinding::new(
                unsupported,
                self.source_span(expression.span()),
            ));
        }
    }

    /// Adds `unsupported` at each conditional branch of `expression` that cannot supply
    /// `kind`. Call this after the whole expression resolved successfully.
    ///
    /// Literal `null`, `undefined`, and `false` branches add no finding because they
    /// select no value, as they do in other style inputs.
    pub(crate) fn report_unsupported_branches(
        &mut self,
        expression: &Expression<'a>,
        kind: AssetKind,
        unsupported: &LintDiagnostic,
    ) {
        let branches = match expression.get_inner_expression() {
            Expression::ConditionalExpression(conditional) => {
                [&conditional.consequent, &conditional.alternate]
            }
            Expression::LogicalExpression(logical) if logical.operator == LogicalOperator::And => {
                self.report_unsupported_branches(&logical.right, kind, unsupported);
                return;
            }
            _ => return,
        };
        for branch in branches {
            match self.asset_candidates(branch, kind) {
                Ok(_) => {
                    self.report_unsupported_branches(branch, kind, unsupported);
                }
                Err(ResolutionFailure::Unsupported) if !selects_no_value(branch) => {
                    self.lint_findings.push(LintFinding::new(
                        unsupported.clone(),
                        self.source_span(branch.span()),
                    ));
                }
                Err(_) => {}
            }
        }
    }

    /// Returns whether a use on `receiver` must lower each conditional branch
    /// separately.
    ///
    /// A runtime lookup reads the receiver value, so it is valid only when every branch
    /// supplies `kind`. A `&&` receiver, a branch that selects no value, and an
    /// unsupported branch require [`Self::lower_receiver_branches`]. A receiver that is
    /// not `?:` or `&&` never requires it.
    pub(crate) fn needs_branch_lowering(&self, receiver: &Expression<'a>, kind: AssetKind) -> bool {
        match receiver.get_inner_expression() {
            Expression::ConditionalExpression(conditional) => {
                [&conditional.consequent, &conditional.alternate]
                    .into_iter()
                    .any(|branch| {
                        self.needs_branch_lowering(branch, kind)
                            || self.asset_candidates(branch, kind).is_err()
                    })
            }
            Expression::LogicalExpression(logical) => logical.operator == LogicalOperator::And,
            _ => false,
        }
    }

    /// Lowers a use separately for each branch of a conditional `receiver`.
    ///
    /// A `?:` receiver becomes a ternary and a `&&` receiver becomes a logical AND, so
    /// the runtime evaluates only the use of the selected branch. Conditional branches
    /// expand in the same way. `lower` receives every other receiver and returns its
    /// contribution, or `None` for an empty contribution. Literal `null`, `undefined`,
    /// and `false` branches contribute nothing without calling `lower`.
    ///
    /// Returns `None` when every branch is empty.
    pub(crate) fn lower_receiver_branches(
        &mut self,
        receiver: &Expression<'a>,
        lower: &mut dyn FnMut(&mut Self, &Expression<'a>) -> Option<ClassExpression>,
    ) -> Option<ClassExpression> {
        let receiver = receiver.get_inner_expression();
        let (test, branches) = match receiver {
            Expression::ConditionalExpression(conditional) => (
                conditional.test.span(),
                ConditionalBranches::Ternary {
                    consequent: self.lower_receiver_branch(&conditional.consequent, lower),
                    alternate: self.lower_receiver_branch(&conditional.alternate, lower),
                },
            ),
            Expression::LogicalExpression(logical) if logical.operator == LogicalOperator::And => (
                logical.left.span(),
                ConditionalBranches::And {
                    consequent: self.lower_receiver_branch(&logical.right, lower),
                },
            ),
            _ => return lower(self, receiver),
        };
        let empty = branches.values().all(Vec::is_empty);
        (!empty).then(|| ClassExpression::Conditional {
            test: self.source_span(test),
            branches,
        })
    }

    /// Lowers one branch for [`Self::lower_receiver_branches`].
    fn lower_receiver_branch(
        &mut self,
        branch: &Expression<'a>,
        lower: &mut dyn FnMut(&mut Self, &Expression<'a>) -> Option<ClassExpression>,
    ) -> Vec<ClassExpression> {
        if selects_no_value(branch) {
            return Vec::new();
        }
        self.lower_receiver_branches(branch, lower)
            .into_iter()
            .collect()
    }

    /// Resolves finite declaration identities for `kind`. Checker facts take precedence
    /// over local values.
    ///
    /// Conditional branches resolve independently. The result contains the candidates
    /// of supported branches, so an unsupported branch selects no declaration. The
    /// right operand supplies the candidates of `&&`.
    ///
    /// Returns `Unsupported` for syntax that cannot carry an asset, checked non-assets,
    /// a known different asset family, or a conditional whose branches are all
    /// unsupported. Returns `MissingFacts` when any branch has absent facts or
    /// nonfinite candidates.
    pub(crate) fn asset_candidates(
        &self,
        expression: &Expression<'a>,
        kind: AssetKind,
    ) -> Result<Vec<IdentityId>, ResolutionFailure> {
        let expression = expression.get_inner_expression();
        self.record_type_probe(expression);
        if let Some(fact) = self.type_fact(expression.span()) {
            return match fact {
                CheckedValue::NonAsset => Err(ResolutionFailure::Unsupported),
                CheckedValue::Asset(value) => candidates(value, kind),
            };
        }
        if let Expression::ConditionalExpression(conditional) = expression {
            // Resolve both branches before returning so that each branch records its probe.
            let branches = [
                self.asset_candidates(&conditional.consequent, kind),
                self.asset_candidates(&conditional.alternate, kind),
            ];
            let mut resolved = None::<Vec<IdentityId>>;
            for branch in branches {
                match branch {
                    Ok(values) => {
                        resolved.get_or_insert_default().extend(values);
                    }
                    Err(ResolutionFailure::Unsupported) => {}
                    Err(ResolutionFailure::MissingFacts) => {
                        return Err(ResolutionFailure::MissingFacts);
                    }
                }
            }
            let mut resolved = resolved.ok_or(ResolutionFailure::Unsupported)?;
            resolved.sort();
            resolved.dedup();
            return Ok(resolved);
        }
        if let Expression::LogicalExpression(logical) = expression
            && logical.operator == LogicalOperator::And
        {
            return self.asset_candidates(&logical.right, kind);
        }
        if let Expression::Identifier(identifier) = expression {
            let value = identifier
                .reference_id
                .get()
                .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
                .and_then(|symbol| self.local_values.get(&symbol));
            return value.map_or(Err(ResolutionFailure::MissingFacts), |value| {
                candidates(value, kind)
            });
        }
        if cannot_carry_asset(expression) {
            Err(ResolutionFailure::Unsupported)
        } else {
            Err(ResolutionFailure::MissingFacts)
        }
    }
}

/// Returns whether the syntax of `expression` proves that its value is not a Stillwind
/// asset, so that it needs no checker fact.
pub(crate) fn cannot_carry_asset(expression: &Expression<'_>) -> bool {
    matches!(
        expression.get_inner_expression(),
        Expression::StringLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::BigIntLiteral(_)
            | Expression::RegExpLiteral(_)
            | Expression::TemplateLiteral(_)
            | Expression::ObjectExpression(_)
            | Expression::ArrayExpression(_)
            | Expression::ArrowFunctionExpression(_)
            | Expression::FunctionExpression(_)
            | Expression::UnaryExpression(_)
            | Expression::UpdateExpression(_)
            | Expression::BinaryExpression(_)
    )
}

/// Returns whether `expression` is a literal that selects no value in a branch.
pub(crate) fn selects_no_value(expression: &Expression<'_>) -> bool {
    match expression.get_inner_expression() {
        Expression::NullLiteral(_) => true,
        Expression::BooleanLiteral(value) => !value.value,
        Expression::Identifier(identifier) => identifier.name == "undefined",
        _ => false,
    }
}

/// Returns the finite candidates of `value` when it belongs to `expected`.
///
/// The asset family is compared first. A known different family is unsupported even
/// when its candidates are not finite.
fn candidates(
    value: &ExportedValue,
    expected: AssetKind,
) -> Result<Vec<IdentityId>, ResolutionFailure> {
    if value.kind() != expected {
        return Err(ResolutionFailure::Unsupported);
    }
    let (ExportedValue::Styles(symbolic)
    | ExportedValue::Refs(symbolic)
    | ExportedValue::Keyframes(symbolic)
    | ExportedValue::ViewTransitions(symbolic)) = value;
    let values = symbolic
        .finite_candidates()
        .ok_or(ResolutionFailure::MissingFacts)?;
    Ok(values.iter().cloned().collect())
}
