//! Records resolved contributions while their source uses and conditions are available.

use cssparser::{Parser, Token};

use stillwind_css::{declaration_ranges, selector_is_element_condition};
use stillwind_ir::ConditionalBranches;
use stillwind_semantic::IdentityId;
use stillwind_source::Span;

use super::super::literal::LiteralContribution;
use super::super::props::GuardStep;
use super::super::utilities::render::render_utility_rule;
use super::CallEmitter;
use super::ref_projection::rule_contribution;
use crate::style_analysis::{
    DeclarationValue, RuleContext, RuleSubject, StyleEffect, StyleSupply, SupplyKind, UseId,
};

impl CallEmitter<'_> {
    /// Records expanded declarations before contributions are merged into class units.
    /// `gate` identifies a provider's required marker. `choice` selects a finite receiver.
    pub(super) fn record_properties(
        &mut self,
        contribution: &LiteralContribution,
        provider: bool,
        span: Span,
        guard: &[GuardStep],
        gate: Option<&IdentityId>,
        choice: Option<(usize, usize)>,
    ) {
        for (ordinal, utility) in contribution.utilities.iter().enumerate() {
            let mut supplies = Vec::new();
            let rendered = render_utility_rule(
                "&",
                utility,
                self.theme,
                &utility.declaration,
                None,
                self.identifiers,
            );
            match rendered {
                Ok(css) => collect_declarations(&css, "", &[], provider, gate, &mut supplies),
                Err(kind) => self.reject_name(kind, span),
            }
            for nested in &utility.nested {
                match render_utility_rule(
                    "&",
                    utility,
                    self.theme,
                    &nested.declaration,
                    Some(nested),
                    self.identifiers,
                ) {
                    Ok(css) => collect_declarations(&css, "", &[], provider, gate, &mut supplies),
                    Err(kind) => self.reject_name(kind, span),
                }
            }
            self.record_supplies_at(span, guard, supplies, choice, ordinal);
        }
    }

    /// Adds supplies under the source guards and optional runtime candidate selection.
    pub(super) fn record_supplies(
        &mut self,
        span: Span,
        guard: &[GuardStep],
        supplies: Vec<SupplyKind>,
        choice: Option<(usize, usize)>,
    ) {
        self.record_supplies_at(span, guard, supplies, choice, 0);
    }

    fn record_supplies_at(
        &mut self,
        span: Span,
        guard: &[GuardStep],
        supplies: Vec<SupplyKind>,
        choice: Option<(usize, usize)>,
        ordinal: usize,
    ) {
        let site = UseId {
            module: self.module_id.clone(),
            span,
        };
        let mut node = StyleEffect::All(
            supplies
                .into_iter()
                .map(|kind| {
                    StyleEffect::Supply(StyleSupply {
                        site: site.clone(),
                        kind,
                        ordinal,
                    })
                })
                .collect(),
        );
        if let Some((index, count)) = choice {
            let mut alternatives = vec![StyleEffect::All(Vec::new()); count];
            alternatives[index] = node;
            node = StyleEffect::Choice {
                site: site.clone(),
                alternatives,
            };
        }
        self.record_effect(node, guard);
    }

    /// Appends an effect with the enclosing source conditions, without enumerating outcomes.
    pub(super) fn record_effect(&mut self, mut node: StyleEffect, guard: &[GuardStep]) {
        for step in guard.iter().rev() {
            let branches = if step.when_true {
                ConditionalBranches::And {
                    consequent: Box::new(node),
                }
            } else {
                ConditionalBranches::Ternary {
                    consequent: Box::new(StyleEffect::All(Vec::new())),
                    alternate: Box::new(node),
                }
            };
            node = StyleEffect::Branch {
                condition: UseId {
                    module: self.module_id.clone(),
                    span: step.test,
                },
                branches,
            };
        }
        self.style_effects.push(node);
    }

    /// Records transition participation and its name in the resolved CSS context.
    pub(super) fn record_transition(
        &mut self,
        identity: &IdentityId,
        span: Span,
        guard: &[GuardStep],
        contribution: &LiteralContribution,
        name: Option<&DeclarationValue>,
        choice: Option<(usize, usize)>,
    ) {
        let mut declarations = Vec::new();
        if let Some(plan) = contribution
            .view_transitions
            .first()
            .and_then(|transition| transition.plan.as_ref())
        {
            let probe = rule_contribution(plan, "view-transition-class:initial;".into(), None);
            for utility in &probe.utilities {
                if let Ok(css) = render_utility_rule(
                    "&",
                    utility,
                    self.theme,
                    &utility.declaration,
                    None,
                    self.identifiers,
                ) {
                    collect_declarations(&css, "", &[], false, None, &mut declarations);
                }
            }
        } else {
            declarations.push(SupplyKind::Declaration {
                property: "view-transition-class".into(),
                value: DeclarationValue::Css("initial".into()),
                important: false,
                context: RuleContext {
                    selector: "&".into(),
                    at_rules: Vec::new(),
                    subject: RuleSubject::Target,
                },
                provider: false,
                gate: None,
            });
        }
        let mut supplies = Vec::new();
        for declaration in declarations {
            if let SupplyKind::Declaration { context, .. } = declaration {
                supplies.push(SupplyKind::Transition {
                    identity: identity.clone(),
                    context: context.clone(),
                });
                if let Some(value) = name {
                    supplies.push(SupplyKind::Declaration {
                        property: "view-transition-name".into(),
                        value: value.clone(),
                        important: false,
                        context,
                        provider: false,
                        gate: None,
                    });
                }
            }
        }
        self.record_supplies(span, guard, supplies, choice);
    }
}

