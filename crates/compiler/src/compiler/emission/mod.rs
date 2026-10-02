//! Structure-preserving style emission.
//!
//! [`CallEmitter::run`] resolves style inputs into class units and typed contributions
//! for fragment folding. It retains guards and source occurrences. Subtrees with no
//! contribution are omitted under the macro purity contract. It does not choose final
//! framework keys or allocate runtime helper imports.
//!
//! [`ClassUnits`] owns class identities and base-before-conditional CSS publication.
//! [`LiteralContribution`] resolves utility text,
//! and fragment rendering later chooses a representation without changing those CSS
//! identities or declaration order.
//!
//! [`CallEmitter`] declares the walk state here so every file in this directory can
//! read the private fields while the rest of the compiler cannot. `call` runs the walk
//! and dispatches one style input; `class_value` lowers class expressions;
//! `ref_projection`, `keyframes_use`, and `view_transition` hold the arms of one
//! contribution kind each. `style_analysis` records declarations, markers, and source
//! decisions before class-unit merging removes their original use boundaries.

mod call;
mod class_value;
mod keyframes_use;
mod ref_projection;
mod style_analysis;
mod view_transition;

use std::collections::BTreeSet;

use stillwind_design_system::ThemeIndex;
use stillwind_diagnostics::{CompileError, LintFinding};
use stillwind_ir::StyleCall;
use stillwind_semantic::IdentityId;
use stillwind_source::{ModuleId, SourceFile};

use super::CssContribution;
use super::class_units::ClassUnits;
use super::literal::LiteralContribution;
use super::props::StyleEmissionPart;
use super::symbols::GeneratedSymbol;
use crate::style_analysis::StyleEffect;
use crate::{CompilerOptions, IdentifierContext};

/// Object key under which a compiled ref record carries its canonical marker token.
pub(super) const CLASS_KEY: &str = "c";

/// Object key under which a keyframes value carries its generated keyframes name.
///
/// The name is what the declaration's `animation-name` states, and a use selects its
/// prebuilt class by reading it.
pub(super) const KEYFRAMES_KEY: &str = "k";

/// Object key under which a view-transition value carries the class token that marks
/// its participants.
pub(super) const VIEW_TRANSITION_CLASS_KEY: &str = "v";

/// One `style()` call being lowered.
///
/// The emitter accumulates state rather than building a tree in one pass, because the
/// base unit spans argument positions while each conditional region owns a generated
/// rule class and guard path.
pub(super) struct CallEmitter<'a> {
    /// The buffer the module's original text is sliced from.
    source: &'a SourceFile,
    /// Module identity used for generated names.
    module_id: &'a ModuleId,
    /// Compiler configuration for generated identities and output.
    options: &'a CompilerOptions,
    /// Name allocation shared with the rest of this compilation.
    identifiers: &'a IdentifierContext,
    /// Resolved theme and utility registry for this compilation.
    theme: &'a ThemeIndex,
    /// Analyzed style call being lowered.
    call: &'a StyleCall,
    /// Compilation errors appended while lowering the call.
    diagnostics: &'a mut Vec<CompileError>,
    /// Nonfatal findings appended in encounter order.
    lint_findings: &'a mut Vec<LintFinding>,
    /// Identities referenced at runtime. The pipeline retains compiled identity
    /// records for these declarations and erases declarations without runtime uses.
    runtime_assets: &'a mut BTreeSet<IdentityId>,
    /// Asset definitions whose CSS this module must emit. Static and dynamic uses
    /// record their possible definitions here, including `.for(...)` inputs.
    required_definitions: &'a mut BTreeSet<IdentityId>,
    /// Runtime contributions accumulated for source-position ordering during rendering.
    parts: Vec<StyleEmissionPart>,
    /// Rule-bearing contributions inside the current JavaScript condition region.
    pending_region: Option<PendingRegion>,
    /// Owns class allocation and base-before-conditional CSS publication.
    units: ClassUnits<'a>,
    /// Anchor and container names this call wrote, reported with the class symbols.
    written_name_symbols: Vec<GeneratedSymbol>,
    /// Direct contributions with their conditions and base references.
    style_effects: Vec<StyleEffect>,
}

/// CSS rules gathered between JavaScript condition boundaries before one class identity
/// is allocated for the region.
struct PendingRegion {
    /// Source traversal order reserved before visiting the region's children.
    order: u32,
    /// Rule-bearing contributions accumulated in encounter order.
    rules: LiteralContribution,
}

/// Source, analysis, and shared output destinations for lowering one style call. Spans
/// in the call must address `source`; destinations are appended during lowering and are
/// not individually rolled back on failure.
pub(super) struct CallEmitterInputs<'a> {
    /// Original buffer used to slice user expressions.
    pub source: &'a SourceFile,
    /// Compiler identity of the source module.
    pub module_id: &'a ModuleId,
    /// Compiler configuration for this call.
    pub options: &'a CompilerOptions,
    /// Name allocation shared with the rest of this compilation.
    pub identifiers: &'a IdentifierContext,
    /// Resolved theme and utility registry.
    pub theme: &'a ThemeIndex,
    /// Analyzed style call to lower.
    pub call: &'a StyleCall,
    /// Shared destination for compilation errors.
    pub diagnostics: &'a mut Vec<CompileError>,
    /// Shared destination for lint findings, appended in encounter order.
    pub lint_findings: &'a mut Vec<LintFinding>,
    /// Shared destination for module CSS contributions.
    pub utility_css: &'a mut Vec<CssContribution>,
    /// Shared destination for generated symbol records.
    pub symbols: &'a mut Vec<GeneratedSymbol>,
    /// Previously emitted (class name, canonical input) pairs used to suppress
    /// identical units.
    pub emitted_utility_classes: &'a mut BTreeSet<(String, String)>,
    /// Referenced runtime identities used to retain declarations that need runtime
    /// values.
    pub runtime_assets: &'a mut BTreeSet<IdentityId>,
    /// Definition identities whose CSS the module must emit, recorded by each use.
    pub required_definitions: &'a mut BTreeSet<IdentityId>,
}
