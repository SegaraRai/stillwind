//! Identifier families, CSS namespaces, generation, and write-position escaping.
//!
//! One family is one kind of name the compiler allocates: a utility class, a ref
//! marker, an animation name, and so on. Identity is not the same thing as a CSS
//! namespace, because several families can write into one namespace when the built-in
//! generators reuse a single string per declaration.
//!
//! [`IdentifierPool`] groups the families whose names must stay unique among
//! themselves, because CSS resolves them in one namespace. Validation compares a name
//! to the identity that produced it, so one declaration that appears in two pools is
//! not a conflict and only distinct identities sharing a spelling are reported.
//!
//! [`GeneratorSet`] is the injection point. Each family has its own generator method,
//! because each one is given different context: a utility class belongs to a call and a
//! branch, a declaration name belongs to a module and a binding, and a layer name
//! belongs to a template and a position. Every method is also given the hash of that
//! context, so a generator may use the hash alone without reimplementing the identity
//! rule. A method that returns [`GeneratedName::Builtin`] keeps the built-in name.
//!
//! A prefix (the class, marker, and custom-property prefixes below) marks a name as
//! compiler-owned and is added by the built-in generator only. A host body is written
//! exactly as returned, with no prefix: the prefix separates compiler names from
//! authored CSS, and a host that computes its own names owns that separation.
//!
//! A name becomes a canonical identifier when the characters a write position cannot
//! hold are replaced, and it is escaped separately for each position it is written to.
//! Comparisons, reports, and DOM tokens use the canonical spelling, so a name never
//! changes meaning by being written.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use cssparser::serialize_identifier;
use serde::{Deserialize, Serialize};

use stillwind_diagnostics::CompileErrorKind;

/// The prefix of a built-in utility class token.
pub(crate) const DEFAULT_CLASS_PREFIX: &str = "_s_";

/// The prefix that marks a built-in ref marker class token as compiler-owned.
pub(crate) const DEFAULT_REF_MARKER_PREFIX: &str = "s_";

/// The prefix that marks a built-in scope marker class token as compiler-owned.
pub(crate) const DEFAULT_SCOPE_MARKER_PREFIX: &str = "s_";

/// The custom-property prefix of a built-in ref anchor name.
pub(crate) const DEFAULT_ANCHOR_PREFIX: &str = "--sw-r-";

/// The custom-property prefix of a built-in scope container name.
pub(crate) const DEFAULT_CONTAINER_PREFIX: &str = "--sw-s-";

/// The custom-property prefix of a built-in keyframes name.
pub(crate) const DEFAULT_ANIMATION_PREFIX: &str = "--sw-a-";

/// The custom-property prefix of a built-in view-transition name.
pub(crate) const DEFAULT_VIEW_TRANSITION_PREFIX: &str = "--sw-v-";

/// When the compilation runs, which decides how a rejected name is handled.
///
/// The mode is an input of the compilation, not of [`crate::CompilerOptions`]: it
/// changes what the compiler generates, and a condition-resolved module specifier must
/// not stand in for it, because the same generated code would then meet both the
/// development and the release implementation of a helper.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum IdentifierMode {
    /// A release compilation. A rejected name fails the compilation, so a host build
    /// never ships a name the host expected to control.
    #[default]
    Release,
    /// A development compilation. A rejected name falls back to the built-in name and is
    /// recorded in [`RejectedNames`], so a running dev server stays usable while the
    /// host can warn about the generator.
    Development,
}

/// One family of generated names.
///
/// The variant decides which pool the resulting name must be checked in and how a report
/// spells the family. Serialized spelling is used only in reports, never as name content.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IdentifierKind {
    /// A generated utility class, written to the class attribute and class selectors.
    UtilityClass,
    /// A ref marker class, written where a ref is attached.
    RefMarker,
    /// A scope marker class, written where a scope is attached.
    ScopeMarker,
    /// An `anchor-name` custom-ident, which the element being anchored is named by.
    AnchorName,
    /// A `container-name` custom-ident, used by `@container` queries.
    ContainerName,
    /// A `@keyframes` name, referenced by `animation-name`.
    KeyframesName,
    /// A `view-transition-name` custom-ident.
    ViewTransitionName,
    /// A `view-transition-class` custom-ident.
    ViewTransitionClass,
    /// A custom property name, without its leading `--`.
    CustomProperty,
    /// A generated `@layer` name.
    LayerName,
}

