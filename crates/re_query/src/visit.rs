use re_log_types::{
    external::arrow2_convert::{
        deserialize::{arrow_array_deserialize_iterator, ArrowArray, ArrowDeserialize},
        field::ArrowField,
    },
    field_types::Instance,
    msg_bundle::Component,
};

use crate::EntityView;

impl EntityView {
    /// Visit the primary component of an `EntityView`
    /// See [`visit_entity2`]
    pub fn visit<C0: Component>(&self, mut visit: impl FnMut(Instance, C0)) -> crate::Result<()>
    where
        C0: ArrowDeserialize + ArrowField<Type = C0> + 'static,
        C0::ArrayType: ArrowArray,
        for<'a> &'a C0::ArrayType: IntoIterator,
    {
        let instance_iter = self.primary.iter_instance_keys()?;

        let prim_iter =
            arrow_array_deserialize_iterator::<Option<C0>>(self.primary.values.as_ref())?;

        itertools::izip!(instance_iter, prim_iter).for_each(|(instance, primary)| {
            if let Some(primary) = primary {
                visit(instance, primary);
            }
        });

        Ok(())
    }

    /// Visit the primary and joined components of an `EntityView`
    ///
    /// The first component is the primary, while the remaining are optional
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
    pub fn visit2<C0: Component, C1: Component>(
        &self,
        mut visit: impl FnMut(Instance, C0, Option<C1>),
    ) -> crate::Result<()>
    where
        C0: ArrowDeserialize + ArrowField<Type = C0> + 'static,
        C0::ArrayType: ArrowArray,
        for<'a> &'a C0::ArrayType: IntoIterator,
        C1: ArrowDeserialize + ArrowField<Type = C1> + 'static,
        C1::ArrayType: ArrowArray,
        for<'a> &'a C1::ArrayType: IntoIterator,
    {
        let instance_iter = self.primary.iter_instance_keys()?;

        let prim_iter =
            arrow_array_deserialize_iterator::<Option<C0>>(self.primary.values.as_ref())?;

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

    /// Visit all all of a complex component in a dataframe
    /// See [`visit_components2`]
    pub fn visit3<C0: Component, C1: Component, C2: Component>(
        &self,
        mut visit: impl FnMut(Instance, C0, Option<C1>, Option<C2>),
    ) -> crate::Result<()>
    where
        C0: ArrowDeserialize + ArrowField<Type = C0> + 'static,
        C0::ArrayType: ArrowArray,
        for<'a> &'a C0::ArrayType: IntoIterator,
        C1: ArrowDeserialize + ArrowField<Type = C1> + 'static,
        C1::ArrayType: ArrowArray,
        for<'a> &'a C1::ArrayType: IntoIterator,
        C2: ArrowDeserialize + ArrowField<Type = C2> + 'static,
        C2::ArrayType: ArrowArray,
        for<'a> &'a C2::ArrayType: IntoIterator,
    {
        let instance_iter = self.primary.iter_instance_keys()?;

        let prim_iter =
            arrow_array_deserialize_iterator::<Option<C0>>(self.primary.values.as_ref())?;

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
}
