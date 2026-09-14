//! JavaScript materialization of selected style-fragment plans.
//!
//! [`render_style_call`] tries direct framework props, then renders a folded symbolic
//! tree through the merge runtime. Source-owned fragments retain occurrence mappings;
//! synthetic keys and helpers receive the enclosing transform's fallback location.
//! CSS allocation and effect analysis are inputs, not responsibilities of this module.

use stillwind_diagnostics::{Label, LintFinding, LintKind};
use stillwind_ir::{ConditionalKind, FrameworkSpec, StyleFormat};
use stillwind_source::Span;

use super::super::expression_render::render_plain;
use super::super::props::StyleEmission;
use super::expression::{Atom, Emitted, Fragment, Property, ValueExpr};
use super::fold::{Demand, Field, Rejection, fold, known_record, record_fields, static_class};
use crate::generated::{Code, code, js_string};
use crate::runtime::{RuntimeBindings, RuntimeHelper};

/// Returns code for one finalized call, registering helpers only for the selected output.
///
/// Uses direct framework props when the whole record is representable; otherwise
/// folds subtrees and wraps symbolic values in the merge runtime. An obstructed
/// root evaluation order appends one typed lint finding; shape-only rejection does
/// not. Existing findings are retained and lint suppression is applied by the caller.
/// All source-owned expressions keep their occurrence mappings.
pub(in super::super) fn render_style_call(
    emission: &StyleEmission,
    fw: &FrameworkSpec,
    runtime: &mut RuntimeBindings<'_>,
    findings: &mut Vec<LintFinding>,
) -> Code {
    let source = Fragment::from_emission(emission);
    let mut renderer = Renderer {
        fw,
        runtime,
        span: emission.call_span,
    };
    if let Some(atoms) = known_record(&source) {
        match record_fields(&atoms, Demand::Props, fw) {
            Ok(fields) => return renderer.record(&atoms, &fields, Demand::Props),
            Err(Rejection::EvaluationOrder {
                before,
                between,
                after,
            }) => {
                let mut finding =
                    LintFinding::new(LintKind::StyleFoldEvaluationOrder, emission.call_span);
                for (index, message) in [
                    (before, "first contribution"),
                    (between, "intervening evaluation"),
                    (after, "later contribution"),
                ] {
                    if let Atom::Attribute(p) | Atom::Custom(p) = &atoms[index] {
                        finding.labels.push(Label {
                            span: p.span,
                            message: Some(message.into()),
                        });
                    }
                }
                finding.notes.push("Combining these contributions would move an evaluation across another; the fragment runtime preserves their source order.".into());
                findings.push(finding);
            }
            Err(Rejection::Representation) => {}
        }
    }
    let folded = fold(source, fw);
    let arguments = match &folded {
        Fragment::Sequence(children) => children
            .iter()
            .map(|child| renderer.fragment(child))
            .collect::<Vec<_>>(),
        other => vec![renderer.fragment(other)],
    };
    let helper = renderer
        .runtime
        .require(RuntimeHelper::Merge, renderer.span);
    code![helper, "(", Code::join(arguments, ","), ")"]
}

/// Per-call rendering context shared by selected fragment subtrees.
struct Renderer<'a, 'b> {
    /// Final property names, style format, and runtime conventions.
    fw: &'a FrameworkSpec,
    /// Binding allocator mutated when rendered syntax uses a helper.
    runtime: &'a mut RuntimeBindings<'b>,
    /// Original call span used when a synthetic helper has no more specific origin.
    span: Span,
}

