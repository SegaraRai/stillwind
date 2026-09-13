//! Compiler-wide configuration and the `--sw-` / `--tw-` prefix split.
//!
//! [`CompilerOptions`] travels with one compilation. It carries the Stillwind-owned
//! property prefix, the separately configurable Tailwind-compatibility prefix, the
//! caller's class namespace, and the optional policy that forbids relationship
//! utilities. [`CompilerOptions::resolved`] pins both prefixes to the snapshot's own
//! configuration, so a caller that passes stale options still compiles against the
//! theme it handed over. Every generated text that carries a prefix is substituted by
//! this module's `rewrite_generated`; the two namespaces must stay distinct, because a
//! consumer that recognizes one must not match the other.

use serde::{Deserialize, Serialize};

use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_diagnostics::CompileErrorKind;
use stillwind_source::normalized_module_path;

/// Knobs a host may set per compilation.
///
/// Serialized in camel case with `#[serde(default)]`, so adding a field is compatible
/// with previously written configuration; defaults match the built-in `stillwind`
/// package and an unnamespaced project.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CompilerOptions {
    /// Prefix for Stillwind-owned custom properties, for example `--sw-r-<hash>`.
    pub prefix: String,
    /// Prefix for internal custom properties kept for Tailwind compatibility.
    pub tailwind_prefix: String,
    /// When set, `group`/`peer`-style relationship candidates are reported through the
    /// `prefer-typed-relationships` lint. Candidate compilation still proceeds.
    pub forbid_tailwind_relationships: bool,
    /// Extra namespace mixed into class identity, so two independently built projects
    /// can share one design system without colliding on generated class names.
    pub class_namespace: String,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            prefix: "--sw-".to_owned(),
            tailwind_prefix: "--tw-".to_owned(),
            forbid_tailwind_relationships: false,
            class_namespace: String::new(),
        }
    }
}

impl CompilerOptions {
    /// Identity input for one generated class unit: a utility class belongs to one
    /// call and one branch, never to shared content.
    ///
    /// `call` is the call-site byte offset. `branch` contains the caller's base/leaf
    /// discriminator and canonical candidate/asset spelling, not the resolved CSS
    /// declaration bytes. The same utilities written
    /// in two branches still hash to different names. Length prefixes keep the
    /// variable-length namespace and module boundaries unambiguous, and the whole
    /// string is stable across recompiles of unchanged source.
    pub(crate) fn style_class_input(&self, module: &str, call: u32, branch: &str) -> String {
        let module = normalized_module_path(module);
        format!(
            "stillwind/style/v1:{}:{}{}:{module}:{call}:{branch}",
            self.class_namespace.len(),
            self.class_namespace,
            module.len(),
        )
    }

    /// The design-system configuration these options imply, for snapshot construction.
    pub fn design_system(&self) -> DesignSystemOptions {
        DesignSystemOptions {
            prefix: self.prefix.clone(),
            tailwind_prefix: self.tailwind_prefix.clone(),
        }
    }

    /// Checks that the two prefixes are valid and do not conflict.
    ///
    /// # Errors
    ///
    /// Returns the typed prefix-shape or namespace-conflict failures from
    /// [`DesignSystemOptions::validate`], retaining the rejected configuration values.
    pub fn validate(&self) -> Result<(), CompileErrorKind> {
        self.design_system().validate()
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