impl IdentifierKind {
    /// Stable lowercase spelling used in symbol records and diagnostics.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UtilityClass => "class",
            Self::RefMarker => "ref-marker",
            Self::ScopeMarker => "scope-marker",
            Self::AnchorName => "anchor",
            Self::ContainerName => "container",
            Self::KeyframesName => "keyframes",
            Self::ViewTransitionName => "view-transition",
            Self::ViewTransitionClass => "view-transition-class",
            Self::CustomProperty => "custom-property",
            Self::LayerName => "layer",
        }
    }

    /// The CSS namespace this family writes into.
    pub const fn pool(self) -> IdentifierPool {
        match self {
            Self::UtilityClass | Self::RefMarker | Self::ScopeMarker => IdentifierPool::ClassToken,
            Self::AnchorName => IdentifierPool::AnchorName,
            Self::ContainerName => IdentifierPool::ContainerName,
            Self::KeyframesName => IdentifierPool::KeyframesName,
            Self::ViewTransitionName => IdentifierPool::ViewTransitionName,
            Self::ViewTransitionClass => IdentifierPool::ViewTransitionClass,
            Self::CustomProperty => IdentifierPool::CustomProperty,
            Self::LayerName => IdentifierPool::LayerName,
        }
    }
}

/// One CSS namespace whose names must be distinct across the document.
///
/// Names in different pools do not interact, so the same spelling may appear in both;
/// within one pool a spelling may only belong to one identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IdentifierPool {
    /// Class tokens and class selectors: utility classes and attachment markers.
    ClassToken,
    /// `anchor-name`, `position-anchor`, and `anchor-scope`.
    AnchorName,
    /// `container-name` and `@container` names.
    ContainerName,
    /// `@keyframes` names and `animation-name` values.
    KeyframesName,
    /// `view-transition-name` values. Runtime-derived keyed names are not allocated here.
    ViewTransitionName,
    /// `view-transition-class` values.
    ViewTransitionClass,
    /// Everything in the custom-property namespace.
    CustomProperty,
    /// `@layer` names in the document and in CSS carried by modules.
    LayerName,
}

impl IdentifierPool {
    /// Stable lowercase spelling used in diagnostics and reports.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClassToken => "class",
            Self::AnchorName => "anchor",
            Self::ContainerName => "container",
            Self::KeyframesName => "keyframes",
            Self::ViewTransitionName => "view-transition",
            Self::ViewTransitionClass => "view-transition-class",
            Self::CustomProperty => "custom-property",
            Self::LayerName => "layer",
        }
    }
}

/// A place a generated name is written to, which decides its usable characters.
///
/// The position fixes the intersection of characters the name may contain; the writer
/// escapes whatever remains, so normalization is not repeated per family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamePosition {
    /// A DOM class token. A token is whitespace-separated, so it cannot hold
    /// whitespace, and it is not escaped.
    DomToken,
    /// A class selector, written as a dot followed by the token.
    Selector,
    /// A custom-ident value, such as `anchor-name`, a `@container` name, or a
    /// `view-transition-name`.
    CustomIdent,
    /// A custom property name, written with a leading double hyphen.
    CustomProperty,
    /// A `@layer` name.
    LayerName,
}

impl NamePosition {
    /// Cleans `name` into the canonical spelling this position can hold.
    ///
    /// The compiler replaces what the position cannot carry and leaves everything else
    /// for the writer to escape. A DOM token is the only position whose characters
    /// cannot be escaped, so it also replaces NUL, which CSS cannot represent at all.
    pub fn canonical(self, name: &str) -> String {
        match self {
            Self::DomToken | Self::Selector => name
                .chars()
                .map(|character| {
                    if character.is_whitespace() || character == '\0' {
                        '_'
                    } else {
                        character
                    }
                })
                .collect(),
            Self::CustomIdent | Self::CustomProperty => name.to_owned(),
            Self::LayerName => name.replace('.', "_"),
        }
    }

