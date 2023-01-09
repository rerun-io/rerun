use re_log_types::{
    external::arrow2_convert::{
        deserialize::{ArrowArray, ArrowDeserialize},
        field::ArrowField,
        serialize::ArrowSerialize,
    },
    field_types::Instance,
    msg_bundle::Component,
};

use crate::EntityView;

impl<Primary> EntityView<Primary>
where
    Primary: Component + ArrowSerialize + ArrowDeserialize + ArrowField<Type = Primary> + 'static,
    Primary::ArrayType: ArrowArray,
    for<'a> &'a Primary::ArrayType: IntoIterator,
{
    /// Visit the primary component of an [`EntityView`]
    /// See [`Self::visit2`]
    pub fn visit(&self, mut visit: impl FnMut(Instance, Primary)) -> crate::Result<()> {
        let instance_iter = self.primary.iter_instance_keys()?;
        let prim_iter = self.primary.iter_values::<Primary>()?;

        itertools::izip!(instance_iter, prim_iter).for_each(|(instance, primary)| {
            if let Some(primary) = primary {
                visit(instance, primary);
            }
        });

        Ok(())
    }

    /// Visit the primary and joined components of an [`EntityView`]
    ///
    /// The function signature for the visitor must always use [`Instance`] for
    /// the first argument, the primary [`Component`] for the second argument,
    /// and then any additional components as `Option`s.
    ///
    /// # Usage
    /// ```
    /// # use re_query::EntityView;
    /// # use re_log_types::field_types::{ColorRGBA, Instance, Point2D};
    ///
    /// let points = vec![
    ///     Point2D { x: 1.0, y: 2.0 },
    ///     Point2D { x: 3.0, y: 4.0 },
    ///     Point2D { x: 5.0, y: 6.0 },
    /// ];
    ///
    /// let colors = vec![
    ///     ColorRGBA(0),
    ///     ColorRGBA(1),
    ///     ColorRGBA(2),
    /// ];
    ///
    /// let entity_view = EntityView::from_native2(
    ///     (None, &points),
    ///     (None, &colors),
    /// )
    /// .unwrap();
    ///
    /// let mut points_out = Vec::<Point2D>::new();
    /// let mut colors_out = Vec::<ColorRGBA>::new();
    ///
    /// entity_view
    ///     .visit2(|_: Instance, point: Point2D, color: Option<ColorRGBA>| {
    ///         points_out.push(point);
    ///         colors_out.push(color.unwrap());
    ///     })
    ///     .ok()
    ///     .unwrap();
    ///
    /// assert_eq!(points, points_out);
    /// assert_eq!(colors, colors_out);
    /// ```
    pub fn visit2<C1: Component>(
        &self,
        mut visit: impl FnMut(Instance, Primary, Option<C1>),
    ) -> crate::Result<()>
    where
        C1: ArrowDeserialize + ArrowField<Type = C1> + 'static,
        C1::ArrayType: ArrowArray,
        for<'a> &'a C1::ArrayType: IntoIterator,
    {
        let instance_iter = self.primary.iter_instance_keys()?;
        let prim_iter = self.primary.iter_values::<Primary>()?;
        let c1_iter = self.iter_component::<C1>()?;

        itertools::izip!(instance_iter, prim_iter, c1_iter).for_each(
            |(instance, primary, c1_data)| {
                if let Some(primary) = primary {
                    visit(instance, primary, c1_data);
                }
            },
        );

        Ok(())
    }

    /// Visit the primary and joined components of an [`EntityView`]
    /// See [`Self::visit2`]
    pub fn visit3<C1: Component, C2: Component>(
        &self,
        mut visit: impl FnMut(Instance, Primary, Option<C1>, Option<C2>),
    ) -> crate::Result<()>
    where
        C1: ArrowDeserialize + ArrowField<Type = C1> + 'static,
        C1::ArrayType: ArrowArray,
        for<'a> &'a C1::ArrayType: IntoIterator,
        C2: ArrowDeserialize + ArrowField<Type = C2> + 'static,
        C2::ArrayType: ArrowArray,
        for<'a> &'a C2::ArrayType: IntoIterator,
    {
        let instance_iter = self.primary.iter_instance_keys()?;
        let prim_iter = self.primary.iter_values::<Primary>()?;
        let c1_iter = self.iter_component::<C1>()?;
        let c2_iter = self.iter_component::<C2>()?;

        itertools::izip!(instance_iter, prim_iter, c1_iter, c2_iter).for_each(
            |(instance, primary, c1_data, c2_data)| {
                if let Some(primary) = primary {
                    visit(instance, primary, c1_data, c2_data);
                }
            },
        );

        Ok(())
    }

    /// Visit the primary and joined components of an [`EntityView`]
    /// See [`Self::visit2`]
    pub fn visit4<C1: Component, C2: Component, C3: Component>(
        &self,
        mut visit: impl FnMut(Instance, Primary, Option<C1>, Option<C2>, Option<C3>),
    ) -> crate::Result<()>
    where
        C1: ArrowDeserialize + ArrowField<Type = C1> + 'static,
        C1::ArrayType: ArrowArray,
        for<'a> &'a C1::ArrayType: IntoIterator,
        C2: ArrowDeserialize + ArrowField<Type = C2> + 'static,
        C2::ArrayType: ArrowArray,
        for<'a> &'a C2::ArrayType: IntoIterator,
        C3: ArrowDeserialize + ArrowField<Type = C3> + 'static,
        C3::ArrayType: ArrowArray,
        for<'a> &'a C3::ArrayType: IntoIterator,
    {
        let instance_iter = self.primary.iter_instance_keys()?;
        let prim_iter = self.primary.iter_values::<Primary>()?;
        let c1_iter = self.iter_component::<C1>()?;
        let c2_iter = self.iter_component::<C2>()?;
        let c3_iter = self.iter_component::<C3>()?;

        itertools::izip!(instance_iter, prim_iter, c1_iter, c2_iter, c3_iter).for_each(
            |(instance, primary, c1_data, c2_data, c3_iter)| {
                if let Some(primary) = primary {
                    visit(instance, primary, c1_data, c2_data, c3_iter);
                }
            },
        );

        Ok(())
    }

    /// Visit the primary and joined components of an [`EntityView`]
    /// See [`Self::visit2`]
    pub fn visit5<C1: Component, C2: Component, C3: Component, C4: Component>(
        &self,
        mut visit: impl FnMut(Instance, Primary, Option<C1>, Option<C2>, Option<C3>, Option<C4>),
    ) -> crate::Result<()>
    where
        C1: ArrowDeserialize + ArrowField<Type = C1> + 'static,
        C1::ArrayType: ArrowArray,
        for<'a> &'a C1::ArrayType: IntoIterator,
        C2: ArrowDeserialize + ArrowField<Type = C2> + 'static,
        C2::ArrayType: ArrowArray,
        for<'a> &'a C2::ArrayType: IntoIterator,
        C3: ArrowDeserialize + ArrowField<Type = C3> + 'static,
        C3::ArrayType: ArrowArray,
        for<'a> &'a C3::ArrayType: IntoIterator,
        C4: ArrowDeserialize + ArrowField<Type = C4> + 'static,
        C4::ArrayType: ArrowArray,
        for<'a> &'a C4::ArrayType: IntoIterator,
    {
        let instance_iter = self.primary.iter_instance_keys()?;
        let prim_iter = self.primary.iter_values::<Primary>()?;
        let c1_iter = self.iter_component::<C1>()?;
        let c2_iter = self.iter_component::<C2>()?;
        let c3_iter = self.iter_component::<C3>()?;
        let c4_iter = self.iter_component::<C4>()?;

        itertools::izip!(instance_iter, prim_iter, c1_iter, c2_iter, c3_iter, c4_iter).for_each(
            |(instance, primary, c1_data, c2_data, c3_iter, c4_iter)| {
                if let Some(primary) = primary {
                    visit(instance, primary, c1_data, c2_data, c3_iter, c4_iter);
                }
            },
        );

        Ok(())
    }

    /// Visit the primary and joined components of an [`EntityView`]
    /// See [`Self::visit2`]
    pub fn visit6<C1: Component, C2: Component, C3: Component, C4: Component, C5: Component>(
        &self,
        mut visit: impl FnMut(
            Instance,
            Primary,
            Option<C1>,
            Option<C2>,
            Option<C3>,
            Option<C4>,
            Option<C5>,
        ),
    ) -> crate::Result<()>
    where
        C1: ArrowDeserialize + ArrowField<Type = C1> + 'static,
        C1::ArrayType: ArrowArray,
        for<'a> &'a C1::ArrayType: IntoIterator,
        C2: ArrowDeserialize + ArrowField<Type = C2> + 'static,
        C2::ArrayType: ArrowArray,
        for<'a> &'a C2::ArrayType: IntoIterator,
        C3: ArrowDeserialize + ArrowField<Type = C3> + 'static,
        C3::ArrayType: ArrowArray,
        for<'a> &'a C3::ArrayType: IntoIterator,
        C4: ArrowDeserialize + ArrowField<Type = C4> + 'static,
        C4::ArrayType: ArrowArray,
        for<'a> &'a C4::ArrayType: IntoIterator,
        C5: ArrowDeserialize + ArrowField<Type = C5> + 'static,
        C5::ArrayType: ArrowArray,
        for<'a> &'a C5::ArrayType: IntoIterator,
    {
        let instance_iter = self.primary.iter_instance_keys()?;
        let prim_iter = self.primary.iter_values::<Primary>()?;
        let c1_iter = self.iter_component::<C1>()?;
        let c2_iter = self.iter_component::<C2>()?;
        let c3_iter = self.iter_component::<C3>()?;
        let c4_iter = self.iter_component::<C4>()?;
        let c5_iter = self.iter_component::<C5>()?;

        itertools::izip!(
            instance_iter,
            prim_iter,
            c1_iter,
            c2_iter,
            c3_iter,
            c4_iter,
            c5_iter
        )
        .for_each(
            |(instance, primary, c1_data, c2_data, c3_iter, c4_iter, c5_iter)| {
                if let Some(primary) = primary {
                    visit(
                        instance, primary, c1_data, c2_data, c3_iter, c4_iter, c5_iter,
                    );
                }
            },
        );

        Ok(())
    }
}
