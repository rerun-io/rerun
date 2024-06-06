use ahash::HashMap;
use re_types::{external::arrow2, ComponentName};

use crate::QueryContext;

/// Lookup table for component placeholder values, used whenever no fallback was provided explicitly.
pub type ComponentPlaceholders = HashMap<ComponentName, Box<dyn arrow2::array::Array>>;

/// Result for a fallback request to a provider.
pub enum ComponentFallbackProviderResult {
    /// A fallback value was successfully provided.
    Value(Box<dyn arrow2::array::Array>),

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
    /// The fallback provider is not able to handle the given component _and_ there was no placeholder value.
    ///
    /// This should never happen, since all components should have a placeholder value
    /// registered in [`crate::ViewerContext::component_placeholders`].
    /// Meaning, that this is an unknown component or something went wrong with the placeholder registration.
    #[error("Missing placeholder for component. Was the component's default registered with the viewer?")]
    MissingPlaceholderValue,

    /// Not directly returned by the fallback provider, but useful when serializing a fallback value.
    #[error("Fallback value turned up to be empty when we expected a value.")]
    UnexpectedEmptyFallback,
}

/// Provides fallback values for components, implemented typically by [`crate::SpaceViewClass`] and [`crate::VisualizerSystem`].
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
                re_log::error_once!("Arrow serialization failed trying to provide a fallback for {component}. Using base fallback instead: {err}");
            }
            ComponentFallbackProviderResult::ComponentNotHandled => {}
        }

        match ctx
            .view_ctx
            .viewer_ctx
            .component_placeholders
            .get(&component)
        {
            Some(placeholder) => Ok(placeholder.clone()),
            None => Err(ComponentFallbackError::MissingPlaceholderValue),
        }
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
/// ```no_run
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
                _component_name: re_types::ComponentName,
            ) -> $crate::ComponentFallbackProviderResult {
                $(
                    if _component_name == <$component as re_types::Loggable>::name() {
                        return  $crate::TypedComponentFallbackProvider::<$component>::fallback_for(self, _ctx).into();
                    }
                )*
                $crate::ComponentFallbackProviderResult::ComponentNotHandled
            }
        }
    };
}