    /// Escapes the canonical spelling for this position.
    ///
    /// `name` must already be canonical. Layer names are escaped like selectors,
    /// because a layer name is an identifier with the same grammar.
    pub fn escaped(self, name: &str) -> String {
        match self {
            Self::DomToken => name.to_owned(),
            Self::Selector | Self::CustomIdent | Self::LayerName => {
                let mut escaped = String::new();
                let _ = serialize_identifier(name, &mut escaped);
                escaped
            }
            Self::CustomProperty => {
                let mut escaped = String::from("--");
                let _ = serialize_identifier(name, &mut escaped);
                escaped
            }
        }
    }
}

/// Writes a class selector for a canonical class token, including its leading dot.
pub(crate) fn selector(name: &str) -> String {
    format!(".{}", NamePosition::Selector.escaped(name))
}

/// Writes a canonical custom-ident value for a property or at-rule prelude.
pub(crate) fn custom_ident(name: &str) -> String {
    NamePosition::CustomIdent.escaped(name)
}

/// Normalizes an authored label for use inside a generated marker token.
///
/// A marker token is written to the DOM and to a class selector, so whitespace and NUL
/// are replaced and every other authored character is kept readable. Escaping for the
/// selector happens where the selector is written, which is what lets a label keep
/// characters such as a colon.
pub(crate) fn marker_label(label: &str) -> String {
    NamePosition::DomToken.canonical(label)
}

/// Lowers a class token that generated code also uses as a JavaScript object key.
///
/// A lookup table maps each carried class token to the class the projection selects,
/// so every class-token family reaches generated JavaScript as a key. A name that an
/// object cannot hold as an own entry would silently change that table's meaning, so it
/// gains a leading underscore once, here, where the name is decided: every writer of the
/// same declaration then sees one spelling.
///
/// The check covers the names every object inherits and the accessors some engines add,
/// which is the whole set a generated key can collide with: `constructor`, `toString`,
/// `valueOf`, `__proto__`, `toLocaleString`, and the `__defineGetter__` family.
pub(crate) fn javascript_key(name: String) -> String {
    let reserved = matches!(
        name.as_str(),
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
    );
    if reserved { format!("_{name}") } else { name }
}

/// Whether a family's names are written to generated JavaScript as object keys.
fn used_as_javascript_key(kind: IdentifierKind) -> bool {
    matches!(
        kind,
        IdentifierKind::UtilityClass | IdentifierKind::RefMarker | IdentifierKind::ScopeMarker
    )
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
    pub label: Option<String>,
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
/// [`crate::CompilerOptions::naming_revision`] so cached artifacts are not reused
/// across naming changes.
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

///
/// The hash is computed over exactly the request's own fields, so a generator may use it
/// alone and stay correct when it does not care about the individual fields.
pub trait NameRequest {
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
    ) -> Result<GeneratedName, String>;
}

/// Declares one family's request type and its hash rule.
///
/// A request type is what tells the families apart: a ref marker and a keyframes name
/// share their context type but not their family, and a generated layer name has a
/// context type of its own. Naming the family in the type is what keeps the identity
/// text, the kind, and the generator method in one declaration.
macro_rules! name_requests {
    ($(
        $name:ident : $context:ident as $kind:ident = $identity:expr , asks($generator:ident) ;
    )*) => {
        $(
            #[doc = concat!("A ", stringify!($kind), " request.")]
            #[derive(Clone, Debug, Eq, PartialEq)]
            pub struct $name(pub $context);

            impl NameRequest for $name {
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
                ) -> Result<GeneratedName, String> {
                    generator.$generator(&self.0, hash)
                }
            }
        )*
    };
}

