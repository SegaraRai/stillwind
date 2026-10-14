//! Resolves asset candidates and preserves why an expression cannot be lowered.

use oxc::ast::ast::Expression;
use oxc::span::GetSpan;
use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintDiagnostic, LintFinding};
use stillwind_semantic::{AssetKind, CheckedValue, ExportedValue, IdentityId};

use crate::state::MacroExtractor;

/// A proven unsupported value or information still needed to identify candidates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResolutionFailure {
    /// The value is known and cannot supply the requested asset family.
    Unsupported,
    /// The checker has not provided usable, finite candidates.
    MissingFacts,
}

impl<'a> MacroExtractor<'a> {
    /// Reports an unsupported receiver when its value is known, or missing checker
    /// facts when a nonlocal expression still needs identity resolution.
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

    /// Resolves finite declaration identities for `kind`. Checker facts take precedence
    /// over local values. Conditional branches must both supply the requested kind.
    ///
    /// Returns `Unsupported` for syntax that cannot carry an asset, checked non-assets,
    /// or a known different asset family.
    /// Returns `MissingFacts` for absent facts or nonfinite candidates. A selection
    /// containing supported and unsupported branches also needs more information.
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
            return match (
                self.asset_candidates(&conditional.consequent, kind),
                self.asset_candidates(&conditional.alternate, kind),
            ) {
                (Ok(mut left), Ok(right)) => {
                    left.extend(right);
                    left.sort();
                    left.dedup();
                    Ok(left)
                }
                (Err(ResolutionFailure::Unsupported), Err(ResolutionFailure::Unsupported)) => {
                    Err(ResolutionFailure::Unsupported)
                }
                _ => Err(ResolutionFailure::MissingFacts),
            };
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
        match expression {
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
            | Expression::BinaryExpression(_) => Err(ResolutionFailure::Unsupported),
            _ => Err(ResolutionFailure::MissingFacts),
        }
    }
}

fn candidates(
    value: &ExportedValue,
    expected: AssetKind,
) -> Result<Vec<IdentityId>, ResolutionFailure> {
    let (ExportedValue::Styles(symbolic)
    | ExportedValue::Refs(symbolic)
    | ExportedValue::Keyframes(symbolic)
    | ExportedValue::ViewTransitions(symbolic)) = value;
    let values = symbolic
        .finite_candidates()
        .ok_or(ResolutionFailure::MissingFacts)?;
    if value.kind() != expected {
        return Err(ResolutionFailure::Unsupported);
    }
    Ok(values.iter().cloned().collect())
}
