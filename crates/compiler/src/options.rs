//! Compiler-wide configuration and the `--sw-` / `--tw-` prefix split.
//!
//! [`CompilerOptions`] travels with one compilation. It carries the Stillwind-owned
//! property prefix, the separately configurable Tailwind-compatibility prefix, the
//! caller's class namespace and the runtime-conditional output strategy.
//! [`CompilerOptions::resolved`] pins both prefixes to the snapshot's own
//! configuration, so a caller that passes stale options still compiles against the
//! theme it handed over. Every generated text that carries a prefix is substituted by
//! this module's `rewrite_generated`; the two namespaces must stay distinct, because a
//! consumer that recognizes one must not match the other.

use serde::{Deserialize, Serialize};

use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_diagnostics::{CompileDiagnostic, LintOverrides, LintSettings};

use crate::identifiers::{DEFAULT_HASH_LENGTH, MAXIMUM_HASH_LENGTH};

/// Knobs a host may set per compilation.
///
/// Serialized with camelCase fields and `#[serde(default)]`, so adding a field is
/// compatible with previously written configuration; defaults match the built-in
/// `stillwind` package and an unnamespaced project. Runtime-condition output strategy
/// applies to utilities and authored CSS, independently of design-system prefix
/// resolution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CompilerOptions {
    /// Project lint overrides, applied field by field after stylesheet settings.
    pub lint: LintOverrides,
    /// Prefix for Stillwind-owned custom properties, for example `--sw-if-<hash>`.
    pub prefix: String,
    /// Prefix for internal custom properties kept for Tailwind compatibility.
    pub tailwind_prefix: String,
    /// Extra namespace mixed into class identity, so two independently built projects
    /// can share one design system without colliding on generated class names.
    pub class_namespace: String,
    /// Emit native CSS `if()` for experimental value and variant conditions.
    /// False (default) uses custom-property fallbacks; all conditional variants need
    /// `revert-rule`. Value comparisons, negated variants, and CSS-wide helper branches require true.
    pub use_css_if: bool,
    /// Revision of the identifier generators a host supplies for this compilation.
    ///
    /// A host that computes generated names in JavaScript passes a hash of its
    /// generator functions here, because their output is otherwise invisible to the
    /// artifact cache. It defaults to the empty string, which denotes the built-in
    /// generators, and carries no meaning for the compiler beyond its effect on the
    /// artifact key.
    pub naming_revision: String,
    /// Number of base-36 digits a built-in generated name keeps from its request hash.
    ///
    /// Every family shares the length. A shorter spelling is easier to read in
    /// generated markup, while a longer one leaves more room before two declarations
    /// can collide, so a host that generates many names raises it.
    pub hash_length: usize,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            lint: LintOverrides::default(),
            prefix: "--sw-".to_owned(),
            tailwind_prefix: "--tw-".to_owned(),
            class_namespace: String::new(),
            use_css_if: false,
            naming_revision: String::new(),
            hash_length: DEFAULT_HASH_LENGTH,
        }
    }
}

impl CompilerOptions {
    /// The design-system configuration these options imply, for snapshot construction.
    pub fn design_system(&self) -> DesignSystemOptions {
        DesignSystemOptions {
            prefix: self.prefix.clone(),
            tailwind_prefix: self.tailwind_prefix.clone(),
        }
    }

    /// Returns effective lint settings for this compilation without modifying the design system.
    pub fn lint_settings(&self, snapshot: &DesignSystemSnapshot) -> LintSettings {
        self.lint.resolve(&snapshot.theme().config().lint_settings)
    }

    /// Checks that the two prefixes are valid and do not conflict, and that the
    /// generated-name hash length can spell a request digest.
    ///
    /// # Errors
    ///
    /// Returns the typed prefix-shape or namespace-conflict failures from
    /// [`DesignSystemOptions::validate`], retaining the rejected configuration values,
    /// or the rejected hash length when it is outside the range a digest can fill.
    pub fn validate(&self) -> Result<(), CompileDiagnostic> {
        self.design_system().validate()?;
        if !(1..=MAXIMUM_HASH_LENGTH).contains(&self.hash_length) {
            return Err(CompileDiagnostic::InvalidHashLength {
                length: self.hash_length,
                maximum: MAXIMUM_HASH_LENGTH,
            });
        }
        Ok(())
    }

    /// Returns the same options with both prefixes taken from `snapshot`.
    ///
    /// A snapshot built from a theme that configures its own prefixes is the source of
    /// truth for them, so generated text always agrees with the theme it was resolved
    /// against.
    pub(crate) fn resolved(self, snapshot: &DesignSystemSnapshot) -> Self {
        Self {
            prefix: snapshot.config().prefix.clone(),
            tailwind_prefix: snapshot.config().tailwind_prefix.clone(),
            ..self
        }
    }

    /// Substitutes the configured prefixes into generated text.
    ///
    /// Built-in declarations are written with the default spellings, so they are
    /// replaced here in one pass. The private-use sentinels keep a configured prefix
    /// that itself contains `--sw-` or `--tw-` from being rewritten twice.
    pub(crate) fn rewrite_generated(&self, value: &str) -> String {
        value
            .replace("--sw-", "\u{e000}stillwind-prefix\u{e001}")
            .replace("--tw-", "\u{e000}tailwind-prefix\u{e001}")
            .replace("\u{e000}stillwind-prefix\u{e001}", &self.prefix)
            .replace("\u{e000}tailwind-prefix\u{e001}", &self.tailwind_prefix)
    }
}