name_requests! {
    ClassName: ClassContext as UtilityClass = |context: &ClassContext| format!(
        "stillwind/style/v1:{module}:{module_len}:{namespace_len}:{namespace}\u{0}{branch_len}:{branch}:{call}:{branch}",
        module = context.module,
        module_len = context.module.len(),
        namespace_len = context.namespace.len(),
        namespace = context.namespace,
        branch_len = context.branch.len(),
        branch = context.branch,
        call = context.call,
    ), asks(utility_class);
    RefMarkerName: DeclarationContext as RefMarker = |context: &DeclarationContext| declaration_identity(context, "ref"), asks(ref_marker);
    ScopeMarkerName: DeclarationContext as ScopeMarker = |context: &DeclarationContext| declaration_identity(context, "scope"), asks(scope_marker);
    AnchorName: LocatorContext as AnchorName = |context: &LocatorContext| declaration_identity(context, "ref"), asks(anchor_name);
    ContainerName: LocatorContext as ContainerName = |context: &LocatorContext| declaration_identity(context, "scope"), asks(container_name);
    KeyframesName: DeclarationContext as KeyframesName = |context: &DeclarationContext| declaration_identity(context, "animation"), asks(keyframes_name);
    ViewTransitionName: DeclarationContext as ViewTransitionName = |context: &DeclarationContext| declaration_identity(context, "view-transition"), asks(view_transition_name);
    ViewTransitionClassName: DeclarationContext as ViewTransitionClass = |context: &DeclarationContext| declaration_identity(context, "view-transition-class"), asks(view_transition_class);
    CustomPropertyName: DeclarationContext as CustomProperty = |context: &DeclarationContext| declaration_identity(context, "custom-property"), asks(custom_property);
    LayerName: LayerContext as LayerName = |context: &LayerContext| format!(
        "stillwind/layer/v1:{}:{}",
        context.template,
        context.index,
    ), asks(layer_name);
}

/// Hashes one request's fields to the eight hexadecimal digits a built-in name carries.
fn hashed(fields: &str) -> String {
    let hash = blake3::hash(fields.as_bytes());
    hash.to_hex()[..8].to_owned()
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
///
/// A ref's written anchor name and its marker class are different families of the same
/// declaration, so both read the same fields through this trait.
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

/// What one generator method returned.
///
/// A host either supplies a body or leaves the family to the compiler. Returning a body
/// means the compiler writes that text, with no prefix; returning
/// [`Self::Builtin`] selects the compiler's own name, with its prefix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GeneratedName {
    /// The built-in name for this request.
    Builtin,
    /// A host body, written as returned and normalized for its write positions.
    Body(String),
}

impl GeneratedName {
    /// Wraps `body` as a host body.
    pub fn body(body: impl Into<String>) -> Self {
        Self::Body(body.into())
    }
}

/// A host-provided generator. Every method has the family's own arguments.
///
/// Implementations must be pure: the same request must produce the same name, and the
/// result must not depend on call order, call count, time, randomness, or the build
/// mode. Every method defaults to [`GeneratedName::Builtin`], so overriding one family
/// is a supported use.
///
/// Sharing a generator across threads only forwards calls, because the purity contract
/// already forbids state-dependent output. Generators are still used synchronously
/// within one compilation.
pub trait IdentifierGenerator: fmt::Debug + Send + Sync {
    /// A utility class body, with no built-in class prefix.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this call.
    fn utility_class(&self, _context: &ClassContext, _hash: &str) -> Result<GeneratedName, String> {
        Ok(GeneratedName::Builtin)
    }

    /// A ref marker body, with no built-in marker prefix.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this declaration.
    fn ref_marker(
        &self,
        _context: &DeclarationContext,
        _hash: &str,
    ) -> Result<GeneratedName, String> {
        Ok(GeneratedName::Builtin)
    }

    /// A scope marker body, with no built-in marker prefix.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this declaration.
    fn scope_marker(
        &self,
        _context: &DeclarationContext,
        _hash: &str,
    ) -> Result<GeneratedName, String> {
        Ok(GeneratedName::Builtin)
    }

    /// An anchor-name body, with no built-in custom-property prefix.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this projection.
    fn anchor_name(&self, _context: &LocatorContext, _hash: &str) -> Result<GeneratedName, String> {
        Ok(GeneratedName::Builtin)
    }

    /// A container-name body, with no built-in custom-property prefix.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this projection.
    fn container_name(
        &self,
        _context: &LocatorContext,
        _hash: &str,
    ) -> Result<GeneratedName, String> {
        Ok(GeneratedName::Builtin)
    }

    /// A keyframes body, with no built-in custom-property prefix.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this declaration.
    fn keyframes_name(
        &self,
        _context: &DeclarationContext,
        _hash: &str,
    ) -> Result<GeneratedName, String> {
        Ok(GeneratedName::Builtin)
    }

