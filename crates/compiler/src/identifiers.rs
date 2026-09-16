//! Branded generated identifiers: cleaning at construction, escaping at write.
//!
//! The compiler decides one name per identity, and the type of that name says where the name
//! may be written. Each family is a newtype over its canonical spelling, so a name that
//! exists has already been cleaned, and an arbitrary string cannot be written as a generated
//! name.
//!
//! A family's constructor cleans the name once. It replaces what the family's destinations
//! cannot carry, and it makes the name usable as a JavaScript object key where the family is
//! written as one. A built-in name and a host-provided name are cleaned the same way: every
//! built-in name already satisfies these rules, so the step never changes a built-in name.
//!
//! A render method escapes for one destination, and a family declares only the render methods
//! its write sites use. A class token is written to a class attribute and to class selectors
//! as the same spelling, so one type covers both writes and the token itself is never escaped:
//! the DOM and the selector share the canonical spelling. A custom-ident family is written to
//! value positions, where the grammar can escape every character the spelling allows.
//!
//! Escaping is not part of the name. Comparisons, reports, and DOM tokens use the canonical
//! spelling, so a name never changes meaning by being written somewhere.
//!
//! [`Identifiers`] is the injection point. Each family has its own generator method, because
//! each one is given different context: a utility class belongs to a call and a branch, a
//! declaration name belongs to a module and a binding, and a layer name belongs to a template
//! and a position. Every method is also given the hash of that context, so a generator may use
//! the hash alone without reimplementing the identity rule.
//!
//! A built-in name carries the family's prefix, which separates compiler names from authored
//! CSS. A host-provided name is written exactly as returned, with no prefix: a host that
//! computes its own names owns that separation too.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use cssparser::serialize_identifier;

use stillwind_diagnostics::CompileErrorKind;

use crate::generated::js_string;
use crate::identifier::{IdentifierKind, IdentifierMode, builtin_prefix};

/// A name a family has not produced yet.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum IdentifierNameError {
    /// The name holds no characters the family can carry.
    #[error("a generated name cannot be empty")]
    Empty,
}

/// A canonical generated name of one family.
///
/// The identity plumbing builds and compares names through this trait. Cleaning a name into a
/// family and writing it to a destination are each family's own business, so the inherent
/// constructor and the render methods are declared per family.
pub trait Token: Sized {
    /// Cleans `name` into this family's canonical spelling.
    ///
    /// # Errors
    ///
    /// Returns [`IdentifierNameError::Empty`] when nothing usable remains.
    fn new(name: &str) -> Result<Self, IdentifierNameError>;

    /// The canonical spelling, which is not escaped.
    fn as_str(&self) -> &str;
}

