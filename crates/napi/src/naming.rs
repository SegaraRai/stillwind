//! JavaScript-facing name generators.
//!
//! A host may name one family per function. Every function is optional, and a function returns the
//! name to write or `undefined` to keep the compiler's own name. Implementations must be pure: the
//! same request has to produce the same name, and the result may not depend on call order, call
//! count, time, or randomness.
//!
//! The functions are JavaScript values, so they are callable only on the thread that owns them and
//! only while a host call is running. [`NativeNaming`] holds the persistent references and marks
//! them callable for the duration of one call, which is where the compiler asks for names.

#![allow(clippy::type_complexity)] // Every field is one JavaScript function signature.

use std::cell::Cell;
use std::fmt;
use std::sync::Arc;

use napi::bindgen_prelude::{Buffer, Env, FnArgs, Function, FunctionRef};
use napi::sys;
use napi_derive::napi;

use stillwind_compiler::{
    AnchorName, ClassContext, ConditionContext, ConditionHelperName, ContainerName,
    DeclarationContext, GeneratedName, IdentifierGenerator, IdentifierHash, KeyframesName,
    LayerContext, LayerName, LocatorContext, RefMarker, UtilityClass, ViewTransitionClass,
    ViewTransitionSlotName,
};

/// The fields of a utility class request.
#[napi(object)]
pub struct NativeClassContext {
    /// Module identity of the call site.
    pub module: String,
    /// The caller's class namespace.
    pub namespace: String,
    /// Byte offset of the call.
    pub call: u32,
    /// Branch discriminator and canonical spelling of the contribution.
    pub branch: String,
}

/// The fields of a named declaration request.
#[napi(object)]
pub struct NativeDeclarationContext {
    /// Module identity of the declaring module.
    pub module: String,
    /// Byte offset of the first character of the declaration's name.
    pub start: u32,
    /// Byte offset just past the declaration's name.
    pub end: u32,
    /// Binding name the declaration is assigned to.
    pub binding: String,
    /// Authored label, when the declaration has one.
    pub label: Option<String>,
}

/// The fields of a projection's custom-ident request.
#[napi(object)]
pub struct NativeLocatorContext {
    /// Module identity of the declaration that owns the name.
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

/// The fields of an anonymous layer request.
#[napi(object)]
pub struct NativeLayerContext {
    /// Identity of the template the layer belongs to.
    pub template: String,
    /// The caller's class namespace.
    pub namespace: String,
}

/// The fields of a conditional helper request.
#[napi(object)]
pub struct NativeConditionContext {
    /// Which helper the name is written for: `rule`, `fallback`, `gate`, `gate-value`, or
    /// `gate-not`.
    pub role: String,
    /// The CSS the helper serves.
    pub text: String,
}

/// The persistent reference a session keeps for one family's function.
type NamingReference<Context> = FunctionRef<FnArgs<(Context, Buffer)>, Option<String>>;

/// The persistent reference for the helper family.
type HelperReference =
    FunctionRef<FnArgs<(NativeConditionContext, Buffer, Buffer)>, Option<String>>;

/// One host generator function per family, all optional.
#[napi(object)]
pub struct NativeIdentifierNaming<'env> {
    /// `(context, hash) => string | undefined`, for a utility class.
    pub utility_class: Option<Function<'env, FnArgs<(NativeClassContext, Buffer)>, Option<String>>>,
    /// `(context, hash) => string | undefined`, for a ref marker class.
    pub ref_marker:
        Option<Function<'env, FnArgs<(NativeDeclarationContext, Buffer)>, Option<String>>>,
    /// `(context, hash) => string | undefined`, for an `anchor-name` custom-ident.
    pub anchor_name: Option<Function<'env, FnArgs<(NativeLocatorContext, Buffer)>, Option<String>>>,
    /// `(context, hash) => string | undefined`, for a `container-name` custom-ident.
    pub container_name:
        Option<Function<'env, FnArgs<(NativeLocatorContext, Buffer)>, Option<String>>>,
    /// `(context, hash) => string | undefined`, for a `@keyframes` name.
    pub keyframes_name:
        Option<Function<'env, FnArgs<(NativeDeclarationContext, Buffer)>, Option<String>>>,
    /// `(context, hash) => string | undefined`, for a `view-transition-class` custom-ident,
    /// which is also the transition's identity.
    pub view_transition_class:
        Option<Function<'env, FnArgs<(NativeDeclarationContext, Buffer)>, Option<String>>>,
    /// `(context, hash) => string | undefined`, for the custom property one guarded
    /// view-transition use writes its class token into.
    pub view_transition_slot:
        Option<Function<'env, FnArgs<(NativeClassContext, Buffer)>, Option<String>>>,
    /// `(context, hash, text_hash) => string | undefined`, for a conditional helper custom
    /// property. `hash` covers the role and the text, `text_hash` the text alone.
    pub condition_helper:
        Option<Function<'env, FnArgs<(NativeConditionContext, Buffer, Buffer)>, Option<String>>>,
    /// `(context, hash) => string | undefined`, for an anonymous layer name without its index.
    pub layer_name: Option<Function<'env, FnArgs<(NativeLayerContext, Buffer)>, Option<String>>>,
}

