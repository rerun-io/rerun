use re_types::external::arrow2;

use crate::QueryContext;

/// Result for a fallback request.
pub enum ComponentFallbackResult {
    /// A fallback value was successfully provided.
    Value(Box<dyn arrow2::array::Array>),

    /// Arrow serialization failed.
    SerializationError(re_types::SerializationError),

    /// The fallback provider is not able to handle the given component.
    ComponentNotHandled,
}

impl<T: re_types::ComponentBatch> From<T> for ComponentFallbackResult {
    fn from(batch: T) -> Self {
        match batch.to_arrow() {
            Ok(value) => Self::Value(value),
            Err(err) => Self::SerializationError(err),
        }
    }
}

// TODO: Docs
pub trait ComponentFallbackProvider {
    // TODO: Docs
    fn fallback_value(
        &self,
        _ctx: &QueryContext<'_>,
        _component: re_types::ComponentName,
    ) -> ComponentFallbackResult {
        ComponentFallbackResult::ComponentNotHandled
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
            fn fallback_value(
                &self,
                ctx: &$crate::QueryContext<'_>,
                component_name: re_types::ComponentName,
            ) -> $crate::ComponentFallbackResult {
                $(
                    if component_name == <$component as re_types::Loggable>::name() {
                        return  $crate::TypedComponentFallbackProvider::<$component>::fallback_value(self, ctx).into();
                    }
                )*
                $crate::ComponentFallbackResult::ComponentNotHandled
            }
        }
    };
}