/// Declares one family's brand type, its constructor, and the plumbing [`Token`] impl.
///
/// Render methods stay out of the macro. Which destinations a family reaches, and how each one
/// escapes, is what a reader of that family needs to see next to it.
macro_rules! brand {
    ($(
        $(#[$meta:meta])*
        $name:ident, $clean:expr;
    )*) => {
        $(
            $(#[$meta])*
            #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
            pub struct $name(String);

            impl $name {
                /// Cleans `name` into this family's canonical spelling.
                ///
                /// # Errors
                ///
                /// Returns [`IdentifierNameError::Empty`] when nothing usable remains.
                pub fn new(name: &str) -> Result<Self, IdentifierNameError> {
                    ($clean)(name).map(Self)
                }

                /// The canonical spelling, which is not escaped.
                pub fn as_str(&self) -> &str {
                    &self.0
                }
            }

            impl fmt::Display for $name {
                fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str(&self.0)
                }
            }

            impl Token for $name {
                fn new(name: &str) -> Result<Self, IdentifierNameError> {
                    Self::new(name)
                }

                fn as_str(&self) -> &str {
                    Self::as_str(self)
                }
            }
        )*
    };
}

brand! {
    /// A generated utility class, written to a class attribute and to class selectors.
    ///
    /// A DOM class token and a class selector are one spelling, so one type covers both.
    UtilityClass, class_token;
    /// A ref marker class, written where a ref is attached.
    ///
    /// Generated code keys a ref's projection lookup by this token.
    RefMarker, object_key_class_token;
    /// A scope marker class, written where a scope is attached.
    ///
    /// Generated code keys a scope's projection lookup by this token.
    ScopeMarker, object_key_class_token;
    /// An `anchor-name` custom-ident, which names the element a ref anchors to.
    AnchorName, custom_ident;
    /// A `container-name` custom-ident, which `@container` queries select.
    ContainerName, custom_ident;
    /// A `@keyframes` name, referenced by `animation-name`.
    KeyframesName, custom_ident;
    /// A `view-transition-name` custom-ident.
    ViewTransitionName, custom_ident;
    /// A custom property name, without its leading double hyphen.
    CustomPropertyName, custom_ident;
    /// A `@layer` name.
    LayerName, layer_name;
}

impl UtilityClass {
    /// This name as a class selector, including its leading dot.
    pub fn selector(&self) -> String {
        class_selector(self.as_str())
    }
}

impl RefMarker {
    /// This name as a class selector, including its leading dot.
    pub fn selector(&self) -> String {
        class_selector(self.as_str())
    }

    /// This name as the value of a JavaScript string literal.
    pub fn javascript_string(&self) -> String {
        js_string(self.as_str())
    }
}

impl ScopeMarker {
    /// This name as a class selector, including its leading dot.
    pub fn selector(&self) -> String {
        class_selector(self.as_str())
    }

    /// This name as the value of a JavaScript string literal.
    pub fn javascript_string(&self) -> String {
        js_string(self.as_str())
    }
}

impl AnchorName {
    /// This name as a custom-ident value in a property or at-rule prelude.
    pub fn custom_ident(&self) -> String {
        escaped_identifier(self.as_str())
    }
}

impl ContainerName {
    /// This name as a custom-ident value in a property or at-rule prelude.
    pub fn custom_ident(&self) -> String {
        escaped_identifier(self.as_str())
    }
}

impl KeyframesName {
    /// This name as a custom-ident value in a property or at-rule prelude.
    pub fn custom_ident(&self) -> String {
        escaped_identifier(self.as_str())
    }

    /// This name as the value of a JavaScript string literal.
    pub fn javascript_string(&self) -> String {
        js_string(self.as_str())
    }
}

impl ViewTransitionName {
    /// This name as the class selector a view-transition pseudo-element selects it by.
    ///
    /// `::view-transition-group()`, `::view-transition-old()`, and `::view-transition-new()`
    /// select the name as a class selector, so those rules write the selector spelling.
    pub fn selector(&self) -> String {
        class_selector(self.as_str())
    }

    /// This name as the value of a JavaScript string literal.
    pub fn javascript_string(&self) -> String {
        js_string(self.as_str())
    }
}

impl CustomPropertyName {
    /// This name as a custom property, including its leading double hyphen.
    ///
    /// The two hyphens belong to the property grammar rather than to the name, so the
    /// compiler adds them here.
    pub fn property(&self) -> String {
        format!("--{}", escaped_identifier(self.as_str()))
    }
}

impl LayerName {
    /// This name as the `@layer` prelude, written after the at-keyword.
    pub fn custom_ident(&self) -> String {
        escaped_identifier(self.as_str())
    }
}

/// An authored label, cleaned for use inside a generated name.
///
/// A label reaches a generator as part of a declaration context. The built-in generator does
/// not embed it, but a generator that does writes it into a name that reaches the DOM, so it
/// is cleaned like a class token.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Label(String);

impl Label {
    /// Cleans `name` into a label.
    ///
    /// # Errors
    ///
    /// Returns [`IdentifierNameError::Empty`] when nothing usable remains.
    pub fn new(name: &str) -> Result<Self, IdentifierNameError> {
        class_token(name).map(Self)
    }

    /// The canonical spelling, which is not escaped.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Label {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Token for Label {
    fn new(name: &str) -> Result<Self, IdentifierNameError> {
        Self::new(name)
    }

    fn as_str(&self) -> &str {
        Self::as_str(self)
    }
}

/// Replaces what a class token cannot carry.
///
/// A class attribute is whitespace-separated, so a token cannot hold whitespace, and NUL has
/// no CSS spelling at all. Both become `_`; every other character is kept, because a selector
/// and a JavaScript string can escape it.
fn class_token(name: &str) -> Result<String, IdentifierNameError> {
    let canonical: String = name
        .chars()
        .map(|character| match character {
            '\0' => '_',
            character if character.is_whitespace() => '_',
            character => character,
        })
        .collect();
    non_empty(canonical)
}

/// Replaces what a class token cannot carry, and keeps the result an own object key.
///
/// Generated code writes these tokens as the keys of a projection lookup, and the key is the
/// canonical token itself, so the token has to be an own entry of every object it is written
/// to. A name an object already carries would not become one, and `__proto__` would even
/// replace the prototype, so a colliding token gains one leading underscore here, where the
/// name is decided: every writer of the declaration then sees one spelling.
fn object_key_class_token(name: &str) -> Result<String, IdentifierNameError> {
    let canonical = class_token(name)?;
    Ok(if is_inherited_member(&canonical) {
        format!("_{canonical}")
    } else {
        canonical
    })
}

/// Keeps a custom-ident as written.
///
/// A custom-ident value can escape every character it can hold, so nothing is replaced. Colon
/// and slash stay in the spelling for the same reason: a selector escapes them.
fn custom_ident(name: &str) -> Result<String, IdentifierNameError> {
    non_empty(name.to_owned())
}

/// Replaces what a `@layer` name cannot carry.
///
/// A dot would name a nested layer instead of a layer of its own, so it becomes `_`.
fn layer_name(name: &str) -> Result<String, IdentifierNameError> {
    non_empty(name.replace('.', "_"))
}

/// Rejects a name that holds nothing.
fn non_empty(name: String) -> Result<String, IdentifierNameError> {
    if name.is_empty() {
        Err(IdentifierNameError::Empty)
    } else {
        Ok(name)
    }
}

/// Whether every object already carries `name`, so it could not become an own entry.
fn is_inherited_member(name: &str) -> bool {
    matches!(
        name,
        "constructor"
            | "toString"
            | "toLocaleString"
            | "valueOf"
            | "hasOwnProperty"
            | "isPrototypeOf"
            | "propertyIsEnumerable"
            | "__proto__"
            | "__defineGetter__"
            | "__defineSetter__"
            | "__lookupGetter__"
            | "__lookupSetter__"
    )
}

/// Escapes one canonical name as a class selector.
fn class_selector(name: &str) -> String {
    format!(".{}", escaped_identifier(name))
}

/// Escapes one canonical name as a CSS identifier.
fn escaped_identifier(name: &str) -> String {
    let mut escaped = String::new();
    let _ = serialize_identifier(name, &mut escaped);
    escaped
}

/// The context one utility class is generated from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassContext {
    /// Normalized module path of the call site.
    pub module: String,
    /// The caller's class namespace, which separates independently built projects.
    pub namespace: String,
    /// Byte offset of the call.
    pub call: u32,
    /// Branch discriminator and canonical spelling of the contribution.
    pub branch: String,
}

