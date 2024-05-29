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

/// Context for a latest at query in a specific view.
///
/// TODO: move elsewhere
/// TODO: this is centered around latest-at queries. does it have to be
pub struct QueryContext<'a> {
    pub viewer_ctx: &'a ViewerContext<'a>,

    /// Target entity path which is lacking the component and needs a fallback.
    ///
    /// For editing overrides/defaults, this is the path to the store entity where they override/default is used.
    /// For view properties this is the path that stores the respective view property archetype.
    pub target_entity_path: &'a re_log_types::EntityPath,

    /// Archetype name in which context the component is needed.
    ///
    /// View properties always have an archetype context, but overrides/defaults may not.
    pub archetype_name: Option<re_types::ArchetypeName>,

    /// Query which didn't yield a result for the component at the target entity path.
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
        _ctx: &QueryContext<'_>,
        _component: re_types::ComponentName,
    ) -> ComponentFallbackResult {
        ComponentFallbackResult::UnknownComponent
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
                $crate::ComponentFallbackResult::UnknownComponent
            }
        }
    };
}
