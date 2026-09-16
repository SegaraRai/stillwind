//! Name allocation: the generator contract, its injection point, and one compilation's memo.
//!
//! [`Identifiers`] is what a host supplies to a compilation. Each family has its own generator
//! method, because each one is given different context: a utility class belongs to a call and a
//! branch, a declaration name belongs to a module and a binding, and an anonymous layer belongs to
//! a template and a position. Every method is also given the hash of that context, untruncated, so
//! a generator may use the hash alone without reimplementing the identity rule. A method that
//! returns [`GeneratedName::Builtin`] keeps the compiler's own name.
//!
//! [`IdentifierContext`] is one compilation's allocation: the host generator first, the compiler's
//! own name second, memoized per family and identity. A helper asked twice therefore cannot
//! observe two calls of an impure generator and cannot produce two names for one declaration, and
//! a rejection is memoized as a rejection, so one compilation reports it once.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use stillwind_diagnostics::CompileErrorKind;

use super::kind::{IdentifierKind, IdentifierMode, builtin_digest_digits, builtin_prefix};
use super::name::{
    AnchorName, ContainerName, CustomPropertyName, KeyframesName, LayerName, RefMarker,
    ScopeMarker, Token, UtilityClass, ViewTransitionName,
};
use super::request::{
    AnchorNameRequest, ClassContext, ClassName, ConditionContext, ConditionHelperRequest,
    ContainerNameRequest, DeclarationContext, IdentifierHash, KeyframesNameRequest, LayerContext,
    LayerNameRequest, LocatorContext, NameRequest, RefMarkerName, ScopeMarkerName,
    ViewTransitionNameRequest,
};

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
    /// The built-in names, in release mode.
    pub fn builtin() -> Self {
        Self::default()
    }

    /// A set that asks `generator` first and falls back per family.
    pub fn with(generator: Arc<dyn IdentifierGenerator>) -> Self {
        Self {
            generators: GeneratorSet::new(generator),
            mode: IdentifierMode::Release,
        }
    }

    /// Returns the same inputs in `mode`.
    ///
    /// A development compilation keeps producing usable names when a generator is
    /// rejected and reports the rejection in the result; a release compilation fails
    /// instead. The mode is a compilation input, so a host that caches artifacts must
    /// keep the two modes apart.
    #[must_use]
    pub fn in_mode(mut self, mode: IdentifierMode) -> Self {
        self.mode = mode;
        self
    }

    /// One compilation's memoized name allocation for these inputs.
    pub(crate) fn context(&self) -> IdentifierContext {
        IdentifierContext::in_mode(self.generators.clone(), self.mode)
    }
}

/// What one generator method returned.
///
/// A host either supplies a name or leaves the family to the compiler. Returning a name
/// means the compiler writes that text, with no prefix; returning [`Self::Builtin`]
/// selects the compiler's own name, with its prefix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GeneratedName<T> {
    /// The compiler's own name for this request.
    Builtin,
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
/// result must not depend on call order, call count, time, randomness, or the build mode.
/// Every method defaults to [`GeneratedName::Builtin`], so overriding one family is a
/// supported use.
///
/// Sharing a generator across threads only forwards calls, because the purity contract
/// already forbids state-dependent output. Generators are still used synchronously within
/// one compilation.
pub trait IdentifierGenerator: fmt::Debug + Send + Sync {
    /// A utility class, on its way into a class attribute and a class selector.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this call.
    fn utility_class(
        &self,
        _context: &ClassContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<UtilityClass>, String> {
        Ok(GeneratedName::Builtin)
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
    ) -> Result<GeneratedName<RefMarker>, String> {
        Ok(GeneratedName::Builtin)
    }

    /// A scope marker class, on its way into a class attribute and a class selector.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this declaration.
    fn scope_marker(
        &self,
        _context: &DeclarationContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<ScopeMarker>, String> {
        Ok(GeneratedName::Builtin)
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
    ) -> Result<GeneratedName<AnchorName>, String> {
        Ok(GeneratedName::Builtin)
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
    ) -> Result<GeneratedName<ContainerName>, String> {
        Ok(GeneratedName::Builtin)
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
    ) -> Result<GeneratedName<KeyframesName>, String> {
        Ok(GeneratedName::Builtin)
    }

    /// A `view-transition-name` custom-ident.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this declaration.
    fn view_transition_name(
        &self,
        _context: &DeclarationContext,
        _hash: &IdentifierHash,
    ) -> Result<GeneratedName<ViewTransitionName>, String> {
        Ok(GeneratedName::Builtin)
    }

    /// A conditional helper custom property, without its leading double hyphen.
    ///
    /// `hash` covers the helper's role together with the text it serves, and `text_hash` covers
    /// the text alone. Naming helpers by `hash` distinguishes every role, while naming them by
    /// `text_hash` and spelling the role in the name lets two roles share one suffix.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this helper.
    fn condition_helper(
        &self,
        _context: &ConditionContext,
        _hash: &IdentifierHash,
        _text_hash: &IdentifierHash,
    ) -> Result<GeneratedName<CustomPropertyName>, String> {
        Ok(GeneratedName::Builtin)
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
    ) -> Result<GeneratedName<LayerName>, String> {
        Ok(GeneratedName::Builtin)
    }
}

/// The generators one compilation uses, with the built-in generator as fallback.
///
/// Equality compares generator identity, so rebuilding a set from the same generator
/// compares equal while a different generator never does.
#[derive(Clone, Debug, Default)]
pub struct GeneratorSet {
    generator: Option<Arc<dyn IdentifierGenerator>>,
}

impl GeneratorSet {
    /// A set that generates every name with the built-in generator.
    pub fn builtin() -> Self {
        Self::default()
    }

    /// A set that asks `generator` first and falls back per family.
    pub fn new(generator: Arc<dyn IdentifierGenerator>) -> Self {
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
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }
}

impl Eq for GeneratorSet {}

/// One name a generator rejected, kept so a host can warn about its own generator.
///
/// A rejected name has no source location: it comes from the generator, not from the
/// source. That is why it is not a lint finding, which always describes authored code, and
/// why its audience is the bundler plugin, which receives the compilation result and can
/// warn with its own position.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RejectedIdentifier {
    /// Family the generator was asked for.
    pub kind: IdentifierKind,
    /// Canonical hash of the request, which identifies the rejected input.
    pub hash: String,
    /// Why the name was rejected: the generator's message, or why a returned name could
    /// not be used.
    pub reason: String,
    /// Whether the built-in name was substituted for it.
    pub substituted: bool,
}

/// Rejected names one compilation collected, in request order.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RejectedNames {
    /// Rejections, deduplicated by family, hash, and reason.
    entries: Vec<RejectedIdentifier>,
    /// Keys of the entries, used to report one rejection once.
    #[serde(skip)]
    seen: BTreeSet<(IdentifierKind, String, String)>,
}

