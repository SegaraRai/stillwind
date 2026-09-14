//! JavaScript materialization of selected style-fragment plans.
//!
//! [`render_style_call`] tries direct framework props, then renders a folded symbolic
//! tree through the merge runtime. Source-owned fragments retain occurrence mappings;
//! synthetic keys and helpers receive the enclosing transform's fallback location.
//! The caller supplies finalized CSS identities and validates the macro contract.

use stillwind_ir::{ConditionalKind, FrameworkSpec, StyleFormat};
use stillwind_source::Span;

use super::super::expression_render::{collect_class_arguments, render_plain};
use super::super::props::StyleEmission;
use super::expression::{Atom, Emitted, Fragment, Property, ValueExpr};
use super::fold::{Demand, Field, fold, known_record, record_fields, static_class};
use crate::generated::{Code, code, js_string};
use crate::runtime::{RuntimeBindings, RuntimeHelper};

/// Returns code for one finalized call, registering helpers only for the selected output.
///
/// Uses direct framework props when the whole record is representable; otherwise
/// folds subtrees and wraps symbolic values in the merge runtime. The input must
/// satisfy the macro value-expression contract, which permits evaluation reordering.
/// All source-owned expressions keep their occurrence mappings.
pub(in super::super) fn render_style_call(
    emission: &StyleEmission,
    fw: &FrameworkSpec,
    runtime: &mut RuntimeBindings<'_>,
) -> Code {
    let source = Fragment::from_emission(emission);
    let mut renderer = Renderer {
        fw,
        runtime,
        span: emission.call_span,
    };
    if let Some(atoms) = known_record(&source)
        && let Some(fields) = record_fields(&atoms, Demand::Props, fw)
    {
        return renderer.record(&atoms, &fields, Demand::Props);
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
                    Some(fields) => self.record(atoms, &fields, Demand::Fragment),
                    None => code![
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
    /// Deferred clsx calls become arguments of the surrounding clsx under the macro
    /// purity contract; guards remain single expressions.
    ///
    /// # Panics
    ///
    /// Panics if any value is not accepted by `render_plain`.
    #[allow(clippy::expect_used)] // Atom::Class is constructed only after render_plain succeeds.
    fn classes(&mut self, values: &[&Emitted]) -> Code {
        let mut arguments = Vec::new();
        for value in values {
            collect_class_arguments(value, &mut arguments);
        }
        let values = arguments;
        if values.is_empty() {
            return js_string("").into();
        }
        let helper = self.runtime.require(RuntimeHelper::Clsx, self.span);
        code![
            helper,
            "(",
            Code::join(
                values
                    .iter()
                    .flat_map(|value| match value {
                        Emitted::Expression(ValueExpr::Call {
                            helper: RuntimeHelper::Clsx,
                            arguments,
                            ..
                        }) => arguments.clone(),
                        _ => vec![render_plain(value, self.runtime).expect("class-only tree")],
                    })
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
