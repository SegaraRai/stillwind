//! Records contributions before class-unit merging removes their source boundaries.
//!
//! `CallEmitter` records declarations from the same resolved utilities used for CSS
//! emission, together with source guards, finite receiver choices, and ref
//! requirements. The collector reads rendered utility fragments; it never reads
//! published module CSS. Selector classification stays conservative when the subject
//! occurs inside a function.

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
    /// Appends declarations from every resolved utility and nested rule in
    /// `contribution`. `span` and `guard` must refer to the original module source.
    /// `provider` marks ref-provider or anchor-scope writes, and `gate` identifies a
    /// required ref marker. `choice` is a zero-based candidate index and the total
    /// candidate count; all records for the same receiver span must share that count
    /// and candidate order. Utilities retain their ordinal so CSS alternatives share
    /// one name-supply origin. Rendering failures append diagnostics; callers must
    /// discard failed compilation.
    ///
    /// # Panics
    ///
    /// Panics if a candidate index is not smaller than its count.
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

    /// Appends `supplies` as contributions of ordinal zero at the original `span`.
    /// `guard` lists enclosing source tests, and `choice` optionally selects one finite
    /// receiver. The candidate index must be smaller than the shared candidate count.
    /// The method preserves all enclosing conditions and changes only the effect
    /// records.
    ///
    /// # Panics
    ///
    /// Panics if a candidate index is not smaller than its count.
    pub(super) fn record_supplies(
        &mut self,
        span: Span,
        guard: &[GuardStep],
        supplies: Vec<SupplyKind>,
        choice: Option<(usize, usize)>,
    ) {
        self.record_supplies_at(span, guard, supplies, choice, 0);
    }

    /// Appends supplies with their source occurrence and resolved utility `ordinal`.
    /// `choice` must use a valid index and a count consistent with the other records
    /// at `span`. Unselected candidates supply nothing. `guard` encloses the whole
    /// choice.
    ///
    /// # Panics
    ///
    /// Panics if a candidate index is not smaller than its count.
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

    /// Appends `node` wrapped in every source test in `guard`, outermost test first.
    /// Guard spans must address the original module source. Falsy uses receive an
    /// empty truthy arm; truthy uses have no contribution on the falsy path. Repeated
    /// test occurrences retain their identity so the resolver can compare decisions.
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

    /// Appends participation in `identity` and an optional name at the original `span`.
    /// `contribution` must be the resolved contribution for this transition use;
    /// its first transition plan supplies the emitted selector and at-rule contexts.
    /// `name` contains the escaped CSS value or original runtime expression. An absent
    /// name records participation alone. Contexts that fail to render are skipped;
    /// ordinary emission diagnoses the same resolved plan. `guard` and `choice` have
    /// the source and candidate contracts of `record_supplies`.
    ///
    /// # Panics
    ///
    /// Panics if a candidate index is not smaller than its count.
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

/// Appends declaration supplies from a rendered utility fragment in `css`.
/// `selector` is the current expanded selector, or empty before the first rule.
/// `at_rules` contains enclosing headers in outermost-first order. Nested selectors
/// substitute that selector for `&`. Provider status and marker `gate` pass through.
/// Decoded ordinary property names become lowercase; custom names retain their case.
/// Malformed declarations are skipped. Values keep their CSS priority suffix.
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

/// Returns the byte offset of the first top-level rule block opening in `text`.
/// Strings and functional values are skipped. Custom-property declarations return
/// `None` because their values may contain brace blocks. The caller supplies one
/// complete declaration or rule fragment from `declaration_ranges`.
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

/// Classifies the element receiving a rendered selector's declarations.
/// `selector` uses `&` for the styled element. Every comma-separated alternative
/// must end in a direct `&` compound to return `Target`. Descendant targets and
/// pseudo-elements return `Other`. Mixed alternatives or subjects hidden inside
/// functional arguments remain `Unknown`; the function does not prove CSS equivalence.
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

#[cfg(test)]
mod tests {
    use stillwind_semantic::AssetKind;
    use stillwind_source::ModuleId;

    use super::*;

    #[test]
    fn rule_blocks_skip_strings_functions_and_custom_property_values() {
        assert_eq!(rule_open(r#"content: "{";"#), None);
        assert_eq!(rule_open(r#"background: url("{image}");"#), None);
        assert_eq!(rule_open("--blocks: { nested: value };"), None);
        let rule = r#"&[data-value="{"] { color: red; }"#;
        assert_eq!(rule_open(rule), rule.find(" { ").map(|offset| offset + 1));
        assert!(rule_open("@media (width > 10px) { & { color:red; } }").is_some());
    }

    #[test]
    fn declaration_collection_retains_nested_contexts_case_values_and_provider_gates() {
        let gate = IdentityId::new(AssetKind::Ref, ModuleId::from("root"), 0, 10, "r");
        let mut supplies = Vec::new();
        collect_declarations(
            "@media (width > 10px) { & { COLOR:red ! IMPORTANT; --Theme:{x:y}; &:hover { padding:1px; } } }",
            "",
            &[],
            true,
            Some(&gate),
            &mut supplies,
        );
        assert_eq!(supplies.len(), 3);
        let SupplyKind::Declaration {
            property,
            value,
            important,
            context,
            provider,
            gate: actual_gate,
        } = &supplies[0]
        else {
            panic!("expected declaration");
        };
        assert_eq!(property, "color");
        assert_eq!(value, &DeclarationValue::Css("red ! IMPORTANT".into()));
        assert!(*important);
        assert!(*provider);
        assert_eq!(actual_gate.as_ref(), Some(&gate));
        assert_eq!(context.selector, "&");
        assert_eq!(context.at_rules, vec!["@media (width > 10px)"]);
        assert!(
            matches!(&supplies[1], SupplyKind::Declaration { property, value, .. } if property == "--Theme" && value == &DeclarationValue::Css("{x:y}".into()))
        );
        assert!(
            matches!(&supplies[2], SupplyKind::Declaration { context, .. } if context.selector == "&:hover" && context.at_rules == vec!["@media (width > 10px)"])
        );
    }

    #[test]
    fn declaration_collection_decodes_names_and_skips_malformed_entries() {
        let mut supplies = Vec::new();
        collect_declarations(
            r"& { broken; :bad; c\6flor:blue; }",
            "",
            &[],
            false,
            None,
            &mut supplies,
        );
        assert_eq!(supplies.len(), 1);
        assert!(
            matches!(&supplies[0], SupplyKind::Declaration { property, important: false, context, .. } if property == "color" && context.subject == RuleSubject::Target)
        );
    }

    #[test]
    fn selector_subjects_distinguish_targets_descendants_pseudo_elements_and_uncertainty() {
        for (selector, expected) in [
            ("&", RuleSubject::Target),
            (".enabled &:hover", RuleSubject::Target),
            ("&:where(.marker)", RuleSubject::Target),
            ("&:hover, &:active", RuleSubject::Target),
            ("& > span", RuleSubject::Other),
            ("& + span", RuleSubject::Other),
            ("&::before", RuleSubject::Other),
            ("&:hover, & > span", RuleSubject::Unknown),
            (":is(&, span)", RuleSubject::Unknown),
        ] {
            assert_eq!(subject(selector), expected, "{selector}");
        }
    }
}