impl RejectedNames {
    /// Records one rejection unless the same one was already recorded.
    pub(crate) fn push(&mut self, rejected: RejectedIdentifier) {
        let key = (
            rejected.kind,
            rejected.hash.clone(),
            rejected.reason.clone(),
        );
        if self.seen.insert(key) {
            self.entries.push(rejected);
        }
    }

    /// Whether nothing was rejected.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The rejections, in request order.
    pub fn entries(&self) -> &[RejectedIdentifier] {
        &self.entries
    }

    /// Consumes the record, returning the rejections in request order.
    pub fn into_entries(self) -> Vec<RejectedIdentifier> {
        self.entries
    }
}

/// One compilation's name allocation: host generator first, built-in fallback, memoized.
///
/// A request always yields one result within a compilation, so a helper asked twice
/// cannot observe two calls of an impure generator and cannot produce two names for one
/// declaration. A rejection is memoized as a rejection, so one compilation reports it once.
#[derive(Clone, Debug)]
pub struct IdentifierContext {
    generators: GeneratorSet,
    /// Compilation mode deciding whether a rejection falls back or fails.
    mode: IdentifierMode,
    memo: RefCell<BTreeMap<MemoKey, Result<String, String>>>,
    /// Rejections collected in development mode.
    rejected: RefCell<RejectedNames>,
}

/// Memo key: the family and the canonical hash of the request.
type MemoKey = (IdentifierKind, IdentifierHash);

impl IdentifierContext {
    /// Starts a context for `generators` in release mode.
    pub fn new(generators: GeneratorSet) -> Self {
        Self {
            generators,
            mode: IdentifierMode::Release,
            memo: RefCell::new(BTreeMap::new()),
            rejected: RefCell::new(RejectedNames::default()),
        }
    }

    /// Starts a context that substitutes the built-in name when a generator is rejected.
    pub fn in_mode(generators: GeneratorSet, mode: IdentifierMode) -> Self {
        Self {
            mode,
            ..Self::new(generators)
        }
    }

    /// Starts a context that uses the built-in generator for every family.
    pub fn builtin() -> Self {
        Self::new(GeneratorSet::builtin())
    }

    /// The mode this context was built for.
    pub fn mode(&self) -> IdentifierMode {
        self.mode
    }

    /// The generators this context was built from.
    pub fn generators(&self) -> &GeneratorSet {
        &self.generators
    }

    /// Drains the rejections collected so far, leaving the context empty of records.
    pub(crate) fn drain_rejected(&self) -> RejectedNames {
        std::mem::take(&mut self.rejected.borrow_mut())
    }