/// The context one declaration name is generated from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarationContext {
    /// Normalized module path of the declaring module.
    pub module: String,
    /// Byte offset of the first character of the declaration's name.
    pub start: u32,
    /// Byte offset just past the declaration's name.
    pub end: u32,
    /// Binding name the declaration is assigned to.
    pub binding: String,
    /// Authored label, normalized for name use, when the declaration has one.
    pub label: Option<Label>,
}

/// The context one written custom-ident is generated from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocatorContext {
    /// Normalized module path of the declaration that owns the name.
    pub module: String,
    /// Byte offset of the first character of the declaration's name.
    pub start: u32,
    /// Byte offset just past the declaration's name.
    pub end: u32,
    /// Binding name of the declaration that owns the name.
    pub binding: String,
    /// Projection the name is written for, when the writer has one.
    pub projection: Option<String>,
}

/// The context one generated layer name is generated from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayerContext {
    /// Identity of the template the layer belongs to.
    pub template: String,
    /// Position of the layer inside its template.
    pub index: u32,
}

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
        _hash: &str,
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
        _hash: &str,
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
        _hash: &str,
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
        _hash: &str,
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
        _hash: &str,
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
        _hash: &str,
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
        _hash: &str,
    ) -> Result<GeneratedName<ViewTransitionName>, String> {
        Ok(GeneratedName::Builtin)
    }

    /// A custom property name, without its leading double hyphen.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this declaration.
    fn custom_property(
        &self,
        _context: &DeclarationContext,
        _hash: &str,
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
        _hash: &str,
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
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RejectedNames {
    /// Rejections, deduplicated by family, hash, and reason.
    entries: Vec<RejectedIdentifier>,
    /// Keys of the entries, used to report one rejection once.
    #[serde(skip)]
    seen: std::collections::BTreeSet<(IdentifierKind, String, String)>,
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
type MemoKey = (IdentifierKind, String);

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
        let key = (kind, hash.clone());
        if let Some(cached) = self.memo.borrow().get(&key) {
            return self.settle(kind, &hash, cached.clone());
        }
        let generated = self.generate(request, &hash);
        self.memo.borrow_mut().insert(key, generated.clone());
        self.settle(kind, &hash, generated)
    }

    /// Asks the host generator, or keeps the built-in name when it declines.
    fn generate<T: Token, R: NameRequest<T>>(
        &self,
        request: &R,
        hash: &str,
    ) -> Result<String, String> {
        let name = match self.generators.generator() {
            Some(generator) => request.name(generator, hash),
            None => Ok(GeneratedName::Builtin),
        };
        match name {
            Ok(GeneratedName::Builtin) => Ok(builtin_text(request.kind(), hash)),
            Ok(GeneratedName::Provided(name)) => Ok(name.as_str().to_owned()),
            Err(message) => Err(message),
        }
    }

    /// Turns one generated or failed request into this context's result.
    fn settle<T: Token>(
        &self,
        kind: IdentifierKind,
        hash: &str,
        generated: Result<String, String>,
    ) -> Result<T, CompileErrorKind> {
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
                        hash: hash.to_owned(),
                        reason,
                        substituted: true,
                    });
                    T::new(&builtin_text(kind, hash)).map_err(|error| {
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
/// Every family the built-in generator owns carries its prefix here, and no host name
/// passes through this function.
fn builtin_text(kind: IdentifierKind, hash: &str) -> String {
    format!("{}{hash}", builtin_prefix(kind))
}

/// One family's name request: the context plus its canonical hash.
pub trait NameRequest<T: Token> {
    /// The family this request belongs to.
    fn kind(&self) -> IdentifierKind;

    /// Canonical hash of this request's fields.
    fn hash(&self) -> String;

    /// Asks the generator for this family, passing this request's own context.
    ///
    /// # Errors
    ///
    /// Returns the generator's own failure message for this request.
    fn name(
        &self,
        generator: &dyn IdentifierGenerator,
        hash: &str,
    ) -> Result<GeneratedName<T>, String>;
}

/// Hashes one request's fields to the eight hexadecimal digits a built-in name carries.
fn hashed(fields: &str) -> String {
    let hash = blake3::hash(fields.as_bytes());
    hash.to_hex()[..8].to_owned()
}

/// Declares one family's request type and its hash rule.
///
/// A request type is what tells the families apart: a ref marker and a keyframes name
/// share their context type but not their family, and a generated layer name has a
/// context type of its own. Naming the family in the type is what keeps the identity text,
/// the kind, and the generator method in one declaration.
macro_rules! name_requests {
    ($(
        $name:ident($context:ident) : $token:ident as $kind:ident = $identity:expr , asks($generator:ident) ;
    )*) => {
        $(
            #[doc = concat!("A ", stringify!($kind), " request.")]
            #[derive(Clone, Debug, Eq, PartialEq)]
            pub struct $name(pub $context);

            impl NameRequest<$token> for $name {
                fn kind(&self) -> IdentifierKind {
                    IdentifierKind::$kind
                }

                fn hash(&self) -> String {
                    hashed(&$identity(&self.0))
                }

                fn name(
                    &self,
                    generator: &dyn IdentifierGenerator,
                    hash: &str,
                ) -> Result<GeneratedName<$token>, String> {
                    generator.$generator(&self.0, hash)
                }
            }
        )*
    };
}

/// Canonical identity of a named declaration, in the versioned identity spelling.
fn declaration_identity(context: &impl DeclarationIdentity, kind: &str) -> String {
    format!(
        "stillwind/v1\0{}\0{kind}\0{}:{}:{}",
        context.module(),
        context.start(),
        context.end(),
        context.binding(),
    )
}

/// The declaration fields one identity is computed from.
trait DeclarationIdentity {
    /// Normalized module path of the declaring module.
    fn module(&self) -> &str;
    /// Byte offset of the first character of the declaration's name.
    fn start(&self) -> u32;
    /// Byte offset just past the declaration's name.
    fn end(&self) -> u32;
    /// Binding name the declaration is assigned to.
    fn binding(&self) -> &str;
}

impl DeclarationIdentity for DeclarationContext {
    fn module(&self) -> &str {
        &self.module
    }

    fn start(&self) -> u32 {
        self.start
    }

    fn end(&self) -> u32 {
        self.end
    }

    fn binding(&self) -> &str {
        &self.binding
    }
}

impl DeclarationIdentity for LocatorContext {
    fn module(&self) -> &str {
        &self.module
    }

    fn start(&self) -> u32 {
        self.start
    }

    fn end(&self) -> u32 {
        self.end
    }

    fn binding(&self) -> &str {
        &self.binding
    }
}

name_requests! {
    ClassName(ClassContext): UtilityClass as UtilityClass = |context: &ClassContext| format!(
        "stillwind/style/v1:{module}:{module_len}:{namespace_len}:{namespace}\u{0}{branch_len}:{branch}:{call}:{branch}",
        module = context.module,
        module_len = context.module.len(),
        namespace_len = context.namespace.len(),
        namespace = context.namespace,
        branch_len = context.branch.len(),
        branch = context.branch,
        call = context.call,
    ), asks(utility_class);
    RefMarkerName(DeclarationContext): RefMarker as RefMarker = |context: &DeclarationContext| declaration_identity(context, "ref"), asks(ref_marker);
    ScopeMarkerName(DeclarationContext): ScopeMarker as ScopeMarker = |context: &DeclarationContext| declaration_identity(context, "scope"), asks(scope_marker);
    AnchorNameRequest(LocatorContext): AnchorName as AnchorName = |context: &LocatorContext| declaration_identity(context, "ref"), asks(anchor_name);
    ContainerNameRequest(LocatorContext): ContainerName as ContainerName = |context: &LocatorContext| declaration_identity(context, "scope"), asks(container_name);
    KeyframesNameRequest(DeclarationContext): KeyframesName as KeyframesName = |context: &DeclarationContext| declaration_identity(context, "animation"), asks(keyframes_name);
    ViewTransitionNameRequest(DeclarationContext): ViewTransitionName as ViewTransitionName = |context: &DeclarationContext| declaration_identity(context, "view-transition"), asks(view_transition_name);
    CustomPropertyNameRequest(DeclarationContext): CustomPropertyName as CustomProperty = |context: &DeclarationContext| declaration_identity(context, "custom-property"), asks(custom_property);
    LayerNameRequest(LayerContext): LayerName as LayerName = |context: &LayerContext| format!(
        "stillwind/layer/v1:{template}:{index}",
        template = context.template,
        index = context.index,
    ), asks(layer_name);
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Answers with a fixed name and records every request.
    #[derive(Debug, Default)]
    struct RecordingGenerator {
        calls: Mutex<Vec<(&'static str, String)>>,
    }

    impl RecordingGenerator {
        /// Records one request and answers with the family's fixed body.
        fn record(&self, family: &'static str, hash: &str) -> String {
            self.calls
                .lock()
                .expect("test generator lock")
                .push((family, hash.to_owned()));
            format!("fixed-{family}")
        }
    }

    impl IdentifierGenerator for RecordingGenerator {
        fn utility_class(
            &self,
            _context: &ClassContext,
            hash: &str,
        ) -> Result<GeneratedName<UtilityClass>, String> {
            Ok(GeneratedName::provided(
                UtilityClass::new(&self.record("class", hash)).expect("canonical test body"),
            ))
        }

        fn ref_marker(
            &self,
            _context: &DeclarationContext,
            hash: &str,
        ) -> Result<GeneratedName<RefMarker>, String> {
            Ok(GeneratedName::provided(
                RefMarker::new(&self.record("ref-marker", hash)).expect("canonical test body"),
            ))
        }
    }

    /// Fails the utility class family.
    #[derive(Debug)]
    struct FailingGenerator;

    impl IdentifierGenerator for FailingGenerator {
        fn utility_class(
            &self,
            _context: &ClassContext,
            _hash: &str,
        ) -> Result<GeneratedName<UtilityClass>, String> {
            Err("no name for this call".to_owned())
        }
    }

    fn class_context(branch: &str) -> ClassContext {
        ClassContext {
            module: "src/a.tsx".to_owned(),
            namespace: String::new(),
            call: 12,
            branch: branch.to_owned(),
        }
    }

    fn declaration_context(label: Option<&str>) -> DeclarationContext {
        DeclarationContext {
            module: "src/a.tsx".to_owned(),
            start: 20,
            end: 24,
            binding: "menu".to_owned(),
            label: label.and_then(|label| Label::new(label).ok()),
        }
    }

    fn locator_context(declaration: &DeclarationContext) -> LocatorContext {
        LocatorContext {
            module: declaration.module.clone(),
            start: declaration.start,
            end: declaration.end,
            binding: declaration.binding.clone(),
            projection: None,
        }
    }

    #[test]
    fn a_host_name_is_written_without_the_builtin_prefix() {
        let context =
            IdentifierContext::new(GeneratorSet::new(Arc::new(RecordingGenerator::default())));
        let name = context.class_name(&class_context("0:flex")).unwrap();
        assert_eq!(name.as_str(), "fixed-class");
        let builtin = IdentifierContext::builtin()
            .class_name(&class_context("0:flex"))
            .unwrap();
        assert!(builtin.as_str().starts_with("_s_"), "{builtin:?}");
    }

    #[test]
    fn every_builtin_family_keeps_its_own_prefix() {
        let context = IdentifierContext::builtin();
        let declaration = declaration_context(Some("primary"));
        assert!(
            context
                .ref_marker_name(&declaration)
                .unwrap()
                .as_str()
                .starts_with("s_")
        );
        assert!(
            context
                .scope_marker_name(&declaration_context(None))
                .unwrap()
                .as_str()
                .starts_with("s_")
        );
        assert!(
            context
                .anchor_name(&locator_context(&declaration))
                .unwrap()
                .as_str()
                .starts_with("--sw-r-")
        );
        assert!(
            context
                .container_name(&locator_context(&declaration))
                .unwrap()
                .as_str()
                .starts_with("--sw-s-")
        );
        assert!(
            context
                .animation_name(&declaration)
                .unwrap()
                .as_str()
                .starts_with("--sw-a-")
        );
        assert!(
            context
                .view_transition_name(&declaration)
                .unwrap()
                .as_str()
                .starts_with("--sw-v-")
        );
    }

    #[test]
    fn repeated_requests_use_one_generator_call() {
        let generator = Arc::new(RecordingGenerator::default());
        let context = IdentifierContext::new(GeneratorSet::new(generator.clone()));
        let first = context.class_name(&class_context("0:flex")).unwrap();
        let second = context.class_name(&class_context("0:flex")).unwrap();
        assert_eq!(first, second);
        assert_eq!(generator.calls.lock().expect("lock").len(), 1);
    }

    #[test]
    fn development_mode_substitutes_and_records_a_rejected_name() {
        let context = IdentifierContext::in_mode(
            GeneratorSet::new(Arc::new(FailingGenerator)),
            IdentifierMode::Development,
        );
        let name = context.class_name(&class_context("0:flex")).unwrap();
        assert!(name.as_str().starts_with("_s_"), "{name:?}");
        let rejected = context.drain_rejected();
        assert_eq!(rejected.entries().len(), 1);
        let entry = &rejected.entries()[0];
        assert_eq!(entry.kind, IdentifierKind::UtilityClass);
        assert_eq!(entry.reason, "no name for this call");
        assert!(entry.substituted);
        assert!(context.drain_rejected().is_empty());
    }

    #[test]
    fn release_mode_fails_on_a_rejected_name() {
        let context = IdentifierContext::new(GeneratorSet::new(Arc::new(FailingGenerator)));
        let failure = context.class_name(&class_context("0:flex")).unwrap_err();
        assert!(matches!(
            failure,
            CompileErrorKind::IdentifierGenerationFailed(..)
        ));
        assert!(context.drain_rejected().is_empty());
    }

    #[test]
    fn the_hash_is_stable_for_equal_requests_and_specific_to_the_family() {
        assert_eq!(
            ClassName(class_context("0:flex")).hash(),
            ClassName(class_context("0:flex")).hash()
        );
        assert_ne!(
            ClassName(class_context("0:flex")).hash(),
            ClassName(class_context("0:grid")).hash()
        );
        let declaration = declaration_context(None);
        assert_ne!(
            RefMarkerName(declaration.clone()).hash(),
            KeyframesNameRequest(declaration.clone()).hash()
        );
        assert_eq!(
            AnchorNameRequest(locator_context(&declaration)).hash(),
            RefMarkerName(declaration).hash(),
            "a ref's anchor name and marker share the declaration's identity"
        );
    }

    #[test]
    fn a_class_token_an_object_already_holds_gains_a_prefix_when_it_is_built() {
        let marker = RefMarker::new("__proto__").unwrap();
        assert_eq!(marker.as_str(), "___proto__");
        assert_eq!(marker.selector(), ".___proto__");
        assert_eq!(marker.javascript_string(), "\"___proto__\"");
        assert_eq!(
            ScopeMarker::new("hasOwnProperty").unwrap().as_str(),
            "_hasOwnProperty"
        );
        // The prefix is applied once, however many times a name is rebuilt.
        assert_eq!(
            RefMarker::new(marker.as_str()).unwrap().as_str(),
            "___proto__"
        );
    }

    #[test]
    fn a_class_token_no_object_holds_keeps_its_spelling() {
        assert_eq!(
            UtilityClass::new("__proto__").unwrap().as_str(),
            "__proto__"
        );
        assert_eq!(
            KeyframesName::new("__proto__").unwrap().as_str(),
            "__proto__"
        );
    }

    #[test]
    fn report_spellings_are_distinct_per_family() {
        let kinds = [
            IdentifierKind::UtilityClass,
            IdentifierKind::RefMarker,
            IdentifierKind::ScopeMarker,
            IdentifierKind::AnchorName,
            IdentifierKind::ContainerName,
            IdentifierKind::KeyframesName,
            IdentifierKind::ViewTransitionName,
            IdentifierKind::ViewTransitionClass,
            IdentifierKind::CustomProperty,
            IdentifierKind::LayerName,
        ];
        let mut spellings = kinds.map(IdentifierKind::as_str).to_vec();
        spellings.sort_unstable();
        spellings.dedup();
        assert_eq!(spellings.len(), kinds.len());
    }
}
