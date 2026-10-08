//! Memoized name allocation, fallback, and rejection records for one compilation.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use lean_string::LeanStr;
use serde::{Deserialize, Serialize};

use stillwind_diagnostics::CompileDiagnostic;

use super::super::kind::{IdentifierKind, IdentifierMode};
use super::super::name::{
    AnchorName, ConditionHelperName, ContainerName, KeyframesName, Label, LayerName, RefMarker,
    Token, UtilityClass, ViewTransitionClass, ViewTransitionSlotName,
};
use super::super::request::{
    AnchorNameRequest, ClassContext, ClassName, ConditionContext, ConditionHelperRequest,
    ContainerNameRequest, DeclarationContext, IdentifierHash, KeyframesNameRequest, LayerContext,
    LayerNameRequest, LocatorContext, NameRequest, RefMarkerName, ViewTransitionClassRequest,
    ViewTransitionSlotRequest,
};
use super::{GeneratedName, GeneratorSet};

/// One name a generator rejected, kept so a host can warn about its own generator.
///
/// A rejected name has no source location: it comes from the generator, not from the
/// source. That is why it is not a lint finding, which always describes authored code,
/// and why its audience is the bundler plugin, which receives the compilation result
/// and can warn with its own position.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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

/// One generated name that is not tied to a declaration in the source.
///
/// A conditional helper is named for the CSS it serves and an anonymous layer for its
/// template, so neither has a source position of its own. A document-wide pool check
/// still has to see their names, so one compilation collects them beside the names each
/// declaration reports.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DetachedName {
    /// Family the name belongs to.
    pub(crate) kind: IdentifierKind,
    /// Canonical spelling of the name.
    pub(crate) name: LeanStr,
    /// Owner identity used by document-wide name collision checks.
    pub(crate) identity: LeanStr,
}

/// Memo key: the family, the canonical hash of the request, and the label it spells.
///
/// The label is not part of the hash, because the identity of a declaration does not
/// depend on it, but it does decide the name, so two requests for one identity that
/// differ in their label are two spellings and must not share a memo entry.
type MemoKey = (IdentifierKind, IdentifierHash, Option<Label>);

/// One compilation's name allocation: host generator first, built-in fallback,
/// memoized.
///
/// A request always yields one result within a compilation, so a helper asked twice
/// cannot observe two calls of an impure generator and cannot produce two names for
/// one declaration. A rejection is memoized as a rejection, so one compilation reports
/// it once.
#[derive(Clone, Debug)]
pub struct IdentifierContext {
    generators: GeneratorSet,
    /// Compilation mode deciding whether a rejection falls back or fails.
    mode: IdentifierMode,
    /// How many base-36 digits of a request hash a built-in name keeps.
    hash_length: usize,
    memo: RefCell<BTreeMap<MemoKey, Result<LeanStr, String>>>,
    /// Rejections collected in development mode.
    rejected: RefCell<RejectedNames>,
    /// Names of the families that carry no declaration position.
    detached: RefCell<Vec<DetachedName>>,
}

impl IdentifierContext {
    /// Starts a context with the requested generators, mode, and hash length.
    ///
    /// `hash_length` is the number of base-36 digits a built-in name keeps. It is the
    /// compilation's own spelling choice and never reaches a host generator, which is
    /// given the whole digest.
    pub fn new(generators: GeneratorSet, mode: IdentifierMode, hash_length: usize) -> Self {
        Self {
            generators,
            mode,
            hash_length,
            memo: RefCell::new(BTreeMap::new()),
            rejected: RefCell::new(RejectedNames::default()),
            detached: RefCell::new(Vec::new()),
        }
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

    /// Drains the names collected so far for families that carry no declaration
    /// position.
    ///
    /// A memoized request is collected once, so a document-wide pool check sees each
    /// spelling once however many rules use it.
    pub(crate) fn drain_detached(&self) -> Vec<DetachedName> {
        std::mem::take(&mut self.detached.borrow_mut())
    }

    /// Records one anonymous layer spelling, whose traversal index the writer appended.
    ///
    /// `context` must be the context the layer name was requested with, so the record
    /// identifies the same owner and the layer pool sees the spelling the document
    /// uses.
    pub(crate) fn record_layer(&self, context: &LayerContext, written: &str) {
        self.detached.borrow_mut().push(DetachedName {
            kind: IdentifierKind::LayerName,
            name: written.into(),
            identity: format!(
                "stillwind-identity/v1\0anonymous-layer\0{}\0{}\0",
                context.template, context.namespace,
            )
            .into(),
        });
    }

    /// A utility class token for one call and branch.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn class_name(
        &self,
        context: &ClassContext,
    ) -> Result<UtilityClass, CompileDiagnostic> {
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
    ) -> Result<RefMarker, CompileDiagnostic> {
        self.resolve_name(&RefMarkerName(context.clone()))
    }