    /// A utility class token for one call and branch.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn class_name(
        &self,
        context: &ClassContext,
    ) -> Result<UtilityClass, CompileErrorKind> {
        self.resolve_name(&ClassName(context.clone()))
    }

    /// A ref marker class token, without a leading dot.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn ref_marker_name(
        &self,
        context: &DeclarationContext,
    ) -> Result<RefMarker, CompileErrorKind> {
        self.resolve_name(&RefMarkerName(context.clone()))
    }

    /// A scope marker class token, without a leading dot.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn scope_marker_name(
        &self,
        context: &DeclarationContext,
    ) -> Result<ScopeMarker, CompileErrorKind> {
        self.resolve_name(&ScopeMarkerName(context.clone()))
    }

    /// An anchor name for one ref projection.
    ///
    /// The built-in name is the identity hash alone, whether or not the declaration has a
    /// label: an anchor name is a custom-ident every position-anchor resolves through, so a
    /// label would make renaming it change which elements anchor to which.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn anchor_name(
        &self,
        context: &LocatorContext,
    ) -> Result<AnchorName, CompileErrorKind> {
        self.resolve_name(&AnchorNameRequest(context.clone()))
    }

    /// A container name for one ref projection.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn container_name(
        &self,
        context: &LocatorContext,
    ) -> Result<ContainerName, CompileErrorKind> {
        self.resolve_name(&ContainerNameRequest(context.clone()))
    }

    /// A keyframes name, referenced by animation-name.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn animation_name(
        &self,
        context: &DeclarationContext,
    ) -> Result<KeyframesName, CompileErrorKind> {
        self.resolve_name(&KeyframesNameRequest(context.clone()))
    }

    /// A view-transition name.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn view_transition_name(
        &self,
        context: &DeclarationContext,
    ) -> Result<ViewTransitionName, CompileErrorKind> {
        self.resolve_name(&ViewTransitionNameRequest(context.clone()))
    }

    /// A conditional helper custom property, without its leading double hyphen.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn condition_helper(
        &self,
        context: &ConditionContext,
    ) -> Result<CustomPropertyName, CompileErrorKind> {
        self.resolve_name(&ConditionHelperRequest(context.clone()))
    }

    /// An anonymous layer name, without the traversal index its writer appends.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn layer_name(&self, context: &LayerContext) -> Result<LayerName, CompileErrorKind> {
        self.resolve_name(&LayerNameRequest(context.clone()))
    }

    /// The text one request resolves to, asking the host generator first.
    ///
    /// A host name is written as returned, with no built-in prefix; a built-in name is
    /// prefixed by the compiler. A rejected request either fails the compilation or, in
    /// development mode, resolves to the built-in name and is recorded once.
    ///
    /// # Errors
    ///
    /// Returns [`CompileErrorKind::IdentifierGenerationFailed`] for a rejected request in
    /// release mode.
    pub(crate) fn resolve_name<T: Token, R: NameRequest<T>>(
        &self,
        request: &R,
    ) -> Result<T, CompileErrorKind> {
        let kind = request.kind();
        let hash = request.hash();
        let key = (kind, hash);
        if let Some(cached) = self.memo.borrow().get(&key) {
            return self.settle(request, &hash, cached.clone());
        }
        let generated = self.generate(request, &hash);
        self.memo.borrow_mut().insert(key, generated.clone());
        self.settle(request, &hash, generated)
    }

    /// Asks the host generator, or keeps the built-in name when it declines.
    fn generate<T: Token, R: NameRequest<T>>(
        &self,
        request: &R,
        hash: &IdentifierHash,
    ) -> Result<String, String> {
        let name = match self.generators.generator() {
            Some(generator) => request.name(generator, hash),
            None => Ok(GeneratedName::Builtin),
        };
        match name {
            Ok(GeneratedName::Builtin) => Ok(request.builtin(hash)),
            Ok(GeneratedName::Provided(name)) => Ok(name.as_str().to_owned()),
            Err(message) => Err(message),
        }
    }

    /// Turns one generated or failed request into this context's result.
    fn settle<T: Token, R: NameRequest<T>>(
        &self,
        request: &R,
        hash: &IdentifierHash,
        generated: Result<String, String>,
    ) -> Result<T, CompileErrorKind> {
        let kind = request.kind();
        match generated {
            Ok(name) => T::new(&name).map_err(|error| {
                CompileErrorKind::IdentifierGenerationFailed(
                    kind.as_str().into(),
                    error.to_string().into(),
                )
            }),
            Err(reason) => match self.mode {
                IdentifierMode::Development => {
                    self.rejected.borrow_mut().push(RejectedIdentifier {
                        kind,
                        hash: hash.to_hex(),
                        reason,
                        substituted: true,
                    });
                    T::new(&request.builtin(hash)).map_err(|error| {
                        CompileErrorKind::IdentifierGenerationFailed(
                            kind.as_str().into(),
                            error.to_string().into(),
                        )
                    })
                }
                IdentifierMode::Release => Err(CompileErrorKind::IdentifierGenerationFailed(
                    kind.as_str().into(),
                    reason.into(),
                )),
            },
        }
    }
}

/// The built-in name for one request, given the request's canonical hash.
///
/// Every family the built-in generator owns carries its prefix here, and no host name passes
/// through this function. The digest is truncated: the pool check compares identities, so a name
/// only has to be short enough to read.
pub(super) fn builtin_text(kind: IdentifierKind, hash: &IdentifierHash) -> String {
    format!(
        "{}{}",
        builtin_prefix(kind),
        hash.short_hex(builtin_digest_digits(kind)),
    )
}
