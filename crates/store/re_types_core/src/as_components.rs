use crate::{
    Component, ComponentBatch, ComponentBatchCowWithDescriptor, LoggableBatch as _, ResultExt as _,
    SerializationResult,
};

/// Describes the interface for interpreting an object as a bundle of [`Component`]s.
///
/// ## Custom bundles
///
/// While, in most cases, component bundles are code generated from our [IDL definitions],
/// it is possible to manually extend existing bundles, or even implement fully custom ones.
///
/// All [`AsComponents`] methods are optional to implement, with the exception of
/// [`AsComponents::as_component_batches`], which describes how the bundle can be interpreted
/// as a set of [`ComponentBatch`]es: arrays of components that are ready to be serialized.
///
/// Have a look at our [Custom Data Loader] example to learn more about handwritten bundles.
///
/// [IDL definitions]: https://github.com/rerun-io/rerun/tree/latest/crates/store/re_types/definitions/rerun
/// [Custom Data Loader]: https://github.com/rerun-io/rerun/blob/latest/examples/rust/custom_data_loader
pub trait AsComponents {
    /// Exposes the object's contents as a set of [`ComponentBatch`]s.
    ///
    /// This is the main mechanism for easily extending builtin archetypes or even writing
    /// fully custom ones.
    /// Have a look at our [Custom Data Loader] example to learn more about extending archetypes.
    ///
    /// Implementers of [`AsComponents`] get one last chance to override the tags in the
    /// [`ComponentDescriptor`], see [`ComponentBatchCowWithDescriptor::descriptor_override`].
    ///
    /// [Custom Data Loader]: https://github.com/rerun-io/rerun/tree/latest/examples/rust/custom_data_loader
    //
    // NOTE: Don't bother returning a CoW here: we need to dynamically discard optional components
    // depending on their presence (or lack thereof) at runtime anyway.
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>>;

    // ---

    /// Serializes all non-null [`Component`]s of this bundle into Arrow arrays.
    ///
    /// The default implementation will simply serialize the result of [`Self::as_component_batches`]
    /// as-is, which is what you want in 99.9% of cases.
    #[inline]
    fn to_arrow(
        &self,
    ) -> SerializationResult<Vec<(::arrow::datatypes::Field, ::arrow::array::ArrayRef)>> {
        self.as_component_batches()
            .into_iter()
            .map(|comp_batch| {
                comp_batch
                    .to_arrow()
                    .map(|array| {
                        let field = arrow::datatypes::Field::new(
                            comp_batch.name().to_string(),
                            array.data_type().clone(),
                            false,
                        );
                        (field, array)
                    })
                    .with_context(comp_batch.name())
            })
            .collect()
    }

    /// Serializes all non-null [`Component`]s of this bundle into Arrow2 arrays.
    ///
    /// The default implementation will simply serialize the result of [`Self::as_component_batches`]
    /// as-is, which is what you want in 99.9% of cases.
    #[inline]
    fn to_arrow2(
        &self,
    ) -> SerializationResult<Vec<(::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>>
    {
        self.as_component_batches()
            .into_iter()
            .map(|comp_batch| {
                comp_batch
                    .to_arrow2()
                    .map(|array| {
                        let field = arrow2::datatypes::Field::new(
                            comp_batch.name().to_string(),
                            array.data_type().clone(),
                            false,
                        );
                        (field, array)
                    })
                    .with_context(comp_batch.name())
            })
            .collect()
    }
}

impl<C: Component> AsComponents for C {
    #[inline]
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        vec![ComponentBatchCowWithDescriptor::new(
            self as &dyn ComponentBatch,
        )]
    }
}

impl AsComponents for dyn ComponentBatch {
    #[inline]
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        vec![ComponentBatchCowWithDescriptor::new(self)]
    }
}

impl<const N: usize> AsComponents for [&dyn ComponentBatch; N] {
    #[inline]
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        self.iter()
            .map(|batch| ComponentBatchCowWithDescriptor::new(*batch))
            .collect()
    }
}

impl<const N: usize> AsComponents for [Box<dyn ComponentBatch>; N] {
    #[inline]
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        self.iter()
            .map(|batch| ComponentBatchCowWithDescriptor::new(&**batch))
            .collect()
    }
}

impl AsComponents for &[&dyn ComponentBatch] {
    #[inline]
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        self.iter()
            .map(|batch| ComponentBatchCowWithDescriptor::new(*batch))
            .collect()
    }
}

impl AsComponents for &[Box<dyn ComponentBatch>] {
    #[inline]
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        self.iter()
            .map(|batch| ComponentBatchCowWithDescriptor::new(&**batch))
            .collect()
    }
}

impl AsComponents for Vec<&dyn ComponentBatch> {
    #[inline]
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        self.iter()
            .map(|batch| ComponentBatchCowWithDescriptor::new(*batch))
            .collect()
    }
}

impl AsComponents for Vec<Box<dyn ComponentBatch>> {
    #[inline]
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        self.iter()
            .map(|batch| ComponentBatchCowWithDescriptor::new(&**batch))
            .collect()
    }
}

impl<AS: AsComponents, const N: usize> AsComponents for [AS; N] {
    #[inline]
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        self.iter()
            .flat_map(|as_components| as_components.as_component_batches())
            .collect()
    }
}

impl<const N: usize> AsComponents for [&dyn AsComponents; N] {
    #[inline]
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        self.iter()
            .flat_map(|as_components| as_components.as_component_batches())
            .collect()
    }
}

impl<const N: usize> AsComponents for [Box<dyn AsComponents>; N] {
    #[inline]
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        self.iter()
            .flat_map(|as_components| as_components.as_component_batches())
            .collect()
    }
}

impl<AS: AsComponents> AsComponents for &[AS] {
    #[inline]
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        self.iter()
            .flat_map(|as_components| as_components.as_component_batches())
            .collect()
    }
}

impl AsComponents for &[&dyn AsComponents] {
    #[inline]
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        self.iter()
            .flat_map(|as_components| as_components.as_component_batches())
            .collect()
    }
}

impl AsComponents for &[Box<dyn AsComponents>] {
    #[inline]
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        self.iter()
            .flat_map(|as_components| as_components.as_component_batches())
            .collect()
    }
}

impl<AS: AsComponents> AsComponents for Vec<AS> {
    #[inline]
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        self.iter()
            .flat_map(|as_components| as_components.as_component_batches())
            .collect()
    }
}

impl AsComponents for Vec<&dyn AsComponents> {
    #[inline]
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        self.iter()
            .flat_map(|as_components| as_components.as_component_batches())
            .collect()
    }
}

impl AsComponents for Vec<Box<dyn AsComponents>> {
    #[inline]
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        self.iter()
            .flat_map(|as_components| as_components.as_component_batches())
            .collect()
    }
}
