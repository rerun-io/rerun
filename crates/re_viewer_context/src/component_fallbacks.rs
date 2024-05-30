use ahash::HashMap;
use re_types::{external::arrow2, ComponentName};

use crate::QueryContext;

/// Lookup table for component base fallbacks.
///
/// Base fallbacks are the default values for components that are used when no other context specific fallback is available.
pub type ComponentBaseFallbacks = HashMap<ComponentName, Box<dyn arrow2::array::Array>>;

/// Result for a fallback request to a provider.
pub enum ComponentFallbackProviderResult {
    /// A fallback value was successfully provided.
    Value(Box<dyn arrow2::array::Array>),

    /// The fallback provider is not able to handle the given component.
    ///
    /// This is not treated as an error and should be handled by looking up a base fallback.
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

/// Result for a fallback request.
pub enum ComponentFallbackError {
    /// The fallback provider is not able to handle the given component _and_ there was no base fallback.
    /// This should never happen, since all components should have a base fallback.
    MissingBaseFallback,
}

// TODO: Docs
pub trait ComponentFallbackProvider {
    /// Tries to provide a fallback value for a given component.
    ///
    /// If the provider can't handle the component or simply want to use a base fallback,
    /// it should return `ComponentFallbackProviderResult::ComponentNotHandled`.
    ///
    /// Fallbacks can be based on arbitrarily complex & context sensitive heuristics.
    fn try_provide_fallback(
        &self,
        ctx: &QueryContext<'_>,
        component: ComponentName,
    ) -> ComponentFallbackProviderResult;

    /// Provides a fallback value for a given component, first trying the provider and then falling back to the base fallbacks.
    fn fallback_for(
        &self,
        ctx: &QueryContext<'_>,
        component: ComponentName,
    ) -> Result<Box<dyn arrow2::array::Array>, ComponentFallbackError> {
        match self.try_provide_fallback(ctx, component) {
            ComponentFallbackProviderResult::Value(value) => {
                return Ok(value);
            }
            ComponentFallbackProviderResult::SerializationError(err) => {
                // We still want to provide the base fallback value so we can move on,
                // but arrow serialization should never fail.
                // Giving out _both_ the error and the fallback value gets messy,
                // so given that this should be a rare bug, we log it and return the fallback value as success.
                re_log::error_once!("Arrow serialization failed trying to provide a fallback for {:?}. Using base fallback instead: {}", component, err);
            }
            ComponentFallbackProviderResult::ComponentNotHandled => {}
        }

        match ctx.viewer_ctx.component_base_fallbacks.get(&component) {
            Some(fallback) => Ok(fallback.clone()),
            None => Err(ComponentFallbackError::MissingBaseFallback),
        }
    }
}

// TODO: Docs
pub trait TypedComponentFallbackProvider<C: re_types::Component> {
    fn fallback_value(&self, ctx: &QueryContext<'_>) -> C;
}

/// Implements the [`ComponentFallbackProvider`] trait for a given type, using a number of [`TypedComponentFallbackProvider`].
#[macro_export]
macro_rules! impl_component_fallback_provider {
    ($type:ty => [$($component:ty),*]) => {
        impl $crate::ComponentFallbackProvider for $type {
            fn try_provide_fallback(
                &self,
                ctx: &$crate::QueryContext<'_>,
                component_name: re_types::ComponentName,
            ) -> $crate::ComponentFallbackProviderResult {
                $(
                    if component_name == <$component as re_types::Loggable>::name() {
                        return  $crate::TypedComponentFallbackProvider::<$component>::fallback_value(self, ctx).into();
                    }
                )*
                $crate::ComponentFallbackProviderResult::ComponentNotHandled
            }
        }
    };
}