/// One host's generators, with the references that outlive the call that created them.
pub(crate) struct NativeNaming {
    /// Thread and lifetime the functions belong to, set for one host call at a time.
    env: Cell<Option<sys::napi_env>>,
    utility_class: Option<NamingReference<NativeClassContext>>,
    ref_marker: Option<NamingReference<NativeDeclarationContext>>,
    anchor_name: Option<NamingReference<NativeLocatorContext>>,
    container_name: Option<NamingReference<NativeLocatorContext>>,
    keyframes_name: Option<NamingReference<NativeDeclarationContext>>,
    view_transition_class: Option<NamingReference<NativeDeclarationContext>>,
    view_transition_slot: Option<NamingReference<NativeClassContext>>,
    condition_helper: Option<HelperReference>,
    layer_name: Option<NamingReference<NativeLayerContext>>,
}

impl NativeNaming {
    /// Keeps the functions `naming` supplies, or returns `None` when it supplies none.
    pub(crate) fn from_js(
        naming: Option<NativeIdentifierNaming<'_>>,
    ) -> napi::Result<Option<Arc<Self>>> {
        let Some(naming) = naming else {
            return Ok(None);
        };
        let naming = Self {
            env: Cell::new(None),
            utility_class: reference(naming.utility_class)?,
            ref_marker: reference(naming.ref_marker)?,
            anchor_name: reference(naming.anchor_name)?,
            container_name: reference(naming.container_name)?,
            keyframes_name: reference(naming.keyframes_name)?,
            view_transition_class: reference(naming.view_transition_class)?,
            view_transition_slot: reference(naming.view_transition_slot)?,
            condition_helper: reference(naming.condition_helper)?,
            layer_name: reference(naming.layer_name)?,
        };
        // A host generator that only its own thread can call keeps this sharing single-threaded by
        // construction: a compilation never moves its naming inputs across threads.
        #[allow(clippy::arc_with_non_send_sync)]
        let naming = Arc::new(naming);
        Ok((!naming.is_empty()).then_some(naming))
    }

    /// Whether the host supplied no function at all.
    fn is_empty(&self) -> bool {
        self.utility_class.is_none()
            && self.ref_marker.is_none()
            && self.anchor_name.is_none()
            && self.container_name.is_none()
            && self.keyframes_name.is_none()
            && self.view_transition_class.is_none()
            && self.view_transition_slot.is_none()
            && self.condition_helper.is_none()
            && self.layer_name.is_none()
    }

