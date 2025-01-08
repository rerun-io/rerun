use crate::{
    ComponentBatch, ComponentBatchCowWithDescriptor, SerializationResult, SerializedComponentBatch,
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
/// [`Component`]: [crate::Component]
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
    /// [`ComponentDescriptor`]: [crate::ComponentDescriptor]
    //
    // NOTE: Don't bother returning a CoW here: we need to dynamically discard optional components
    // depending on their presence (or lack thereof) at runtime anyway.
    #[deprecated(since = "0.22.0", note = "use as_component_batches_v2 instead")]
    #[allow(clippy::unimplemented)] // temporary, this method is about to be replaced
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        // Eagerly serialized archetypes simply cannot implement this.
        //
        // This method only exist while we are in the process of making all existing archetypes
        // eagerly serialized, at which point it'll be removed.
        unimplemented!()
    }

    /// Exposes the object's contents as a set of [`SerializedComponentBatch`]es.
    ///
    /// This is the main mechanism for easily extending builtin archetypes or even writing
    /// fully custom ones.
    /// Have a look at our [Custom Data Loader] example to learn more about extending archetypes.
    ///
    /// Implementers of [`AsComponents`] get one last chance to override the tags in the
    /// [`ComponentDescriptor`], see [`SerializedComponentBatch::with_descriptor_override`].
    ///
    /// [Custom Data Loader]: https://github.com/rerun-io/rerun/blob/latest/docs/snippets/all/tutorials/custom_data.rs
    /// [`ComponentDescriptor`]: [crate::ComponentDescriptor]
    //
    // NOTE: Don't bother returning a CoW here: we need to dynamically discard optional components
    // depending on their presence (or lack thereof) at runtime anyway.
    fn as_component_batches_v2(&self) -> Vec<SerializedComponentBatch> {
        #[allow(deprecated)] // that's the whole point
        self.as_component_batches()
            .into_iter()
            .filter_map(|batch| batch.serialized())
            .collect()
    }

    // ---

    /// Serializes all non-null [`Component`]s of this bundle into Arrow arrays.
    ///
    /// The default implementation will simply serialize the result of [`Self::as_component_batches`]
    /// as-is, which is what you want in 99.9% of cases.
    ///
    /// [`Component`]: [crate::Component]
    #[inline]
    fn to_arrow(
        &self,
    ) -> SerializationResult<Vec<(::arrow::datatypes::Field, ::arrow::array::ArrayRef)>> {
        self.as_component_batches_v2()
            .into_iter()
            .map(|comp_batch| {
                let field = arrow::datatypes::Field::new(
                    comp_batch.descriptor.component_name.to_string(),
                    comp_batch.array.data_type().clone(),
                    false,
                );
                Ok((field, comp_batch.array))
            })
            .collect()
    }

    /// Serializes all non-null [`Component`]s of this bundle into Arrow2 arrays.
    ///
    /// The default implementation will simply serialize the result of [`Self::as_component_batches`]
    /// as-is, which is what you want in 99.9% of cases.
    ///
    /// [`Component`]: [crate::Component]
    #[inline]
    fn to_arrow2(
        &self,
    ) -> SerializationResult<Vec<(::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>>
    {
        self.as_component_batches_v2()
            .into_iter()
            .map(|comp_batch| {
                let field = arrow2::datatypes::Field::new(
                    comp_batch.descriptor.component_name.to_string(),
                    comp_batch.array.data_type().clone().into(),
                    false,
                );
                Ok((field, comp_batch.array.into()))
            })
            .collect()
    }
}

#[allow(dead_code)]
fn assert_object_safe() {
    let _: &dyn AsComponents;
}

impl AsComponents for dyn ComponentBatch {
    #[inline]
    fn as_component_batches_v2(&self) -> Vec<SerializedComponentBatch> {
        self.serialized().into_iter().collect()
    }
}

impl<const N: usize> AsComponents for [&dyn ComponentBatch; N] {
    #[inline]
    fn as_component_batches_v2(&self) -> Vec<SerializedComponentBatch> {
        self.iter().filter_map(|batch| batch.serialized()).collect()
    }
}

impl<const N: usize> AsComponents for [Box<dyn ComponentBatch>; N] {
    #[inline]
    fn as_component_batches_v2(&self) -> Vec<SerializedComponentBatch> {
        self.iter().filter_map(|batch| batch.serialized()).collect()
    }
}

