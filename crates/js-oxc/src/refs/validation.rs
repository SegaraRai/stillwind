//! Checks marker attachments required by ref providers in each style call.
//!
//! `validate_ref_attachments` runs after traversal and matches uses by declaration
//! identity within one call. Composition can supply an attachment that extraction
//! cannot inspect, so calls with composition do not report a missing marker.
//!
//! These checks add lint findings. Extraction reports fatal syntax and identity
//! errors separately, and lint suppression does not affect those errors.

use stillwind_diagnostics::{LintDiagnostic, LintFinding};
use stillwind_ir::{ConditionalBranches, StyleInput};

use super::RefUseKind;
use crate::state::MacroExtractor;

impl MacroExtractor<'_> {
    /// Adds a finding for each provider whose style call supplies no matching marker.
    /// Composition makes the attachment uncertain and suppresses the finding. Uses
    /// match by declaration identity; conditional alternatives may supply the marker.
    /// Calling this function again adds the findings again.
    pub(crate) fn validate_ref_attachments(&mut self) {
        for provider in self
            .ref_uses
            .iter()
            .filter(|use_| use_.projection == RefUseKind::Provider)
        {
            let Some(call) = self.style_calls.iter().find(|call| {
                call.span.start <= provider.span.start && call.span.end >= provider.span.end
            }) else {
                continue;
            };
            let attached = self.ref_uses.iter().any(|use_| {
                use_.projection == RefUseKind::Marker
                    && use_.identity == provider.identity
                    && call.span.start <= use_.span.start
                    && call.span.end >= use_.span.end
            });
            if !attached && !call.inputs.iter().any(has_composition) {
                self.lint_findings.push(LintFinding::new(
                    LintDiagnostic::MissingRefAttachment {
                        name: provider.name.clone(),
                    },
                    provider.span,
                ));
            }
        }
    }
}

/// Returns whether `input` contains composition, including inside arrays and either
/// conditional branch. It does not predict which branch will run.
///
/// If composition is present, this call alone cannot prove a missing attachment.
fn has_composition(input: &StyleInput) -> bool {
    match input {
        StyleInput::Composition { .. } => true,
        StyleInput::Array(inputs) => inputs.iter().any(has_composition),
        StyleInput::Conditional { branches, .. } => match branches {
            ConditionalBranches::And { consequent } => consequent.iter().any(has_composition),
            ConditionalBranches::Ternary {
                consequent,
                alternate,
            } => consequent.iter().chain(alternate).any(has_composition),
        },
        _ => false,
    }
}