    /// An anchor name for one ref projection.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn anchor_name(
        &self,
        context: &LocatorContext,
    ) -> Result<AnchorName, CompileDiagnostic> {
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
    ) -> Result<ContainerName, CompileDiagnostic> {
        self.resolve_name(&ContainerNameRequest(context.clone()))
    }

    /// A keyframes name, referenced by animation-name.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn keyframes_name(
        &self,
        context: &DeclarationContext,
    ) -> Result<KeyframesName, CompileDiagnostic> {
        self.resolve_name(&KeyframesNameRequest(context.clone()))
    }

    /// A view-transition class token, which is also the transition's identity.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn view_transition_class(
        &self,
        context: &DeclarationContext,
    ) -> Result<ViewTransitionClass, CompileDiagnostic> {
        self.resolve_name(&ViewTransitionClassRequest(context.clone()))
    }

    /// A conditional helper or rule-boundary query name, without its leading double
    /// hyphen.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn condition_helper(
        &self,
        context: &ConditionContext,
    ) -> Result<ConditionHelperName, CompileDiagnostic> {
        self.resolve_name(&ConditionHelperRequest(context.clone()))
    }

    /// The custom property one guarded view-transition use writes its class token into.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn view_transition_slot(
        &self,
        context: &ClassContext,
    ) -> Result<ViewTransitionSlotName, CompileDiagnostic> {
        self.resolve_name(&ViewTransitionSlotRequest(context.clone()))
    }

    /// An anonymous layer name, without the traversal index its writer appends.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn layer_name(
        &self,
        context: &LayerContext,
    ) -> Result<LayerName, CompileDiagnostic> {
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
    /// Returns [`CompileDiagnostic::IdentifierGenerationFailed`] for a rejected request
    /// in release mode.
    pub(crate) fn resolve_name<T: Token, R: NameRequest<T>>(
        &self,
        request: &R,
    ) -> Result<T, CompileDiagnostic> {
        let kind = request.kind();
        let hash = request.hash();
        let key = (kind, hash, request.label().cloned());
        if let Some(cached) = self.memo.borrow().get(&key) {
            return self.settle(request, &hash, cached.clone());
        }
        let generated = self.generate(request, &hash);
        self.memo.borrow_mut().insert(key, generated.clone());
        let settled = self.settle(request, &hash, generated);
        if let (Some(identity), Ok(name)) = (request.detached_owner_identity(), &settled) {
            self.detached.borrow_mut().push(DetachedName {
                kind,
                name: name.as_str().into(),
                identity: identity.into(),
            });
        }
        settled
    }

    /// Asks the host generator, or keeps the built-in name when it declines.
    fn generate<T: Token, R: NameRequest<T>>(
        &self,
        request: &R,
        hash: &IdentifierHash,
    ) -> Result<LeanStr, String> {
        let name = match self.generators.generator() {
            Some(generator) => request.name(generator, hash),
            None => Ok(GeneratedName::Builtin),
        };
        match name {
            Ok(GeneratedName::Builtin) => Ok(request.builtin(hash, self.hash_length).into()),
            Ok(GeneratedName::Provided(name)) => Ok(name.as_str().into()),
            Err(message) => Err(message),
        }
    }

    /// Turns one generated or failed request into this context's result.
    fn settle<T: Token, R: NameRequest<T>>(
        &self,
        request: &R,
        hash: &IdentifierHash,
        generated: Result<LeanStr, String>,
    ) -> Result<T, CompileDiagnostic> {
        match generated {
            // A host name arrives as a name its own family already built, so cleaning
            // it again only proves the family's rule is idempotent.
            Ok(name) => T::new(&name).map_err(|error| {
                CompileDiagnostic::IdentifierGenerationFailed(
                    request.kind().as_str().into(),
                    error.to_string().into(),
                )
            }),
            Err(reason) => self.reject(request, hash, reason),
        }
    }

    /// Fails the compilation, or substitutes the compiler's own name in development
    /// mode.
    ///
    /// A substitution is recorded once per family and identity, so a host can warn
    /// about its own generator without the same request being reported again and again.
    fn reject<T: Token, R: NameRequest<T>>(
        &self,
        request: &R,
        hash: &IdentifierHash,
        reason: String,
    ) -> Result<T, CompileDiagnostic> {
        let kind = request.kind();
        let failure = |detail: String| {
            CompileDiagnostic::IdentifierGenerationFailed(kind.as_str().into(), detail.into())
        };
        if self.mode == IdentifierMode::Release {
            return Err(failure(reason));
        }
        self.rejected.borrow_mut().push(RejectedIdentifier {
            kind,
            hash: hash.to_hex(),
            reason,
            substituted: true,
        });
        T::new(&request.builtin(hash, self.hash_length)).map_err(|error| failure(error.to_string()))
    }
}