impl AsComponents for Vec<&dyn ComponentBatch> {
    #[inline]
    fn as_component_batches_v2(&self) -> Vec<SerializedComponentBatch> {
        self.iter().filter_map(|batch| batch.serialized()).collect()
    }
}

impl AsComponents for Vec<Box<dyn ComponentBatch>> {
    #[inline]
    fn as_component_batches_v2(&self) -> Vec<SerializedComponentBatch> {
        self.iter().filter_map(|batch| batch.serialized()).collect()
    }
}

impl AsComponents for SerializedComponentBatch {
    #[inline]
    fn as_component_batches_v2(&self) -> Vec<SerializedComponentBatch> {
        vec![self.clone()]
    }
}

impl<AS: AsComponents, const N: usize> AsComponents for [AS; N] {
    #[inline]
    fn as_component_batches_v2(&self) -> Vec<SerializedComponentBatch> {
        self.iter()
            .flat_map(|as_components| as_components.as_component_batches_v2())
            .collect()
    }
}

impl<const N: usize> AsComponents for [&dyn AsComponents; N] {
    #[inline]
    fn as_component_batches_v2(&self) -> Vec<SerializedComponentBatch> {
        self.iter()
            .flat_map(|as_components| as_components.as_component_batches_v2())
            .collect()
    }
}

impl<const N: usize> AsComponents for [Box<dyn AsComponents>; N] {
    #[inline]
    fn as_component_batches_v2(&self) -> Vec<SerializedComponentBatch> {
        self.iter()
            .flat_map(|as_components| as_components.as_component_batches_v2())
            .collect()
    }
}

impl<AS: AsComponents> AsComponents for Vec<AS> {
    #[inline]
    fn as_component_batches_v2(&self) -> Vec<SerializedComponentBatch> {
        self.iter()
            .flat_map(|as_components| as_components.as_component_batches_v2())
            .collect()
    }
}

impl AsComponents for Vec<&dyn AsComponents> {
    #[inline]
    fn as_component_batches_v2(&self) -> Vec<SerializedComponentBatch> {
        self.iter()
            .flat_map(|as_components| as_components.as_component_batches_v2())
            .collect()
    }
}

impl AsComponents for Vec<Box<dyn AsComponents>> {
    #[inline]
    fn as_component_batches_v2(&self) -> Vec<SerializedComponentBatch> {
        self.iter()
            .flat_map(|as_components| as_components.as_component_batches_v2())
            .collect()
    }
}

// ---

// NOTE: These needs to not be tests in order for doc-tests to work.

/// ```compile_fail
/// let comp = re_types_core::components::ClearIsRecursive::default();
/// let _ = (&comp as &dyn re_types_core::AsComponents).as_component_batches();
/// ```
#[allow(dead_code)]
#[allow(rustdoc::private_doc_tests)] // doc-tests are the only way to assert failed compilation
fn single_ascomponents() {}

/// ```compile_fail
/// let comp = re_types_core::components::ClearIsRecursive::default();
/// let _ = (&[comp] as &dyn re_types_core::AsComponents).as_component_batches();
/// ```
#[allow(dead_code)]
#[allow(rustdoc::private_doc_tests)] // doc-tests are the only way to assert failed compilation
fn single_ascomponents_wrapped() {
    // This is non-sense (and more importantly: dangerous): a single component shouldn't be able to
    // autocast straight to a collection of batches.
}

/// ```compile_fail
/// let comp = re_types_core::components::ClearIsRecursive::default();
/// let _ = (&[comp, comp, comp] as &dyn re_types_core::AsComponents).as_component_batches();
/// ```
#[allow(dead_code)]
#[allow(rustdoc::private_doc_tests)] // doc-tests are the only way to assert failed compilation
fn single_ascomponents_wrapped_many() {
    // This is non-sense (and more importantly: dangerous): a single component shouldn't be able to
    // autocast straight to a collection of batches.
}

/// ```compile_fail
/// let comp = re_types_core::components::ClearIsRecursive::default();
/// let comps = vec![comp, comp, comp];
/// let _ = (&comps as &dyn re_types_core::AsComponents).as_component_batches();
/// ```
#[allow(dead_code)]
#[allow(rustdoc::private_doc_tests)] // doc-tests are the only way to assert failed compilation
fn many_ascomponents() {}

