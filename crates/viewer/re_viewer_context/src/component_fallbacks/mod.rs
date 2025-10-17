use std::borrow::Cow;

use ahash::HashMap;
use arrow::array::{ArrayRef, NullArray};

use nohash_hasher::IntMap;
use re_chunk::ComponentIdentifier;
use re_types::{Component, ComponentDescriptor, ComponentType, SerializationError};

use crate::{QueryContext, ViewerContext};

mod default_component_fallbacks;

pub fn create_component_fallback_registry() -> FallbackProviderRegistry {
    let mut registry = FallbackProviderRegistry::default();

    default_component_fallbacks::register_component_type_defaults(&mut registry);

    default_component_fallbacks::register_component_identifier_defaults(&mut registry);

    registry
}

pub trait FallbackContext: 'static + Send + Sync + std::any::Any {
    fn fallback_for(
        &self,
        query_context: &QueryContext<'_>,
        component_descr: &ComponentDescriptor,
    ) -> ArrayRef;
}

/// Tries to get a fallback for the type `C`.
pub fn typed_fallback_for<C: Component>(
    query_context: &QueryContext<'_>,
    fallback_ctx: &dyn FallbackContext,
    component_descr: &ComponentDescriptor,
) -> C {
    debug_assert_eq!(
        Some(C::name()),
        component_descr.component_type,
        "Passed component descriptor doesn't match generic arg `C`"
    );

    let array = fallback_ctx.fallback_for(query_context, component_descr);

    let Some(v) = C::from_arrow(&array)
        .ok()
        .and_then(|v| v.into_iter().next())
    else {
        panic!(
            "Missing fallback provider for `{}`",
            std::any::type_name::<C>()
        );
    };

    v
}

impl<T: 'static + Send + Sync + std::any::Any> FallbackContext for T {
    fn fallback_for(
        &self,
        query_context: &QueryContext<'_>,
        component_descr: &ComponentDescriptor,
    ) -> ArrayRef {
        query_context
            .viewer_ctx()
            .component_fallback_registry
            .fallback_for(self, component_descr, query_context)
    }
}

/// Result for a fallback request to a provider.
pub enum ComponentFallbackProviderResult {
    /// A fallback value was successfully provided.
    Value(ArrayRef),

    /// The fallback provider is not able to handle the given component.
    ///
    /// This is not treated as an error and should be handled by looking up a placeholder value.
    ComponentNotHandled,

    /// Arrow serialization failed.
    ///
    /// Unlike [`ComponentFallbackProviderResult::ComponentNotHandled`], this is treated as an unexpected error.
    SerializationError(re_types::SerializationError),
}

impl<T: re_types::ComponentBatch> From<T> for ComponentFallbackProviderResult {
    fn from(batch: T) -> Self {
        match batch.to_arrow() {
            Ok(value) => Self::Value(value),
            Err(err) => Self::SerializationError(err),
        }
    }
}

/// Error type for a fallback request.
#[derive(thiserror::Error, Debug)]
pub enum ComponentFallbackError {
    /// Not directly returned by the fallback provider, but useful when serializing a fallback value.
    #[error("Fallback value turned up to be empty when we expected a value.")]
    UnexpectedEmptyFallback,
}

