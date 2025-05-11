use arrow::array::ArrayRef;

use nohash_hasher::IntMap;
use re_types::{
    Component, ComponentDescriptor, ComponentName, DeserializationError, SerializationError,
};

use crate::QueryContext;

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

// todo: blabla
type ComponentFallbackProviderFn =
    Box<dyn Fn(&QueryContext<'_>) -> Result<ArrayRef, SerializationError> + 'static>;

// todo: blabla
pub struct FallbackProviderRegistry {
    /// Maps exact descriptor to fallback providers.
    exact_fallback_providers: IntMap<ComponentDescriptor, ComponentFallbackProviderFn>,

    /// Maps component names to fallback descriptors, used if there's no matching fallback in `exact_fallback_providers``.
    type_fallback_providers: IntMap<ComponentName, ComponentFallbackProviderFn>,
}

impl FallbackProviderRegistry {
    // todo: blabla
    // todo: awful name!
    pub fn register_type_based_fallback_provider(
        &mut self,
        component_name: ComponentName,
        func: ComponentFallbackProviderFn,
    ) {
        if self
            .exact_fallback_providers
            .insert(component_name, func)
            .is_some()
        {
            re_log::warn!(
                "There was already a component fallback provider registered for {component_name}"
            );
        }
    }

    // todo: blabla
    // todo: awful name!
    pub fn register_typed_type_based_fallback_provider<C: Component>(
        &mut self,
        component_name: ComponentName,
        func: impl Fn(&QueryContext<'_>) -> C + 'static,
    ) {
        self.register_type_based_fallback_provider(
            component_name,
            Box::new(move |query_ctx| {
                let component = func(query_ctx);
                C::to_arrow([std::borrow::Cow::Owned(component)])
            }),
        );
    }

    // todo: blabla
    pub fn register_fallback_provider(
        &mut self,
        component_descr: ComponentDescriptor,
        func: ComponentFallbackProviderFn,
    ) {
        if self
            .exact_fallback_providers
            .insert(component_descr.clone(), func)
            .is_some()
        {
            re_log::warn!(
                "There was already a component fallback provider registered for {component_descr}"
            );
        }
    }

    // todo: blabla
    pub fn register_typed_fallback_provider<C: Component>(
        &mut self,
        component_descr: ComponentDescriptor,
        func: impl Fn(&QueryContext<'_>) -> C + 'static,
    ) {
        self.register_fallback_provider(
            component_descr,
            Box::new(move |query_ctx| {
                let component = func(query_ctx);
                C::to_arrow([std::borrow::Cow::Owned(component)])
            }),
        );
    }

    /// Provides a fallback value for a given component.
    ///
    /// Will attempt to source a fallback by trying the following fallback sources after each other:
    /// * descriptor based fallback provider
    /// * component name based fallback provider
    /// * generic placeholder value
    pub fn fallback_for(
        &self,
        ctx: &QueryContext<'_>,
        component_descr: &ComponentDescriptor,
    ) -> ArrayRef {
        if let Some(exact_fallback_provider) = self.exact_fallback_providers.get(component_descr) {
            match exact_fallback_provider(ctx) {
                Ok(array) => return array,
                Err(err) => {
                    re_log::log_once!(
                        "Failed to deserialize result of fallback provider for {component_descr}: {err}"
                    );
                }
            }
        }

        let component_name = component_descr.component_name;

        if let Some(type_fallback_provider) = self.type_fallback_providers.get(&component_name) {
            match type_fallback_provider(ctx) {
                Ok(array) => return array,
                Err(err) => {
                    re_log::log_once!(
                        "Failed to deserialize result of fallback provider for {component_name}: {err}"
                    );
                }
            }
        }

        ctx.viewer_ctx.placeholder_for(component_name)
    }

    // todo: docs
    pub fn typed_fallback_for<C: re_types::Component>(
        &self,
        ctx: &QueryContext<'_>,
        component_descr: &ComponentDescriptor,
    ) -> Result<Vec<C>, DeserializationError> {
        C::from_arrow(&self.fallback_for(ctx, component_descr))
    }
}

/// Provides fallback values for components, implemented typically by [`crate::ViewClass`] and [`crate::VisualizerSystem`].
///
/// Fallbacks can be based on arbitrarily complex & context sensitive heuristics.
pub trait ComponentFallbackProvider {
    /// Tries to provide a fallback value for a given component.
    ///
    /// If the provider can't handle the component or simply want to use a placeholder value,
    /// it should return [`ComponentFallbackProviderResult::ComponentNotHandled`].
    ///
    /// Fallbacks can be based on arbitrarily complex & context sensitive heuristics.
    fn try_provide_fallback(
        &self,
        ctx: &QueryContext<'_>,
        component: ComponentName,
    ) -> ComponentFallbackProviderResult;

    /// Provides a fallback value for a given component, first trying the provider and
    /// then falling back to the placeholder value registered in the viewer context.
    fn fallback_for(&self, ctx: &QueryContext<'_>, component: ComponentDescriptor) -> ArrayRef {
        match self.try_provide_fallback(ctx, component) {
            ComponentFallbackProviderResult::Value(value) => {
                return value;
            }
            ComponentFallbackProviderResult::SerializationError(err) => {
                // We still want to provide the base fallback value so we can move on,
                // but arrow serialization should never fail.
                // Giving out _both_ the error and the fallback value gets messy,
                // so given that this should be a rare bug, we log it and return the fallback value as success.
                re_log::error_once!(
                    "Arrow serialization failed trying to provide a fallback for {component}. Using base fallback instead: {err}"
                );
            }
            ComponentFallbackProviderResult::ComponentNotHandled => {}
        }

        ctx.viewer_ctx.placeholder_for(component)
    }
}

/// Provides a fallback value for a given component with known type.
///
/// Use the [`crate::impl_component_fallback_provider`] macro to build a [`ComponentFallbackProvider`]
/// out of several strongly typed [`TypedComponentFallbackProvider`]s.
pub trait TypedComponentFallbackProvider<C: re_types::Component> {
    fn fallback_for(&self, ctx: &QueryContext<'_>, component_descriptor: &ComponentDescriptor)
    -> C;
}

/// Implements the [`ComponentFallbackProvider`] trait for a given type, using a number of [`TypedComponentFallbackProvider`].
///
/// Usage examples:
/// ```ignore
/// impl_component_fallback_provider!(MySystem => []);              // Empty fallback provider
/// impl_component_fallback_provider!(MySystem => [Color, Text]);   // Fallback provider handling the Color and Text components.
/// ```
#[macro_export]
macro_rules! impl_component_fallback_provider {
    ($type:ty => [$($component:ty),*]) => {
        impl $crate::ComponentFallbackProvider for $type {
            fn try_provide_fallback(
                &self,
                _ctx: &$crate::QueryContext<'_>,
                _component_descriptor: &re_types::ComponentDescriptor,
            ) -> $crate::ComponentFallbackProviderResult {
                $(
                    if _component_name == <$component as re_types::Component>::name() {
                        return  $crate::TypedComponentFallbackProvider::<$component>::fallback_for(self, _ctx).into();
                    }
                )*
                $crate::ComponentFallbackProviderResult::ComponentNotHandled
            }
        }
    };
}