    /// Makes the functions callable until the returned guard is dropped.
    pub(crate) fn enter<'a>(self: &'a Arc<Self>, env: &Env) -> EnteredNaming<'a> {
        self.env.set(Some(env.raw()));
        EnteredNaming { naming: self }
    }

    /// Calls one host function, reading the arguments as the family expects them.
    fn call<Args>(
        &self,
        function: Option<&FunctionRef<Args, Option<String>>>,
        args: Args,
    ) -> Result<Option<String>, String>
    where
        Args: napi::bindgen_prelude::JsValuesTupleIntoVec,
    {
        let Some(function) = function else {
            return Ok(None);
        };
        let Some(raw) = self.env.get() else {
            return Err(
                "a host name generator was asked for a name outside a host call".to_owned(),
            );
        };
        let env = Env::from_raw(raw);
        let function = function.borrow_back(&env).map_err(reason)?;
        function.call(args).map_err(reason)
    }
}

/// The message one rejected host call reports, without the N-API status prefix.
fn reason(error: napi::Error) -> String {
    if error.reason.is_empty() {
        error.status.to_string()
    } else {
        error.reason
    }
}

impl fmt::Debug for NativeNaming {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("NativeNaming")
    }
}

/// Marks the functions callable for one call and clears the mark on drop.
pub(crate) struct EnteredNaming<'a> {
    naming: &'a NativeNaming,
}

impl Drop for EnteredNaming<'_> {
    fn drop(&mut self) {
        self.naming.env.set(None);
    }
}

/// The hash a host function receives, as the whole digest.
fn digest(hash: &IdentifierHash) -> Buffer {
    Buffer::from(hash.as_bytes().as_slice())
}

/// Keeps one host function for as long as the session lives.
fn reference<Args, Return>(
    function: Option<Function<'_, Args, Return>>,
) -> napi::Result<Option<FunctionRef<Args, Return>>>
where
    Args: napi::bindgen_prelude::JsValuesTupleIntoVec,
{
    match function {
        Some(function) => function.create_ref().map(Some),
        None => Ok(None),
    }
}

impl IdentifierGenerator for NativeNaming {
    fn utility_class(
        &self,
        context: &ClassContext,
        hash: &IdentifierHash,
    ) -> Result<GeneratedName<UtilityClass>, String> {
        let Some(name) = self.call(
            self.utility_class.as_ref(),
            FnArgs::from((
                NativeClassContext {
                    module: context.module.as_str().to_owned(),
                    namespace: context.namespace.clone(),
                    call: context.call,
                    branch: context.branch.clone(),
                },
                digest(hash),
            )),
        )?
        else {
            return Ok(GeneratedName::Builtin);
        };
        Ok(GeneratedName::provided(
            UtilityClass::new(&name).map_err(|error| error.to_string())?,
        ))
    }

    fn ref_marker(
        &self,
        context: &DeclarationContext,
        hash: &IdentifierHash,
    ) -> Result<GeneratedName<RefMarker>, String> {
        let Some(name) = self.call(
            self.ref_marker.as_ref(),
            FnArgs::from((declaration(context), digest(hash))),
        )?
        else {
            return Ok(GeneratedName::Builtin);
        };
        Ok(GeneratedName::provided(
            RefMarker::new(&name).map_err(|error| error.to_string())?,
        ))
    }

    fn anchor_name(
        &self,
        context: &LocatorContext,
        hash: &IdentifierHash,
    ) -> Result<GeneratedName<AnchorName>, String> {
        let Some(name) = self.call(
            self.anchor_name.as_ref(),
            FnArgs::from((locator(context), digest(hash))),
        )?
        else {
            return Ok(GeneratedName::Builtin);
        };
        Ok(GeneratedName::provided(
            AnchorName::new(&name).map_err(|error| error.to_string())?,
        ))
    }

    fn container_name(
        &self,
        context: &LocatorContext,
        hash: &IdentifierHash,
    ) -> Result<GeneratedName<ContainerName>, String> {
        let Some(name) = self.call(
            self.container_name.as_ref(),
            FnArgs::from((locator(context), digest(hash))),
        )?
        else {
            return Ok(GeneratedName::Builtin);
        };
        Ok(GeneratedName::provided(
            ContainerName::new(&name).map_err(|error| error.to_string())?,
        ))
    }

