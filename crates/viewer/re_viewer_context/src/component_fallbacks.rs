use arrow::array::{ArrayRef, NullArray};

use re_types::{ComponentDescriptor, ComponentType};

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
        component: ComponentType,
    ) -> ComponentFallbackProviderResult;

    /// Provides a fallback value for a given component, first trying the provider and
    /// then falling back to the placeholder value registered in the viewer context.
    fn fallback_for(
        &self,
        ctx: &QueryContext<'_>,
        component_descr: &ComponentDescriptor,
    ) -> ArrayRef {
        let Some(component_type) = component_descr.component_type else {
            re_log::warn!(
                "Requested fallback for component descr {component_descr} without component type"
            );
            return std::sync::Arc::new(NullArray::new(0));
        };

        match self.try_provide_fallback(ctx, component_type) {
            ComponentFallbackProviderResult::Value(value) => {
                return value;
            }
            ComponentFallbackProviderResult::SerializationError(err) => {
                // We still want to provide the base fallback value so we can move on,
                // but arrow serialization should never fail.
                // Giving out _both_ the error and the fallback value gets messy,
                // so given that this should be a rare bug, we log it and return the fallback value as success.
                re_log::error_once!(
                    "Arrow serialization failed trying to provide a fallback for {component_type}. Using base fallback instead: {err}"
                );
            }
            ComponentFallbackProviderResult::ComponentNotHandled => {}
        }

        ctx.viewer_ctx().placeholder_for(component_type)
    }
}

/// Provides a fallback value for a given component with known type.
///
/// Use the [`crate::impl_component_fallback_provider`] macro to build a [`ComponentFallbackProvider`]
/// out of several strongly typed [`TypedComponentFallbackProvider`]s.
pub trait TypedComponentFallbackProvider<C: re_types::Component> {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> C;
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
                _component_type: re_types::ComponentType,
            ) -> $crate::ComponentFallbackProviderResult {
                $(
                    if _component_type == <$component as re_types::Component>::name() {
                        return  $crate::TypedComponentFallbackProvider::<$component>::fallback_for(self, _ctx).into();
                    }
                )*
                $crate::ComponentFallbackProviderResult::ComponentNotHandled
            }
        }
    };
}
