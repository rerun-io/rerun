use std::borrow::Cow;

use ahash::HashMap;
use arrow::array::{ArrayRef, NullArray};

use nohash_hasher::IntMap;
use re_chunk::ComponentIdentifier;
use re_types::{
    Component, ComponentDescriptor, ComponentType, SerializationError, ViewClassIdentifier,
};

use crate::{QueryContext, ViewerContext};

/// Tries to get a fallback for the type `C`.
pub fn typed_fallback_for<C: Component>(
    query_context: &QueryContext<'_>,
    component_descr: &ComponentDescriptor,
) -> C {
    debug_assert_eq!(
        Some(C::name()),
        component_descr.component_type,
        "Passed component descriptor doesn't match generic arg `C`"
    );

    let array = query_context
        .viewer_ctx()
        .component_fallback_registry
        .fallback_for(component_descr, query_context);

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

/// Error type for a fallback request.
#[derive(thiserror::Error, Debug)]
pub enum ComponentFallbackError {
    /// Not directly returned by the fallback provider, but useful when serializing a fallback value.
    #[error("Fallback value turned up to be empty when we expected a value.")]
    UnexpectedEmptyFallback,
}

type ComponentFallbackProviderFn =
    Box<dyn Fn(&QueryContext<'_>) -> Result<ArrayRef, SerializationError> + Send + Sync + 'static>;

/// A registry to handle component fallbacks.
///
/// This has 5 layers of fallbacks:
/// - First try to use a fallback for the view context and [`ComponentIdentifier`].
/// - Then try to use a fallback for the [`ComponentIdentifier`].
/// - Then try to use a fallback for the [`ComponentType`].
/// - Then try to use the default value registered into our reflection.
/// - And finally we try to give some sensible value based on the arrow type.
///
/// The first 3 of those fallbacks are registered to this registry.
///
/// For [`ComponentIdentifier`] and [`ComponentType`] specific that is usually done in the
/// `re_component_fallbacks` crate. For view specific fallbacks that is usually done in
/// the views or visualizers `on_register` function.
#[derive(Default)]
pub struct FallbackProviderRegistry {
    view_component_fallback_providers:
        HashMap<(ViewClassIdentifier, ComponentIdentifier), ComponentFallbackProviderFn>,

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
        view: ViewClassIdentifier,
        component: ComponentIdentifier,
        provider: ComponentFallbackProviderFn,
    ) {
        if self
            .view_component_fallback_providers
            .insert((view, component), provider)
            .is_some()
        {
            re_log::warn!(
                "There was already a view component fallback provider registered for {component} in {view}"
            );
        }
    }

    /// Registers a fallback provider function for a given component identifier
    /// in a specific view.
    pub fn register_view_fallback_provider<C: re_types::Component>(
        &mut self,
        view: ViewClassIdentifier,
        descriptor: &ComponentDescriptor,
        provider: impl Fn(&QueryContext<'_>) -> C + Send + Sync + 'static,
    ) {
        debug_assert_eq!(descriptor.component_type, Some(C::name()));

        self.register_dyn_view_fallback_provider(
            view,
            descriptor.component,
            Box::new(move |query_context| {
                let value = provider(query_context);

                C::to_arrow([Cow::Owned(value)])
            }),
        );
    }

    fn get_fallback_function<'a>(
        &'a self,
        descriptor: &ComponentDescriptor,
        ctx: &QueryContext<'_>,
    ) -> Option<&'a ComponentFallbackProviderFn> {
        // First try view specific fallbacks.
        if let Some(f) = self
            .view_component_fallback_providers
            .get(&(ctx.view_ctx.view_class_identifier, descriptor.component))
        {
            return Some(f);
        }

        // Then archetype component field specific.
        if let Some(f) = self.component_fallback_providers.get(&descriptor.component) {
            return Some(f);
        }

        // And finally try component type.
        if let Some(ty) = descriptor.component_type
            && let Some(f) = self.type_fallback_providers.get(&ty)
        {
            return Some(f);
        }

        None
    }

    /// Provides a fallback value for a given component, first trying the provider and
    /// then falling back to the placeholder value registered in the viewer context.
    pub fn fallback_for(
        &self,
        descriptor: &ComponentDescriptor,
        ctx: &QueryContext<'_>,
    ) -> ArrayRef {
        let res = self.get_fallback_function(descriptor, ctx).map(|f| f(ctx));

        match res {
            // Fallback succeeded.
            Some(Ok(array)) => return array,
            // Serialization error
            Some(Err(err)) => {
                // We still want to provide the base fallback value so we can move on,
                // but arrow serialization should never fail.
                // Giving out _both_ the error and the fallback value gets messy,
                // so given that this should be a rare bug, we log it and return the fallback value as success.
                re_log::error_once!(
                    "Arrow serialization failed trying to provide a fallback for {descriptor}. Using base fallback instead: {err}"
                );
            }
            // No specific fallback registered, use placeholder value.
            None => {}
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