/// ```compile_fail
/// let comp = re_types_core::components::ClearIsRecursive::default();
/// let comps = vec![comp, comp, comp];
/// let _ = (&[comps] as &dyn re_types_core::AsComponents).as_component_batches();
/// ```
#[allow(dead_code)]
#[allow(rustdoc::private_doc_tests)] // doc-tests are the only way to assert failed compilation
fn many_ascomponents_wrapped() {}

/// ```compile_fail
/// let comp = re_types_core::components::ClearIsRecursive::default();
/// let comps = vec![comp, comp, comp];
/// let _ = (&[comps] as &dyn re_types_core::ComponentBatch).to_arrow();
/// ```
#[allow(dead_code)]
#[allow(rustdoc::private_doc_tests)] // doc-tests are the only way to assert failed compilation
fn many_componentbatch_wrapped() {}

/// ```compile_fail
/// let comp = re_types_core::components::ClearIsRecursive::default();
/// let comps = vec![comp, comp, comp];
/// let _ = (&[comps.clone(), comps.clone(), comps.clone()] as &dyn re_types_core::AsComponents).as_component_batches();
/// ```
#[allow(dead_code)]
#[allow(rustdoc::private_doc_tests)] // doc-tests are the only way to assert failed compilation
fn many_ascomponents_wrapped_many() {}