impl Renderer<'_, '_> {
    /// Renders a subtree as symbolic fragment input, retaining guards and source mappings.
    /// Empty records become `null`; a record that cannot share fields becomes an array
    /// of singleton contributions. Required helpers are registered while rendering.
    fn fragment(&mut self, fragment: &Fragment) -> Code {
        match fragment {
            Fragment::Record(atoms) => {
                if atoms.is_empty() {
                    return "null".into();
                }
                match record_fields(atoms, Demand::Fragment, self.fw) {
                    Ok(fields) => self.record(atoms, &fields, Demand::Fragment),
                    Err(_) => code![
                        "[",
                        Code::join(
                            atoms
                                .iter()
                                .map(|atom| match atom {
                                    Atom::Attribute(p) => code![
                                        "{",
                                        Self::property(p, Code::source(p.value.clone(), p.span)),
                                        "}"
                                    ],
                                    Atom::Class(_) => self.record(
                                        std::slice::from_ref(atom),
                                        &[Field::Classes(vec![0])],
                                        Demand::Fragment
                                    ),
                                    Atom::Custom(_) => self.record(
                                        std::slice::from_ref(atom),
                                        &[Field::Custom(vec![0])],
                                        Demand::Fragment
                                    ),
                                })
                                .collect::<Vec<_>>(),
                            ","
                        ),
                        "]"
                    ],
                }
            }
            Fragment::Sequence(children) => code![
                "[",
                Code::join(
                    children
                        .iter()
                        .map(|child| self.fragment(child))
                        .collect::<Vec<_>>(),
                    ","
                ),
                "]"
            ],
            Fragment::Branch {
                test,
                kind,
                consequent,
                alternate,
            } => {
                let consequent = self.fragment(consequent);
                match kind {
                    ConditionalKind::LogicalAnd => code!["(", test, ") && (", consequent, ")"],
                    ConditionalKind::Ternary => {
                        code![
                            "(",
                            test,
                            ") ? (",
                            consequent,
                            ") : (",
                            self.fragment(alternate),
                            ")"
                        ]
                    }
                }
            }
            Fragment::FromProps(expression) => {
                let helper = self.runtime.require(RuntimeHelper::ToFragment, self.span);
                code![helper, "((", expression, "))"]
            }
            Fragment::StylePayload(expression) => {
                let key = self.runtime.require(RuntimeHelper::StyleKey, self.span);
                code!["{[", key, "]:", expression.render(self.runtime), "}"]
            }
        }
    }

