//! Structure-preserving style emission.
//!
//! [`CallEmitter::run`] resolves style inputs into class units and typed contributions
//! for fragment folding. It retains guards, source occurrences; subtrees with no
//! contribution are omitted under the macro purity contract. It does not choose final
//! framework keys or allocate runtime helper imports.
//!
//! [`ClassUnits`](super::class_units::ClassUnits) owns class identities and
//! base-before-conditional CSS publication.
//! [`LiteralContribution`](super::literal::LiteralContribution) resolves utility text, and
//! fragment rendering later chooses a representation without changing those CSS identities
//! or declaration order.
//!
//! [`CallEmitter`] declares the walk state here so every file in this directory can read
//! the private fields while the rest of the compiler cannot. `call` runs the walk and
//! dispatches one style input; `class_value` lowers class expressions; `ref_projection`,
//! `animation_use`, and `view_transition` hold the arms of one contribution kind each.

mod animation_use;
mod call;
mod class_value;
mod ref_projection;
mod view_transition;

use std::collections::BTreeSet;

use stillwind_design_system::ThemeIndex;
use stillwind_diagnostics::{CompileError, LintFinding};
use stillwind_ir::StyleCall;
use stillwind_semantic::IdentityId;
use stillwind_source::SourceFile;

use super::CssContribution;
use super::class_units::ClassUnits;
use super::props::StyleEmissionPart;
use crate::identifiers::IdentifierContext;
use crate::{CompilerOptions, GeneratedSymbol};

/// Object key under which a ref or scope value carries its canonical class token.
pub(super) const CLASS_KEY: &str = "c";

/// Object key under which a view-transition value carries the class token that marks
/// its participants.
pub(super) const VIEW_TRANSITION_CLASS_KEY: &str = "v";

/// Object key under which a keyed view-transition value carries the key its
/// participants pair under.
pub(super) const VIEW_TRANSITION_KEY: &str = "k";

/// One `style()` call being lowered.
///
/// The emitter accumulates state rather than building a tree in one pass, because the base unit
/// has to be collected across argument positions while each conditional leaf needs its own class
/// unit and guard path.
pub(super) struct CallEmitter<'a> {
    /// The buffer the module's original text is sliced from.
    source: &'a SourceFile,
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
    /// Identities the module references at runtime; the pipeline uses this set to decide which
    /// declarations are replaced by a runtime call and which are erased.
    runtime_assets: &'a mut BTreeSet<IdentityId>,
    /// Definition identities whose CSS this module must emit; every use, static or dynamic,
    /// Definition identities whose CSS this module must emit; every use, static or dynamic,
    /// and every `.for` application records here.
    required_definitions: &'a mut BTreeSet<IdentityId>,
    /// Runtime contributions accumulated for source-position ordering during rendering.
    parts: Vec<StyleEmissionPart>,
    /// Owns class allocation and base-before-conditional CSS publication.
    units: ClassUnits<'a>,
    /// Anchor and container names this call wrote, reported with the class symbols.
    written_name_symbols: Vec<GeneratedSymbol>,
    /// Whether the call marks participants with a view-transition class.
    view_transition_classes: bool,
    /// How many sources of a view-transition-name the call states.
    view_transition_names: usize,
}

/// Source, analysis, and shared output destinations for lowering one style call.
/// Spans in the call must address `source`; destinations are appended
/// during lowering and are not individually rolled back on failure.
pub(super) struct CallEmitterInputs<'a> {
    /// Original buffer used to slice user expressions.
    pub source: &'a SourceFile,
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
    /// Previously emitted (class name, canonical input) pairs used to suppress identical units.
    pub emitted_utility_classes: &'a mut BTreeSet<(String, String)>,
    /// Referenced runtime identities used to retain declarations that need runtime values.
    pub runtime_assets: &'a mut BTreeSet<IdentityId>,
    /// Definition identities whose CSS the module must emit, recorded by every use and by
    /// Definition identities whose CSS the module must emit, recorded by every use and by
    /// `.for` applications.
    pub required_definitions: &'a mut BTreeSet<IdentityId>,
}