    /// A view-transition-name body, with no built-in custom-property prefix.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this declaration.
    fn view_transition_name(
        &self,
        _context: &DeclarationContext,
        _hash: &str,
    ) -> Result<GeneratedName, String> {
        Ok(GeneratedName::Builtin)
    }

    /// A view-transition-class body, with no built-in prefix.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this declaration.
    fn view_transition_class(
        &self,
        _context: &DeclarationContext,
        _hash: &str,
    ) -> Result<GeneratedName, String> {
        Ok(GeneratedName::Builtin)
    }

    /// A custom property body, with no leading double hyphen.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this declaration.
    fn custom_property(
        &self,
        _context: &DeclarationContext,
        _hash: &str,
    ) -> Result<GeneratedName, String> {
        Ok(GeneratedName::Builtin)
    }

    /// A layer body, with no built-in prefix.
    ///
    /// # Errors
    ///
    /// Returns a failure the compilation reports for this template.
    fn layer_name(&self, _context: &LayerContext, _hash: &str) -> Result<GeneratedName, String> {
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
/// A rejected name has no source location: the generator produced it, not the source.
/// That is why it is not a lint finding, which always describes authored code, and why
/// its audience is the bundler plugin, which receives the compilation result and can
/// warn with its own position.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RejectedIdentifier {
    /// Family the generator was asked for.
    pub kind: IdentifierKind,
    /// Canonical hash of the request, which identifies the rejected input.
    pub hash: String,
    /// Why the name was rejected: the generator's message, or why a returned body could
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
/// declaration. A rejection is memoized as a rejection, so one compilation reports it
/// once.
#[derive(Clone, Debug)]
pub struct IdentifierContext {
    generators: GeneratorSet,
    /// Compilation mode deciding whether a rejection falls back or fails.
    mode: IdentifierMode,
    memo: RefCell<BTreeMap<MemoKey, Result<Resolved, String>>>,
    /// Rejections collected in development mode.
    rejected: RefCell<RejectedNames>,
}

/// Memo key: the family and the canonical hash of the request.
type MemoKey = (IdentifierKind, String);

/// One resolved name: the text to write and whether it still needs a built-in prefix.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Resolved {
    /// Text the compiler writes.
    body: String,
    /// Whether the write step prepends the built-in prefix.
    builtin: bool,
}

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
    ///
    /// The pipeline moves them into the compilation result, so one compilation reports
    /// each rejected name once.
    pub(crate) fn drain_rejected(&self) -> RejectedNames {
        std::mem::take(&mut self.rejected.borrow_mut())
    }