    /// Renders a selected field layout in fragment or final-props form.
    /// `fields` must refer to matching atom variants from this exact slice and satisfy
    /// the representation proof for `demand` and `fw`. Source mappings are preserved;
    /// required key, class, and value-conversion helpers are registered as used.
    ///
    /// # Panics
    ///
    /// Panics on out-of-range field indices or class subtrees that cannot be rendered
    /// as class values. Callers must use a proven layout or a matching singleton field.
    fn record(&mut self, atoms: &[Atom], fields: &[Field], demand: Demand) -> Code {
        let mut entries = Vec::new();
        for field in fields {
            match field {
                Field::Attribute(i) => {
                    if let Atom::Attribute(p) = &atoms[*i] {
                        entries.push(Self::property(p, Code::source(p.value.clone(), p.span)));
                    }
                }
                Field::Classes(indices) => {
                    let values = indices
                        .iter()
                        .filter_map(|i| match &atoms[*i] {
                            Atom::Class(value) => Some(value),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let value = if let Some(strings) = values
                        .iter()
                        .map(|value| static_class(value))
                        .collect::<Option<Vec<_>>>()
                    {
                        let text = strings
                            .into_iter()
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                            .join(" ");
                        if text.is_empty() {
                            continue;
                        }
                        js_string(&text).into()
                    } else if values.len() == 1 && demand == Demand::Fragment {
                        if let Emitted::Expression(expression) = values[0] {
                            code!["(", expression.render(self.runtime), ")"]
                        } else {
                            self.classes(&values)
                        }
                    } else {
                        self.classes(&values)
                    };
                    let key = if demand == Demand::Props {
                        self.fw.class_property.into()
                    } else {
                        format!(
                            "[{}]",
                            self.runtime.require(RuntimeHelper::ClassKey, self.span)
                        )
                    };
                    entries.push(code![key, ":", value]);
                }
                Field::Custom(indices) => {
                    let properties = indices
                        .iter()
                        .filter_map(|i| match &atoms[*i] {
                            Atom::Custom(p) => Some(p),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let value =
                        if demand == Demand::Props && self.fw.style_format == StyleFormat::String {
                            Code::join(
                                properties
                                    .iter()
                                    .map(|p| {
                                        code![
                                            js_string(&format!("{}:", p.name)),
                                            "+",
                                            self.custom_value(p)
                                        ]
                                    })
                                    .collect::<Vec<_>>(),
                                "+\";\"+",
                            )
                        } else {
                            code![
                                "{",
                                Code::join(
                                    properties
                                        .iter()
                                        .map(|p| Self::property(p, self.custom_value(p)))
                                        .collect::<Vec<_>>(),
                                    ","
                                ),
                                "}"
                            ]
                        };
                    let key = if demand == Demand::Props {
                        "style".into()
                    } else {
                        format!(
                            "[{}]",
                            self.runtime
                                .require(RuntimeHelper::CustomPropertiesKey, self.span)
                        )
                    };
                    entries.push(code![key, ":", value]);
                }
            }
        }
        code!["{", Code::join(entries, ","), "}"]
    }

    /// Joins renderable class values through clsx without changing argument provenance.
    /// A single deferred clsx call surrounded only by static leaves is flattened;
    /// other helper calls retain their own evaluation boundaries.
    ///
    /// # Panics
    ///
    /// Panics if any value is not accepted by `render_plain`.
    #[allow(clippy::expect_used)] // Atom::Class is constructed only after render_plain succeeds.
    fn classes(&mut self, values: &[&Emitted]) -> Code {
        // Only static siblings may move into a source clsx call: then argument
        // evaluation and user-owned value coercion stay at the same boundary.
        let mut source_calls = 0;
        let collapsible = values.iter().all(|value| match value {
            Emitted::Leaf { .. } => true,
            Emitted::Expression(ValueExpr::Call {
                helper: RuntimeHelper::Clsx,
                ..
            }) => {
                source_calls += 1;
                true
            }
            _ => false,
        });
        if collapsible && source_calls == 1 {
            let arguments = values
                .iter()
                .flat_map(|value| match value {
                    Emitted::Leaf { names } => vec![js_string(&names.join(" ")).into()],
                    Emitted::Expression(ValueExpr::Call { arguments, .. }) => arguments.clone(),
                    _ => Vec::new(),
                })
                .collect::<Vec<_>>();
            let helper = self.runtime.require(RuntimeHelper::Clsx, self.span);
            return code![helper, "(", Code::join(arguments, ","), ")"];
        }
        let helper = self.runtime.require(RuntimeHelper::Clsx, self.span);
        code![
            helper,
            "(",
            Code::join(
                values
                    .iter()
                    .map(|v| render_plain(v, self.runtime).expect("class-only tree"))
                    .collect::<Vec<_>>(),
                ","
            ),
            ")"
        ]
    }

    /// Wraps one unconverted property value in the CSS-value helper.
    /// Registers the helper at the property's span and maps only the authored value
    /// exactly; the generated call syntax remains synthetic.
    fn custom_value(&mut self, p: &Property) -> Code {
        let helper = self.runtime.require(RuntimeHelper::CssValue, p.span);
        code![helper, "(", Code::source(p.value.clone(), p.span), ")"]
    }

    /// Pairs the property's quoted name with an already-rendered value.
    /// The name is synthetic; `value` retains its existing source mappings and must
    /// already have any custom-property conversion required by its consumer.
    fn property(p: &Property, value: Code) -> Code {
        code![js_string(&p.name), ":", value]
    }
}

/// Builds runtime materialization without combining adjacent records for comparison tests.
/// Uses the same emission-to-fragment conversion and leaf renderer as optimized code,
/// but always retains the outer merge. Helper bindings are allocated in `runtime`.
#[cfg(test)]
pub(super) fn render_reference(
    emission: &StyleEmission,
    fw: &FrameworkSpec,
    runtime: &mut RuntimeBindings<'_>,
) -> Code {
    let mut renderer = Renderer {
        fw,
        runtime,
        span: emission.call_span,
    };
    let fragment = renderer.fragment(&Fragment::from_emission(emission));
    let merge = renderer
        .runtime
        .require(RuntimeHelper::Merge, emission.call_span);
    code![merge, "(", fragment, ")"]
}