fn collect_declarations(
    css: &str,
    selector: &str,
    at_rules: &[String],
    provider: bool,
    gate: Option<&IdentityId>,
    out: &mut Vec<SupplyKind>,
) {
    for range in declaration_ranges(css) {
        let text = css[range].trim();
        if let Some(open) = rule_open(text) {
            let prelude = text[..open].trim();
            let body = text[open + 1..]
                .trim_end()
                .strip_suffix('}')
                .unwrap_or(&text[open + 1..]);
            if prelude.starts_with('@') {
                let mut conditions = at_rules.to_vec();
                conditions.push(prelude.into());
                collect_declarations(body, selector, &conditions, provider, gate, out);
            } else {
                let expanded = if selector.is_empty() {
                    prelude.into()
                } else {
                    stillwind_css::instantiate_selector(prelude, selector)
                };
                collect_declarations(body, &expanded, at_rules, provider, gate, out);
            }
            continue;
        }
        let mut parser = Parser::new(text);
        let Ok(property) = parser.expect_ident().map(ToString::to_string) else {
            continue;
        };
        if parser.expect_colon().is_err() {
            continue;
        }
        let value = text[parser.position().byte_index()..]
            .trim()
            .trim_end_matches(';')
            .trim()
            .to_owned();
        let subject = subject(selector);
        out.push(SupplyKind::Declaration {
            property: if property.starts_with("--") {
                property
            } else {
                property.to_ascii_lowercase()
            },
            important: value
                .rsplit_once('!')
                .is_some_and(|(_, priority)| priority.trim().eq_ignore_ascii_case("important")),
            value: DeclarationValue::Css(value),
            context: RuleContext {
                selector: selector.into(),
                at_rules: at_rules.to_vec(),
                subject,
            },
            provider,
            gate: gate.cloned(),
        });
    }
}

/// Finds a rule block while skipping strings and functional declaration values.
fn rule_open(text: &str) -> Option<usize> {
    let mut parser = Parser::new(text);
    if parser
        .expect_ident()
        .is_ok_and(|name| name.starts_with("--"))
        && parser.expect_colon().is_ok()
    {
        return None;
    }
    let mut parser = Parser::new(text);
    while let Ok(token) = parser.next_including_whitespace_and_comments() {
        if matches!(token, Token::CurlyBracketBlock) {
            return Some(parser.position().byte_index() - 1);
        }
    }
    None
}

/// Identifies the final compound's nesting subject. Functional selectors whose
/// subject is hidden inside an argument remain unknown; pseudo-elements are other subjects.
fn subject(selector: &str) -> RuleSubject {
    if !selector_is_element_condition(selector, false) {
        return RuleSubject::Other;
    }
    let mut parser = Parser::new(selector);
    let mut direct_subject = false;
    let mut target = false;
    let mut separated = false;
    let mut outcomes = Vec::new();
    while let Ok(token) = parser.next_including_whitespace_and_comments() {
        match token {
            Token::WhiteSpace(_) => separated = true,
            Token::Delim('>' | '+' | '~' | '|') => {
                target = false;
                separated = false;
            }
            Token::Comma => {
                outcomes.push(target);
                target = false;
                separated = false;
            }
            Token::Comment(_) => {}
            token => {
                if separated {
                    target = false;
                    separated = false;
                }
                if matches!(token, Token::Delim('&')) {
                    target = true;
                    direct_subject = true;
                }
                if matches!(
                    token,
                    Token::Function(_) | Token::SquareBracketBlock | Token::ParenthesisBlock
                ) {
                    let _: Result<(), cssparser::ParseError<()>> =
                        parser.parse_nested_block(|inner| {
                            while inner.next_including_whitespace_and_comments().is_ok() {}
                            Ok(())
                        });
                }
            }
        }
    }
    outcomes.push(target);
    if outcomes.iter().all(|&target| target) {
        RuleSubject::Target
    } else if outcomes.iter().all(|&target| !target) && direct_subject {
        RuleSubject::Other
    } else {
        RuleSubject::Unknown
    }
}