    /// A utility class token for one call and branch.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn class_name(&self, context: &ClassContext) -> Result<String, CompileErrorKind> {
        self.resolve(&ClassName(context.clone()))
    }

    /// A ref marker class token, without a leading dot.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn ref_marker_name(
        &self,
        context: &DeclarationContext,
    ) -> Result<String, CompileErrorKind> {
        self.resolve(&RefMarkerName(context.clone()))
    }

    /// A scope marker class token, without a leading dot.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn scope_marker_name(
        &self,
        context: &DeclarationContext,
    ) -> Result<String, CompileErrorKind> {
        self.resolve(&ScopeMarkerName(context.clone()))
    }

    /// An anchor name for one ref projection.
    ///
    /// The built-in name is the identity hash alone, whether or not the declaration has a
    /// label: an anchor name is a custom-ident every position-anchor resolves through, so
    /// a label would make renaming it change which elements anchor to which.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn anchor_name(&self, context: &LocatorContext) -> Result<String, CompileErrorKind> {
        self.resolve(&AnchorName(context.clone()))
    }

    /// A container name for one ref projection.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn container_name(
        &self,
        context: &LocatorContext,
    ) -> Result<String, CompileErrorKind> {
        self.resolve(&ContainerName(context.clone()))
    }

    /// A keyframes name, referenced by animation-name.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn animation_name(
        &self,
        context: &DeclarationContext,
    ) -> Result<String, CompileErrorKind> {
        self.resolve(&KeyframesName(context.clone()))
    }

    /// A view-transition name.
    ///
    /// # Errors
    ///
    /// Returns the host generator's failure in release mode.
    pub(crate) fn view_transition_name(
        &self,
        context: &DeclarationContext,
    ) -> Result<String, CompileErrorKind> {
        self.resolve(&ViewTransitionName(context.clone()))
    }

    /// The text one request resolves to, asking the host generator first.
    ///
    /// A host body is written as returned, with no built-in prefix; a built-in name is
    /// prefixed by the write step. A rejected request either fails the compilation or, in
    /// development mode, resolves to the built-in name and is recorded once.
    ///
    /// # Errors
    ///
    /// Returns [`CompileErrorKind::IdentifierGenerationFailed`] for a rejected request in
    /// release mode.
    fn resolve<R: NameRequest>(&self, request: &R) -> Result<String, CompileErrorKind> {
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
    fn generate<R: NameRequest>(&self, request: &R, hash: &str) -> Result<Resolved, String> {
        let Some(generator) = self.generators.generator() else {
            return Ok(Resolved::builtin());
        };
        let generated = request.name(generator, hash);
        match generated {
            Ok(GeneratedName::Builtin) => Ok(Resolved::builtin()),
            Ok(GeneratedName::Body(body)) => {
                let mut canonical = canonicalize(write_positions(request.kind()), &body);
                if used_as_javascript_key(request.kind()) {
                    canonical = javascript_key(canonical);
                }
                if canonical.is_empty() {
                    Err(format!("body {body} is empty once normalized"))
                } else {
                    Ok(Resolved {
                        body: canonical,
                        builtin: false,
                    })
                }
            }
            Err(message) => Err(message),
        }
    }

    /// Turns one generated or failed request into this context's result.
    fn settle(
        &self,
        kind: IdentifierKind,
        hash: &str,
        generated: Result<Resolved, String>,
    ) -> Result<String, CompileErrorKind> {
        match generated {
            Ok(resolved) => Ok(resolved.text(kind, hash)),
            Err(reason) => match self.mode {
                IdentifierMode::Development => {
                    self.rejected.borrow_mut().push(RejectedIdentifier {
                        kind,
                        hash: hash.to_owned(),
                        reason,
                        substituted: true,
                    });
                    Ok(builtin_text(kind, hash))
                }
                IdentifierMode::Release => Err(CompileErrorKind::IdentifierGenerationFailed(
                    kind.as_str().into(),
                    reason.into(),
                )),
            },
        }
    }
}

impl Resolved {
    /// The built-in name for one request.
    fn builtin() -> Self {
        Self {
            body: String::new(),
            builtin: true,
        }
    }

    /// The text to write for this resolution.
    ///
    /// `hash` is the request hash the built-in name is built from.
    fn text(self, kind: IdentifierKind, hash: &str) -> String {
        if self.builtin {
            let name = builtin_text(kind, hash);
            if used_as_javascript_key(kind) {
                javascript_key(name)
            } else {
                name
            }
        } else {
            self.body
        }
    }
}

/// The write positions one family appears in.
///
/// A name may be written more than once; the canonical spelling must be usable in every
/// position, so a character is only kept when every position can hold it.
fn write_positions(kind: IdentifierKind) -> &'static [NamePosition] {
    match kind {
        IdentifierKind::UtilityClass | IdentifierKind::RefMarker | IdentifierKind::ScopeMarker => {
            &[NamePosition::DomToken, NamePosition::Selector]
        }
        IdentifierKind::AnchorName
        | IdentifierKind::ContainerName
        | IdentifierKind::KeyframesName
        | IdentifierKind::ViewTransitionName
        | IdentifierKind::ViewTransitionClass => &[NamePosition::CustomIdent],
        IdentifierKind::CustomProperty => &[NamePosition::CustomProperty],
        IdentifierKind::LayerName => &[NamePosition::LayerName],
    }
}

/// Normalizes a generated body for every position its family is written to.
fn canonicalize(positions: &[NamePosition], name: &str) -> String {
    positions
        .iter()
        .fold(name.to_owned(), |name, position| position.canonical(&name))
}