/// ```compile_fail
/// let comp = re_types_core::components::ClearIsRecursive::default();
/// let comps = vec![comp, comp, comp];
/// let _ = (&[comps.clone(), comps.clone(), comps.clone()] as &dyn re_types_core::ComponentBatch).to_arrow();
/// ```
#[allow(dead_code)]
#[allow(rustdoc::private_doc_tests)] // doc-tests are the only way to assert failed compilation
fn many_componentbatch_wrapped_many() {}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::{
        types::UInt32Type, Array as ArrowArray, PrimitiveArray as ArrowPrimitiveArray,
    };
    use itertools::Itertools;
    use similar_asserts::assert_eq;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
    #[repr(transparent)]
    pub struct MyColor(pub u32);

    crate::macros::impl_into_cow!(MyColor);

    impl re_byte_size::SizeBytes for MyColor {
        #[inline]
        fn heap_size_bytes(&self) -> u64 {
            let Self(_) = self;
            0
        }
    }

    impl crate::Loggable for MyColor {
        fn arrow_datatype() -> arrow::datatypes::DataType {
            arrow::datatypes::DataType::UInt32
        }

        fn to_arrow_opt<'a>(
            data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
        ) -> crate::SerializationResult<arrow::array::ArrayRef>
        where
            Self: 'a,
        {
            use crate::datatypes::UInt32;
            UInt32::to_arrow_opt(
                data.into_iter()
                    .map(|opt| opt.map(Into::into).map(|c| UInt32(c.0))),
            )
        }

        fn from_arrow_opt(
            data: &dyn arrow::array::Array,
        ) -> crate::DeserializationResult<Vec<Option<Self>>> {
            use crate::datatypes::UInt32;
            Ok(UInt32::from_arrow_opt(data)?
                .into_iter()
                .map(|opt| opt.map(|v| Self(v.0)))
                .collect())
        }
    }

    impl crate::Component for MyColor {
        fn descriptor() -> crate::ComponentDescriptor {
            crate::ComponentDescriptor::new("example.MyColor")
        }
    }

    #[allow(dead_code)]
    fn data() -> (MyColor, MyColor, MyColor, Vec<MyColor>) {
        let red = MyColor(0xDD0000FF);
        let green = MyColor(0x00DD00FF);
        let blue = MyColor(0x0000DDFF);
        let colors = vec![red, green, blue];
        (red, green, blue, colors)
    }

    #[test]
    fn single_ascomponents_howto() {
        let (red, _, _, _) = data();

        let got = {
            let red = &red as &dyn crate::ComponentBatch;
            (&[red] as &dyn crate::AsComponents)
                .as_component_batches_v2()
                .into_iter()
                .map(|batch| batch.array)
                .collect_vec()
        };
        let expected = vec![
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![red.0])) as Arc<dyn ArrowArray>,
        ];
        assert_eq!(&expected, &got);
    }

    #[test]
    fn single_componentbatch() -> anyhow::Result<()> {
        let (red, _, _, _) = data();

        // A single component should autocast to a batch with a single instance.
        let got = (&red as &dyn crate::ComponentBatch).to_arrow()?;
        let expected =
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![red.0])) as Arc<dyn ArrowArray>;
        similar_asserts::assert_eq!(&expected, &got);

        Ok(())
    }

    #[test]
    fn single_ascomponents_wrapped_howto() {
        let (red, _, _, _) = data();

        let got = {
            let red = &red as &dyn crate::ComponentBatch;
            (&[red] as &dyn crate::AsComponents)
                .as_component_batches_v2()
                .into_iter()
                .map(|batch| batch.array)
                .collect_vec()
        };
        let expected = vec![
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![red.0])) as Arc<dyn ArrowArray>,
        ];
        assert_eq!(&expected, &got);
    }

    #[test]
    fn single_componentbatch_wrapped() -> anyhow::Result<()> {
        let (red, _, _, _) = data();

        // Nothing out of the ordinary here, a slice of components is indeed a batch.
        let got = (&[red] as &dyn crate::ComponentBatch).to_arrow()?;
        let expected =
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![red.0])) as Arc<dyn ArrowArray>;
        similar_asserts::assert_eq!(&expected, &got);

        Ok(())
    }

    #[test]
    fn single_ascomponents_wrapped_many_howto() {
        let (red, green, blue, _) = data();

        let got = {
            let red = &red as &dyn crate::ComponentBatch;
            let green = &green as &dyn crate::ComponentBatch;
            let blue = &blue as &dyn crate::ComponentBatch;
            (&[red, green, blue] as &dyn crate::AsComponents)
                .as_component_batches_v2()
                .into_iter()
                .map(|batch| batch.array)
                .collect_vec()
        };
        let expected = vec![
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![red.0])) as Arc<dyn ArrowArray>,
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![green.0])) as Arc<dyn ArrowArray>,
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![blue.0])) as Arc<dyn ArrowArray>,
        ];
        assert_eq!(&expected, &got);
    }

    #[test]
    fn single_componentbatch_wrapped_many() -> anyhow::Result<()> {
        let (red, green, blue, _) = data();

        // Nothing out of the ordinary here, a slice of components is indeed a batch.
        let got = (&[red, green, blue] as &dyn crate::ComponentBatch).to_arrow()?;
        let expected = Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![
            red.0, green.0, blue.0,
        ])) as Arc<dyn ArrowArray>;
        similar_asserts::assert_eq!(&expected, &got);

        Ok(())
    }

    #[test]
    fn many_componentbatch() -> anyhow::Result<()> {
        let (red, green, blue, colors) = data();

        // Nothing out of the ordinary here, a batch is indeed a batch.
        let got = (&colors as &dyn crate::ComponentBatch).to_arrow()?;
        let expected = Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![
            red.0, green.0, blue.0,
        ])) as Arc<dyn ArrowArray>;
        similar_asserts::assert_eq!(&expected, &got);

        Ok(())
    }

    #[test]
    fn many_ascomponents_wrapped_howto() {
        let (red, green, blue, colors) = data();

        let got = {
            let colors = &colors as &dyn crate::ComponentBatch;
            (&[colors] as &dyn crate::AsComponents)
                .as_component_batches_v2()
                .into_iter()
                .map(|batch| batch.array)
                .collect_vec()
        };
        let expected = vec![Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![
            red.0, green.0, blue.0,
        ])) as Arc<dyn ArrowArray>];
        assert_eq!(&expected, &got);
    }

    #[test]
    fn many_ascomponents_wrapped_many_howto() {
        let (red, green, blue, colors) = data();

        // Nothing out of the ordinary here, a collection of batches is indeed a collection of batches.
        let got = {
            let colors = &colors as &dyn crate::ComponentBatch;
            (&[colors, colors, colors] as &dyn crate::AsComponents)
                .as_component_batches_v2()
                .into_iter()
                .map(|batch| batch.array)
                .collect_vec()
        };
        let expected = vec![
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![
                red.0, green.0, blue.0,
            ])) as Arc<dyn ArrowArray>,
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![
                red.0, green.0, blue.0,
            ])) as Arc<dyn ArrowArray>,
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![
                red.0, green.0, blue.0,
            ])) as Arc<dyn ArrowArray>,
        ];
        assert_eq!(&expected, &got);
    }
}