    fn keyframes_name(
        &self,
        context: &DeclarationContext,
        hash: &IdentifierHash,
    ) -> Result<GeneratedName<KeyframesName>, String> {
        let Some(name) = self.call(
            self.keyframes_name.as_ref(),
            FnArgs::from((declaration(context), digest(hash))),
        )?
        else {
            return Ok(GeneratedName::Builtin);
        };
        Ok(GeneratedName::provided(
            KeyframesName::new(&name).map_err(|error| error.to_string())?,
        ))
    }

    fn view_transition_class(
        &self,
        context: &DeclarationContext,
        hash: &IdentifierHash,
    ) -> Result<GeneratedName<ViewTransitionClass>, String> {
        let Some(name) = self.call(
            self.view_transition_class.as_ref(),
            FnArgs::from((declaration(context), digest(hash))),
        )?
        else {
            return Ok(GeneratedName::Builtin);
        };
        Ok(GeneratedName::provided(
            ViewTransitionClass::new(&name).map_err(|error| error.to_string())?,
        ))
    }

    fn view_transition_slot(
        &self,
        context: &ClassContext,
        hash: &IdentifierHash,
    ) -> Result<GeneratedName<ViewTransitionSlotName>, String> {
        let Some(name) = self.call(
            self.view_transition_slot.as_ref(),
            FnArgs::from((
                NativeClassContext {
                    module: context.module.as_str().to_owned(),
                    namespace: context.namespace.clone(),
                    call: context.call,
                    branch: context.branch.clone(),
                },
                digest(hash),
            )),
        )?
        else {
            return Ok(GeneratedName::Builtin);
        };
        Ok(GeneratedName::provided(
            ViewTransitionSlotName::new(&name).map_err(|error| error.to_string())?,
        ))
    }

    fn condition_helper(
        &self,
        context: &ConditionContext,
        hash: &IdentifierHash,
        text_hash: &IdentifierHash,
    ) -> Result<GeneratedName<ConditionHelperName>, String> {
        let Some(name) = self.call(
            self.condition_helper.as_ref(),
            FnArgs::from((
                NativeConditionContext {
                    role: context.role.as_str().to_owned(),
                    text: context.text.clone(),
                },
                digest(hash),
                digest(text_hash),
            )),
        )?
        else {
            return Ok(GeneratedName::Builtin);
        };
        Ok(GeneratedName::provided(
            ConditionHelperName::new(&name).map_err(|error| error.to_string())?,
        ))
    }

    fn layer_name(
        &self,
        context: &LayerContext,
        hash: &IdentifierHash,
    ) -> Result<GeneratedName<LayerName>, String> {
        let Some(name) = self.call(
            self.layer_name.as_ref(),
            FnArgs::from((
                NativeLayerContext {
                    template: context.template.as_str().to_owned(),
                    namespace: context.namespace.clone(),
                },
                digest(hash),
            )),
        )?
        else {
            return Ok(GeneratedName::Builtin);
        };
        Ok(GeneratedName::provided(
            LayerName::new(&name).map_err(|error| error.to_string())?,
        ))
    }
}

/// The fields of one declaration request, as the host sees them.
fn declaration(context: &DeclarationContext) -> NativeDeclarationContext {
    NativeDeclarationContext {
        module: context.module.as_str().to_owned(),
        start: context.start,
        end: context.end,
        binding: context.binding.clone(),
        label: context
            .label
            .as_ref()
            .map(|label| label.as_str().to_owned()),
    }
}

/// The fields of one projection request, as the host sees them.
fn locator(context: &LocatorContext) -> NativeLocatorContext {
    NativeLocatorContext {
        module: context.module.as_str().to_owned(),
        start: context.start,
        end: context.end,
        binding: context.binding.clone(),
        projection: context.projection.clone(),
    }
}
