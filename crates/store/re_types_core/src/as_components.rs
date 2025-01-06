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

// TODO: This is our biggest problem.
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

// ---

// TODO: Right now, none of these fail to compile, which is _NOT_ good.

// NOTE: These needs to not be tests in order for doc-tests to work.

/// ```compile_fail
/// let comp = re_types_core::components::ClearIsRecursive::default();
/// let _ = (&comp as &dyn re_types_core::AsComponents).as_component_batches();
/// ```
#[allow(dead_code)]
fn single_ascomponents() {}

/// ```compile_fail
/// let comp = re_types_core::components::ClearIsRecursive::default();
/// let _ = (&[comp] as &dyn re_types_core::AsComponents).as_component_batches();
/// ```
#[allow(dead_code)]
fn single_ascomponents_wrapped() {
    // This is non-sense (and more importantly: dangerous): a single component shouldn't be able to
    // autocast straight to a collection of batches.
}

/// ```compile_fail
/// let comp = re_types_core::components::ClearIsRecursive::default();
/// let _ = (&[comp, comp, comp] as &dyn re_types_core::AsComponents).as_component_batches();
/// ```
#[allow(dead_code)]
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
fn many_ascomponents() {}

/// ```compile_fail
/// let comp = re_types_core::components::ClearIsRecursive::default();
/// let comps = vec![comp, comp, comp];
/// let _ = (&[comps] as &dyn re_types_core::AsComponents).as_component_batches();
/// ```
#[allow(dead_code)]
fn many_ascomponents_wrapped() {}

/// ```compile_fail
/// let comp = re_types_core::components::ClearIsRecursive::default();
/// let comps = vec![comp, comp, comp];
/// let _ = (&[comps] as &dyn re_types_core::ComponentBatch).to_arrow();
/// ```
#[allow(dead_code)]
fn many_componentbatch_wrapped() {}

/// ```compile_fail
/// let comp = re_types_core::components::ClearIsRecursive::default();
/// let comps = vec![comp, comp, comp];
/// let _ = (&[comps.clone(), comps.clone(), comps.clone()] as &dyn re_types_core::AsComponents).as_component_batches();
/// ```
#[allow(dead_code)]
fn many_ascomponents_wrapped_many() {}