/// The built-in name for one request, given the request's canonical hash.
///
/// Every family the built-in generator owns carries its prefix here, and no host body
/// passes through this function.
fn builtin_text(kind: IdentifierKind, hash: &str) -> String {
    let prefix = match kind {
        IdentifierKind::UtilityClass => DEFAULT_CLASS_PREFIX,
        IdentifierKind::RefMarker => DEFAULT_REF_MARKER_PREFIX,
        IdentifierKind::ScopeMarker => DEFAULT_SCOPE_MARKER_PREFIX,
        IdentifierKind::AnchorName => DEFAULT_ANCHOR_PREFIX,
        IdentifierKind::ContainerName => DEFAULT_CONTAINER_PREFIX,
        IdentifierKind::KeyframesName => DEFAULT_ANIMATION_PREFIX,
        IdentifierKind::ViewTransitionName => DEFAULT_VIEW_TRANSITION_PREFIX,
        IdentifierKind::ViewTransitionClass
        | IdentifierKind::CustomProperty
        | IdentifierKind::LayerName => "",
    };
    format!("{prefix}{hash}")
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Answers with a fixed body and records every request.
    #[derive(Debug, Default)]
    struct RecordingGenerator {
        calls: Mutex<Vec<(&'static str, String)>>,
    }

    impl RecordingGenerator {
        /// Records one request and answers with the family's fixed body.
        fn record(&self, family: &'static str, hash: &str) -> GeneratedName {
            self.calls
                .lock()
                .expect("test generator lock")
                .push((family, hash.to_owned()));
            GeneratedName::body(format!("fixed-{family}"))
        }
    }

    impl IdentifierGenerator for RecordingGenerator {
        fn utility_class(
            &self,
            _context: &ClassContext,
            hash: &str,
        ) -> Result<GeneratedName, String> {
            Ok(self.record("class", hash))
        }

        fn ref_marker(
            &self,
            _context: &DeclarationContext,
            hash: &str,
        ) -> Result<GeneratedName, String> {
            Ok(self.record("ref-marker", hash))
        }
    }

    /// Fails every family it is asked about.
    #[derive(Debug)]
    struct FailingGenerator;

    impl IdentifierGenerator for FailingGenerator {
        fn ref_marker(
            &self,
            _context: &DeclarationContext,
            _hash: &str,
        ) -> Result<GeneratedName, String> {
            Err("no name for this declaration".to_owned())
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
            label: label.map(marker_label),
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
    fn a_host_body_is_written_without_the_builtin_prefix() {
        let context =
            IdentifierContext::new(GeneratorSet::new(Arc::new(RecordingGenerator::default())));
        let name = context.class_name(&class_context("0:flex")).unwrap();
        assert_eq!(name, "fixed-class");
        assert!(!name.starts_with(DEFAULT_CLASS_PREFIX));
        let builtin = IdentifierContext::builtin()
            .class_name(&class_context("0:flex"))
            .unwrap();
        assert!(builtin.starts_with(DEFAULT_CLASS_PREFIX));
    }

    #[test]
    fn every_builtin_family_keeps_its_own_prefix() {
        let context = IdentifierContext::builtin();
        let declaration = declaration_context(Some("primary"));
        assert!(
            context
                .ref_marker_name(&declaration)
                .unwrap()
                .starts_with(DEFAULT_REF_MARKER_PREFIX)
        );
        assert!(
            context
                .scope_marker_name(&declaration_context(None))
                .unwrap()
                .starts_with(DEFAULT_SCOPE_MARKER_PREFIX)
        );
        assert!(
            context
                .anchor_name(&locator_context(&declaration))
                .unwrap()
                .starts_with(DEFAULT_ANCHOR_PREFIX)
        );
        assert!(
            context
                .container_name(&locator_context(&declaration))
                .unwrap()
                .starts_with(DEFAULT_CONTAINER_PREFIX)
        );
        assert!(
            context
                .animation_name(&declaration)
                .unwrap()
                .starts_with(DEFAULT_ANIMATION_PREFIX)
        );
        assert!(
            context
                .view_transition_name(&declaration)
                .unwrap()
                .starts_with(DEFAULT_VIEW_TRANSITION_PREFIX)
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
        let name = context.ref_marker_name(&declaration_context(None)).unwrap();
        assert!(name.starts_with(DEFAULT_REF_MARKER_PREFIX));
        let rejected = context.drain_rejected();
        assert_eq!(rejected.entries().len(), 1);
        let entry = &rejected.entries()[0];
        assert_eq!(entry.kind, IdentifierKind::RefMarker);
        assert_eq!(entry.reason, "no name for this declaration");
        assert!(entry.substituted);
        assert!(context.drain_rejected().is_empty());
    }

    #[test]
    fn release_mode_fails_on_a_rejected_name() {
        let context = IdentifierContext::new(GeneratorSet::new(Arc::new(FailingGenerator)));
        let failure = context
            .ref_marker_name(&declaration_context(None))
            .unwrap_err();
        assert!(matches!(
            failure,
            CompileErrorKind::IdentifierGenerationFailed(..)
        ));
        assert!(context.drain_rejected().is_empty());
    }

    #[test]
    fn a_body_is_normalized_for_every_position_it_is_written_to() {
        #[derive(Debug)]
        struct TokenGenerator;

        impl IdentifierGenerator for TokenGenerator {
            fn ref_marker(
                &self,
                _context: &DeclarationContext,
                _hash: &str,
            ) -> Result<GeneratedName, String> {
                Ok(GeneratedName::body("fixed token"))
            }

            fn keyframes_name(
                &self,
                _context: &DeclarationContext,
                _hash: &str,
            ) -> Result<GeneratedName, String> {
                Ok(GeneratedName::body("fixed-ok"))
            }
        }

        let context = IdentifierContext::new(GeneratorSet::new(Arc::new(TokenGenerator)));
        assert_eq!(
            context.ref_marker_name(&declaration_context(None)).unwrap(),
            "fixed_token"
        );
        assert_eq!(
            context.animation_name(&declaration_context(None)).unwrap(),
            "fixed-ok"
        );
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
            KeyframesName(declaration.clone()).hash()
        );
        assert_eq!(
            AnchorName(locator_context(&declaration)).hash(),
            RefMarkerName(declaration).hash(),
            "a ref's anchor name and marker share the declaration's identity"
        );
    }

    #[test]
    fn a_class_token_that_an_object_cannot_hold_gains_a_prefix() {
        #[derive(Debug)]
        struct PrototypeGenerator;

        impl IdentifierGenerator for PrototypeGenerator {
            fn ref_marker(
                &self,
                _context: &DeclarationContext,
                _hash: &str,
            ) -> Result<GeneratedName, String> {
                Ok(GeneratedName::body("__proto__"))
            }

            fn keyframes_name(
                &self,
                _context: &DeclarationContext,
                _hash: &str,
            ) -> Result<GeneratedName, String> {
                Ok(GeneratedName::body("__proto__"))
            }
        }

        let context = IdentifierContext::new(GeneratorSet::new(Arc::new(PrototypeGenerator)));
        assert_eq!(
            context.ref_marker_name(&declaration_context(None)).unwrap(),
            "___proto__"
        );
        // A keyframes name is written to CSS only, so it keeps the body it was given.
        assert_eq!(
            context.animation_name(&declaration_context(None)).unwrap(),
            "__proto__"
        );
    }

    #[test]
    fn the_builtin_prefix_already_separates_a_generated_name_from_object_members() {
        let context = IdentifierContext::builtin();
        for name in [
            context.class_name(&class_context("0:flex")).unwrap(),
            context.ref_marker_name(&declaration_context(None)).unwrap(),
            context
                .scope_marker_name(&declaration_context(None))
                .unwrap(),
        ] {
            assert!(
                !javascript_key(name.clone()).starts_with('_') || name.starts_with('_'),
                "{name}"
            );
            assert!(name.starts_with("s_") || name.starts_with("_s_"), "{name}");
        }
    }

    #[test]
    fn positions_replace_what_they_cannot_hold_and_escape_the_rest() {
        assert_eq!(NamePosition::DomToken.canonical("a b\0c"), "a_b_c");
        assert_eq!(NamePosition::Selector.canonical("a\tb"), "a_b");
        assert_eq!(NamePosition::CustomIdent.canonical("a b"), "a b");
        assert_eq!(NamePosition::LayerName.canonical("sw.anon"), "sw_anon");
        assert_eq!(selector("a:b/c"), r".a\:b\/c");
        assert_eq!(custom_ident("a:b"), r"a\:b");
        assert_eq!(NamePosition::CustomProperty.escaped("a:b"), r"--a\:b");
        assert_eq!(NamePosition::LayerName.escaped("sw_anon"), "sw_anon");
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