type ComponentFallbackProviderFn =
    Box<dyn Fn(&QueryContext<'_>) -> Result<ArrayRef, SerializationError> + Send + Sync + 'static>;

type ComponentViewFallbackProviderFn = Box<
    dyn Fn(&dyn std::any::Any, &QueryContext<'_>) -> Result<ArrayRef, SerializationError>
        + Send
        + Sync
        + 'static,
>;

/// A registry to handle component fallbacks.
///
/// This has two layers of fallbacks. The first one being for specific [`ComponentDescriptor`]s,
/// i.e certain fields in archetypes. And the second being for [`ComponentTypes`]s.
#[derive(Default)]
pub struct FallbackProviderRegistry {
    view_component_fallback_providers:
        HashMap<(std::any::TypeId, ComponentIdentifier), ComponentViewFallbackProviderFn>,

    /// Maps component identifier to fallback providers.
    component_fallback_providers: IntMap<ComponentIdentifier, ComponentFallbackProviderFn>,

    /// Maps component types to fallback descriptors, used if there's no matching
    /// fallback in `exact_fallback_providers`.
    type_fallback_providers: IntMap<ComponentType, ComponentFallbackProviderFn>,
}

impl FallbackProviderRegistry {
    /// Registers a fallback provider function for a given component type.
    ///
    /// The function is expected to return the correct type for the given
    /// component type.
    pub fn register_dyn_type_fallback_provider(
        &mut self,
        component: ComponentType,
        provider: ComponentFallbackProviderFn,
    ) {
        if self
            .type_fallback_providers
            .insert(component, provider)
            .is_some()
        {
            re_log::warn!(
                "There was already a component fallback provider registered for {component}"
            );
        }
    }

    /// Registers a fallback provider function for a given component type.
    pub fn register_type_fallback_provider<C: re_types::Component>(
        &mut self,
        f: impl Fn(&QueryContext<'_>) -> C + Send + Sync + 'static,
    ) {
        self.register_dyn_type_fallback_provider(
            C::name(),
            Box::new(move |query_context| {
                let value = f(query_context);

                C::to_arrow([Cow::Owned(value)])
            }),
        );
    }

    /// Registers a fallback provider for a given component type based on
    /// [`Default::default`].
    pub fn register_default_type_fallback_provider<C>(&mut self)
    where
        C: re_types::Component + Default,
    {
        self.register_type_fallback_provider(|_| C::default());
    }

    /// Registers a fallback provider function for a given component identifier.
    pub fn register_dyn_fallback_provider(
        &mut self,
        identifier: ComponentIdentifier,
        provider: ComponentFallbackProviderFn,
    ) {
        if self
            .component_fallback_providers
            .insert(identifier, provider)
            .is_some()
        {
            re_log::warn!(
                "There was already a component fallback provider registered for {identifier}"
            );
        }
    }

    /// Registers a fallback provider function for a given component identifier.
    pub fn register_fallback_provider<C: re_types::Component>(
        &mut self,
        descriptor: &ComponentDescriptor,
        provider: impl Fn(&QueryContext<'_>) -> C + Send + Sync + 'static,
    ) {
        debug_assert_eq!(descriptor.component_type, Some(C::name()));

        self.register_dyn_fallback_provider(
            descriptor.component,
            Box::new(move |query_context| {
                let value = provider(query_context);

                C::to_arrow([Cow::Owned(value)])
            }),
        );
    }

    /// Registers a fallback provider function for a given component identifier
    /// in a specific view.
    pub fn register_dyn_view_fallback_provider(
        &mut self,
        ctx_type: std::any::TypeId,
        component: ComponentIdentifier,
        provider: ComponentViewFallbackProviderFn,
    ) {
        if self
            .view_component_fallback_providers
            .insert((ctx_type, component), provider)
            .is_some()
        {
            re_log::warn!(
                "There was already a view component fallback provider registered for {component} in {ctx_type:?}"
            );
        }
    }

    /// Registers a fallback provider function for a given component identifier
    /// in a specific view.
    pub fn register_context_fallback_provider<T: FallbackContext, C: re_types::Component>(
        &mut self,
        descriptor: &ComponentDescriptor,
        provider: impl Fn(&T, &QueryContext<'_>) -> C + Send + Sync + 'static,
    ) {
        debug_assert_eq!(descriptor.component_type, Some(C::name()));

        self.register_dyn_view_fallback_provider(
            std::any::TypeId::of::<T>(),
            descriptor.component,
            Box::new(move |view, query_context| {
                let Some(view) = view.downcast_ref() else {
                    re_log::error_once!("Failed to get fallback provider because passed view is not the expected type");

                    return Ok(C::arrow_empty());
                };

                let value = provider(view, query_context);

                C::to_arrow([Cow::Owned(value)])
            }),
        );
    }

    pub fn try_fallback_for(
        &self,
        fallback_ctx: &dyn FallbackContext,
        descriptor: &ComponentDescriptor,
        ctx: &QueryContext<'_>,
    ) -> ComponentFallbackProviderResult {
        let res = self
            .view_component_fallback_providers
            .get(&(fallback_ctx.type_id(), descriptor.component))
            .map(|f| f(fallback_ctx, ctx))
            .or_else(|| {
                let f = self
                    .component_fallback_providers
                    .get(&descriptor.component)
                    .or_else(|| {
                        let ty = descriptor.component_type?;
                        self.type_fallback_providers.get(&ty)
                    })?;

                Some(f(ctx))
            });

        match res {
            Some(Ok(v)) => ComponentFallbackProviderResult::Value(v),
            Some(Err(err)) => ComponentFallbackProviderResult::SerializationError(err),
            None => ComponentFallbackProviderResult::ComponentNotHandled,
        }
    }

    /// Provides a fallback value for a given component, first trying the provider and
    /// then falling back to the placeholder value registered in the viewer context.
    pub fn fallback_for(
        &self,
        fallback_ctx: &dyn FallbackContext,
        descriptor: &ComponentDescriptor,
        ctx: &QueryContext<'_>,
    ) -> ArrayRef {
        match self.try_fallback_for(fallback_ctx, descriptor, ctx) {
            ComponentFallbackProviderResult::Value(array) => return array,
            ComponentFallbackProviderResult::ComponentNotHandled => {}
            ComponentFallbackProviderResult::SerializationError(err) => {
                // We still want to provide the base fallback value so we can move on,
                // but arrow serialization should never fail.
                // Giving out _both_ the error and the fallback value gets messy,
                // so given that this should be a rare bug, we log it and return the fallback value as success.
                re_log::error_once!(
                    "Arrow serialization failed trying to provide a fallback for {}. Using base fallback instead: {err}",
                    descriptor
                );
            }
        }

        if let Some(ty) = descriptor.component_type {
            placeholder_for(ctx.viewer_ctx(), ty)
        } else {
            re_log::warn_once!(
                "Requested fallback for component descr {descriptor} without component type"
            );
            std::sync::Arc::new(NullArray::new(0))
        }
    }
}

/// Returns a placeholder value for a given component, solely identified by its type.
///
/// A placeholder is an array of the component type with a single element which takes on some default value.
/// It can be set as part of the reflection information, see [`re_types_core::reflection::ComponentReflection::custom_placeholder`].
/// Note that automatically generated placeholders ignore any extension types.
///
/// This requires the component type to be known by either datastore or blueprint store and
/// will return a placeholder for a nulltype otherwise, logging an error.
/// The rationale is that to get into this situation, we need to know of a component type for which
/// we don't have a datatype, meaning that we can't make any statement about what data this component should represent.
// TODO(andreas): Are there cases where this is expected and how to handle this?
fn placeholder_for(viewer_ctx: &ViewerContext<'_>, component: re_chunk::ComponentType) -> ArrayRef {
    let datatype = if let Some(reflection) = viewer_ctx.reflection().components.get(&component) {
        // It's a builtin type with reflection. We either have custom place holder, or can rely on the known datatype.
        if let Some(placeholder) = reflection.custom_placeholder.as_ref() {
            return placeholder.clone();
        }
        reflection.datatype.clone()
    } else {
        viewer_ctx.recording_engine()
                .store()
                .lookup_datatype(&component)
                .or_else(|| viewer_ctx.blueprint_engine().store().lookup_datatype(&component))
                .unwrap_or_else(|| {
                         re_log::error_once!("Could not find datatype for component {component}. Using null array as placeholder.");
                                    arrow::datatypes::DataType::Null})
    };

    // TODO(andreas): Is this operation common enough to cache the result? If so, here or in the reflection data?
    // The nice thing about this would be that we could always give out references (but updating said cache wouldn't be easy in that case).
    re_types::reflection::generic_placeholder_for_datatype(&datatype)
}
