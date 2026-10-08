//! Animation and view-transition compilation.
//!
//! Pins keyframe emission for local `@utility` frames, selector-scoped keyframes uses, keyed view
//! transitions, the independent `--sw-` and `--tw-` prefix configuration, and the rejection of a
//! second view transition in one call.
//!
//! `declarations` checks animation declarations, labels, and compatibility prefixes.
//! `transition_rules` checks transition phases, conditions, asset uses, and composition.
//! `transition_runtime` checks guarded transitions and runtime participant names.

use stillwind_compiler::{CompilerOptions, SourceTransform};
use stillwind_css::fold_calc_expressions;
use stillwind_diagnostics::{CompileDiagnostic, LintDiagnostic, LintKind, LintSeverity};
use stillwind_ir::{ScriptLanguage, ScriptModuleKind};
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_source::{CssSourceName, ModuleId};
use stillwind_test_support::{CssAssertions as _, css_source};

use super::*;

mod declarations;
mod transition_rules;
mod transition_runtime;