/// ```compile_fail
/// let comp = re_types_core::components::ClearIsRecursive::default();
/// let comps = vec![comp, comp, comp];
/// let _ = (&[comps.clone(), comps.clone(), comps.clone()] as &dyn re_types_core::ComponentBatch).to_arrow();
/// ```
#[allow(dead_code)]
fn many_componentbatch_wrapped_many() {}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::{
        types::UInt32Type, Array as ArrowArray, PrimitiveArray as ArrowPrimitiveArray,
    };
    use similar_asserts::assert_eq;

    use crate::LoggableBatch;

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
        fn arrow2_datatype() -> arrow2::datatypes::DataType {
            arrow2::datatypes::DataType::UInt32
        }

        fn to_arrow2_opt<'a>(
            data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
        ) -> crate::SerializationResult<Box<dyn arrow2::array::Array>>
        where
            Self: 'a,
        {
            use crate::datatypes::UInt32;
            UInt32::to_arrow2_opt(
                data.into_iter()
                    .map(|opt| opt.map(Into::into).map(|c| UInt32(c.0))),
            )
        }

        fn from_arrow2_opt(
            data: &dyn arrow2::array::Array,
        ) -> crate::DeserializationResult<Vec<Option<Self>>> {
            use crate::datatypes::UInt32;
            Ok(UInt32::from_arrow2_opt(data)?
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
    fn single_ascomponents() -> anyhow::Result<()> {
        let (red, _, _, _) = data();

        // TODO: This test should not compile, but this is what it does at the moment.
        let got = {
            let got: Result<Vec<_>, _> = (&red as &dyn crate::AsComponents)
                .as_component_batches()
                .into_iter()
                .map(|batch| batch.to_arrow())
                .collect();
            got?
        };
        let expected = vec![
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![red.0])) as Arc<dyn ArrowArray>,
        ];
        assert_eq!(&expected, &got);

        Ok(())
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
    fn single_ascomponents_wrapped() -> anyhow::Result<()> {
        let (red, _, _, _) = data();

        // TODO: This test should not compile, but this is what it does at the moment.
        let got = {
            let got: Result<Vec<_>, _> = (&[red] as &dyn crate::AsComponents)
                .as_component_batches()
                .into_iter()
                .map(|batch| batch.to_arrow())
                .collect();
            got?
        };
        let expected = vec![
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![red.0])) as Arc<dyn ArrowArray>,
        ];
        assert_eq!(&expected, &got);

        Ok(())
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
    fn single_ascomponents_wrapped_many() -> anyhow::Result<()> {
        let (red, green, blue, _) = data();

        // TODO: This test should not compile, but this is what it does at the moment (which is
        // complete non-sense).
        // TODO: The issue is that we do want a ComponentBatch to autocast to a AsComponents...
        let got = {
            let got: Result<Vec<_>, _> = (&[red, green, blue] as &dyn crate::AsComponents)
                .as_component_batches()
                .into_iter()
                .map(|batch| batch.to_arrow())
                .collect();
            got?
        };
        let expected = vec![
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![red.0])) as Arc<dyn ArrowArray>,
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![green.0])) as Arc<dyn ArrowArray>,
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![blue.0])) as Arc<dyn ArrowArray>,
        ];
        assert_eq!(&expected, &got);

        Ok(())
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
    fn many_ascomponents() -> anyhow::Result<()> {
        let (red, green, blue, colors) = data();

        // TODO: This test should not compile, but this is what it does at the moment (which is
        // complete non-sense).
        // TODO: The issue is that we do want a ComponentBatch to autocast to a AsComponents...
        let got = {
            let got: Result<Vec<_>, _> = (&colors as &dyn crate::AsComponents)
                .as_component_batches()
                .into_iter()
                .map(|batch| batch.to_arrow())
                .collect();
            got?
        };
        let expected = vec![
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![red.0])) as Arc<dyn ArrowArray>,
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![green.0])) as Arc<dyn ArrowArray>,
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![blue.0])) as Arc<dyn ArrowArray>,
        ];
        assert_eq!(&expected, &got);

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
    fn many_ascomponents_wrapped() -> anyhow::Result<()> {
        let (red, green, blue, colors) = data();

        // TODO: This test should not compile, but this is what it does at the moment (which is
        // complete non-sense).
        // TODO: The issue is that we do want a ComponentBatch to autocast to a AsComponents...
        let got = {
            let got: Result<Vec<_>, _> = (&[colors] as &dyn crate::AsComponents)
                .as_component_batches()
                .into_iter()
                .map(|batch| batch.to_arrow())
                .collect();
            got?
        };
        // TODO: This doesn't make any sense at all.
        // let expected = vec![Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![
        //     red.0, green.0, blue.0,
        // ])) as Arc<dyn ArrowArray>];
        let expected = vec![
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![red.0])) as Arc<dyn ArrowArray>,
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![green.0])) as Arc<dyn ArrowArray>,
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![blue.0])) as Arc<dyn ArrowArray>,
        ];
        assert_eq!(&expected, &got);

        Ok(())
    }

    // TODO: that one will never compile no matter what
    // #[test]
    // fn many_componentbatch_wrapped() -> anyhow::Result<()> {
    //     let (red, green, blue, colors) = data();
    //
    //     // Nothing out of the ordinary here, a batch is indeed a batch.
    //     let got = (&[colors] as &dyn crate::ComponentBatch).to_arrow()?;
    //     let expected = Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![
    //         red.0, green.0, blue.0,
    //     ])) as Arc<dyn ArrowArray>;
    //     similar_asserts::assert_eq!(&expected, &got);
    //
    //     Ok(())
    // }

    #[test]
    fn many_ascomponents_wrapped_many() -> anyhow::Result<()> {
        let (red, green, blue, colors) = data();

        // Nothing out of the ordinary here, a collection of batches is indeed a colletion of batches.
        let got = {
            let got: Result<Vec<_>, _> = (&[colors.clone(), colors.clone(), colors.clone()]
                as &dyn crate::AsComponents)
                .as_component_batches()
                .into_iter()
                .map(|batch| batch.to_arrow())
                .collect();
            got?
        };
        // TODO: This doesn't make any sense at all.
        let expected = vec![
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![red.0])) as Arc<dyn ArrowArray>,
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![green.0])),
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![blue.0])),
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![red.0])),
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![green.0])),
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![blue.0])),
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![red.0])),
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![green.0])),
            Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![blue.0])),
        ];
        assert_eq!(&expected, &got);

        Ok(())
    }

    // TODO: that one will never compile no matter what
    // #[test]
    // fn many_componentbatch_wrapped_many() -> anyhow::Result<()> {
    //     let (red, green, blue, colors) = data();
    //
    //     // Nothing out of the ordinary here, a batch is indeed a batch.
    //     let got = (&[colors.clone(), colors.clone(), colors.clone()] as &dyn crate::ComponentBatch)
    //         .to_arrow()?;
    //     let expected = Arc::new(ArrowPrimitiveArray::<UInt32Type>::from(vec![
    //         red.0, green.0, blue.0,
    //     ])) as Arc<dyn ArrowArray>;
    //     similar_asserts::assert_eq!(&expected, &got);
    //
    //     Ok(())
    // }
}
