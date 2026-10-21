//! Host naming callbacks and the inputs that select them for a compilation.

use std::rc::Rc;

use lean_string::LeanString;

use super::super::{
    AnchorName, ClassContext, ConditionContext, ConditionHelperName, ContainerName,
    DeclarationContext, IdentifierHash, IdentifierMode, KeyframesName, LayerContext, LayerName,
    LocatorContext, RefMarker, UtilityClass, ViewTransitionClass, ViewTransitionSlotName,
};
use super::IdentifierContext;

/// The naming inputs one compilation runs with.
///
/// Generators are not part of [`crate::CompilerOptions`], because options must stay a
/// plain serializable configuration whose debug output is the artifact key. A host that
/// supplies generators pairs them with a matching
/// [`crate::CompilerOptions::naming_revision`] so cached artifacts are not reused across
/// naming changes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Identifiers {
    generators: GeneratorSet,
    mode: IdentifierMode,
}

impl Identifiers {
    /// Creates naming inputs from an optional host generator and compilation mode.
    ///
    /// A development compilation keeps producing usable names when a generator is
    /// rejected and reports the rejection in the result; a release compilation fails
    /// instead. The mode is a compilation input, so a host that caches artifacts must
    /// keep the two modes apart.
    #[must_use]
    pub fn new(generator: Option<Rc<dyn IdentifierGenerator>>, mode: IdentifierMode) -> Self {
        Self {
            generators: GeneratorSet { generator },
            mode,
        }
    }

    /// One compilation's memoized name allocation for these inputs.
    ///
    /// `hash_length` is the number of base-36 digits the compilation's default names
    /// keep, which comes from [`crate::CompilerOptions::hash_length`].
    pub(crate) fn context(&self, hash_length: usize) -> IdentifierContext {
        IdentifierContext::new(self.generators.clone(), self.mode, hash_length)
    }
}

/// What one generator method returned.
///
/// A host either supplies a name or leaves the family to the compiler. Returning a name
/// means the compiler writes that text, with no prefix; returning [`Self::Default`]
/// selects the compiler's own name, with its prefix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GeneratedName<T> {
    /// The compiler's own name for this request.
    Default,
    /// A host name, cleaned by the family's type and written as returned.
    Provided(T),
}

impl<T> GeneratedName<T> {
    /// Wraps a built name as a host-provided name.
    pub fn provided(name: T) -> Self {
        Self::Provided(name)
    }
}

/// A host-provided generator. Every method has the family's own arguments.
///
/// Implementations must be pure: the same request must produce the same name, and the
/// result must not depend on call order, call count, time, randomness, or the build
/// mode. Every method defaults to [`GeneratedName::Default`], so overriding one family
/// is a supported use.
///
/// Generators are called synchronously on the thread that owns the compilation.
pub trait IdentifierGenerator: std::fmt::Debug {
    /// A utility class, on its way into a class attribute and a class selector.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this call.
    fn utility_class(
        &self,
        _context: &ClassContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<UtilityClass>, LeanString> {
        Ok(GeneratedName::Default)
    }

    /// A ref marker class, on its way into a class attribute and a class selector.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this declaration.
    fn ref_marker(
        &self,
        _context: &DeclarationContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<RefMarker>, LeanString> {
        Ok(GeneratedName::Default)
    }

    /// An `anchor-name` custom-ident.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this projection.
    fn anchor_name(
        &self,
        _context: &LocatorContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<AnchorName>, LeanString> {
        Ok(GeneratedName::Default)
    }

    /// A `container-name` custom-ident.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this projection.
    fn container_name(
        &self,
        _context: &LocatorContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<ContainerName>, LeanString> {
        Ok(GeneratedName::Default)
    }

    /// A `@keyframes` name, referenced by `animation-name`.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this declaration.
    fn keyframes_name(
        &self,
        _context: &DeclarationContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<KeyframesName>, LeanString> {
        Ok(GeneratedName::Default)
    }

    /// A `view-transition-class` custom-ident, which is also the transition's identity.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this declaration.
    fn view_transition_class(
        &self,
        _context: &DeclarationContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<ViewTransitionClass>, LeanString> {
        Ok(GeneratedName::Default)
    }

    /// A conditional helper or rule-boundary query name, without its leading double
    /// hyphen.
    ///
    /// `hash` covers the role and canonical context text, and `text_hash` covers the
    /// context text alone. Naming helpers by `hash` distinguishes every role, while
    /// naming them by `text_hash` and spelling the role in the name lets two roles
    /// share one suffix.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this helper.
    fn condition_helper(
        &self,
        _context: &ConditionContext,
        _hash: &IdentifierHash,
        _text_hash: &IdentifierHash,
    ) -> Result<GeneratedName<ConditionHelperName>, LeanString> {
        Ok(GeneratedName::Default)
    }

    /// The custom property one guarded view-transition use writes its class token into.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this use.
    fn view_transition_slot(
        &self,
        _context: &ClassContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<ViewTransitionSlotName>, LeanString> {
        Ok(GeneratedName::Default)
    }

    /// A `@layer` name.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this template.
    fn layer_name(
        &self,
        _context: &LayerContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<LayerName>, LeanString> {
        Ok(GeneratedName::Default)
    }
}

/// The generators one compilation uses, with the default generator as fallback.
///
/// Equality compares generator identity, so rebuilding a set from the same generator
/// compares equal while a different generator never does.
#[derive(Clone, Debug, Default)]
pub struct GeneratorSet {
    generator: Option<Rc<dyn IdentifierGenerator>>,
}

impl GeneratorSet {
    /// A set that asks `generator` first and falls back per family.
    pub fn new(generator: Rc<dyn IdentifierGenerator>) -> Self {
        Self {
            generator: Some(generator),
        }
    }

    /// The host generator, if any.
    pub fn generator(&self) -> Option<&dyn IdentifierGenerator> {
        self.generator.as_deref()
    }
}

impl PartialEq for GeneratorSet {
    fn eq(&self, other: &Self) -> bool {
        match (&self.generator, &other.generator) {
            (None, None) => true,
            (Some(left), Some(right)) => Rc::ptr_eq(left, right),
            _ => false,
        }
    }
}

impl Eq for GeneratorSet {}
