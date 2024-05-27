use re_types::external::arrow2;

use crate::ViewerContext;

// TODO: Docs
pub enum ComponentFallbackResult {
    Value(Box<dyn arrow2::array::Array>),
    SerializationError(re_types::SerializationError),
    UnknownComponent,
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
pub struct FallbackProviderContext<'a> {
    pub ctx: &'a ViewerContext<'a>,
    pub entity_path: &'a re_log_types::EntityPath,
    pub archetype_name: Option<re_types::ArchetypeName>,
    // TODO(andreas): Can we make this a `ViewQuery` instead?
    // pub query: &'a ViewQuery<'a>,
    pub query: &'a re_data_store::LatestAtQuery,
    // pub view_state: &'a dyn SpaceViewState, // TODO(andreas): Need this, but don't know yet how to patch through everywhere.
}

// TODO: Docs
pub trait ComponentFallbackProvider {
    // TODO: Docs
    fn fallback_value(
        &self,
        _ctx: &FallbackProviderContext<'_>,
        _component: re_types::ComponentName,
    ) -> ComponentFallbackResult {
        ComponentFallbackResult::UnknownComponent
    }
}

// TODO: Docs
pub trait TypedComponentFallbackProvider<C: re_types::Component> {
    fn fallback_value(&self, ctx: &FallbackProviderContext<'_>) -> C;
}

/// Implements the [`ComponentFallbackProvider`] trait for a given type, using a number of [`TypedComponentFallbackProvider`].
#[macro_export]
macro_rules! impl_component_fallback_provider {
    ($type:ty => [$($component:ty),*]) => {
        impl re_viewer_context::ComponentFallbackProvider for $type {
            fn fallback_value(
                &self,
                ctx: &re_viewer_context::FallbackProviderContext<'_>,
                component_name: re_types::ComponentName,
            ) -> re_viewer_context::ComponentFallbackResult {
                $(
                    if component_name == <$component as re_types::Loggable>::name() {
                        return re_viewer_context::TypedComponentFallbackProvider::<$component>::fallback_value(self, ctx).into();
                    }
                )*
                re_viewer_context::ComponentFallbackResult::UnknownComponent
            }
        }
    };
}
