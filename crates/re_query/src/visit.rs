use arrow2::{
    array::{Array, StructArray},
    datatypes::PhysicalType,
};
use itertools::Itertools;
use polars_core::prelude::*;
use re_log_types::{
    external::arrow2_convert::{
        deserialize::{arrow_array_deserialize_iterator, ArrowArray, ArrowDeserialize},
        field::ArrowField,
    },
    field_types::Instance,
    msg_bundle::Component,
};

use crate::{query::EntityView, ComponentWithInstances};

/// Make it so that our arrays can be deserialized again by arrow2-convert
fn fix_polars_nulls<C: Component>(array: &dyn Array) -> Box<dyn Array> {
    // TODO(jleibs): This is an ugly work-around but gets our serializers
    // working again
    //
    // Explanation: Polars Series appear make all fields nullable. Polars
    // doesn't even have a way to express non-nullable types. However, our
    // `field_types` `Component` definitions have non-nullable fields. This
    // causes a "Data type mismatch" on the deserialization and keeps us from
    // getting at our data.
    //
    // This definitely impacts Struct types, but doesn't seem to cause an issue
    // for primitives
    match array.data_type().to_physical_type() {
        PhysicalType::Struct => {
            let phys_arrow = array.as_any().downcast_ref::<StructArray>().unwrap();
            // Polars doesn't store validity for the top-level of a Struct, only
            // its fields. so use that validity of the first field as the
            // validity for the whole structure.

            // TODO(jleibs): This might have issues with complex structs that
            // include optional fields.
            let validity = phys_arrow.values()[0].validity();
            let fixed_arrow = StructArray::new(
                C::data_type(),
                phys_arrow.clone().into_data().1,
                validity.cloned(),
            );
            Box::new(fixed_arrow)
        }
        _ => array.to_boxed(),
    }
}

/// Iterator for a single column in a dataframe as the rust-native Component type
pub fn iter_column<'a, C: Component>(df: &'a DataFrame) -> impl Iterator<Item = Option<C>> + 'a
where
    C: ArrowDeserialize + ArrowField<Type = C> + 'static,
    C::ArrayType: ArrowArray,
    for<'b> &'b C::ArrayType: IntoIterator,
{
    let res = match df.column(C::name().as_str()) {
        Ok(col) => itertools::Either::Left(col.chunks().iter().flat_map(|array| {
            let fixed_array = fix_polars_nulls::<C>(array.as_ref());
            // TODO(jleibs): the need to collect here isn't ideal
            arrow_array_deserialize_iterator::<Option<C>>(fixed_array.as_ref())
                .unwrap()
                .collect_vec()
        })),
        Err(_) => itertools::Either::Right(std::iter::repeat_with(|| None).take(df.height())),
    };
    res.into_iter()
}

#[derive(Debug)]
struct ComponentJoinedIterator<IIter, VIter> {
    prim_instance_iter: IIter,
    component_instance_iter: IIter,
    component_value_iter: VIter,
    next_component: Option<Instance>,
}

impl<IIter, VIter, C> Iterator for ComponentJoinedIterator<IIter, VIter>
where
    IIter: Iterator<Item = Instance>,
    VIter: Iterator<Item = Option<C>>,
{
    type Item = Option<C>;

    fn next(&mut self) -> Option<Option<C>> {
        match self.prim_instance_iter.next() {
            Some(key) => loop {
                match &self.next_component {
                    Some(next) => {
                        match key.0.cmp(&next.0) {
                            std::cmp::Ordering::Less => break Some(None),
                            std::cmp::Ordering::Equal => {
                                self.next_component = self.component_instance_iter.next();
                                break self.component_value_iter.next();
                            }
                            std::cmp::Ordering::Greater => {
                                // Skip components until we've caught up
                                _ = self.component_value_iter.next();
                                self.next_component = self.component_instance_iter.next();
                            }
                        }
                    }
                    None => break Some(None), // We ran out of component elements
                };
            },
            None => None, // We're done iterating
        }
    }
}

fn auto_instance<IIter>(iter: Option<IIter>, len: usize) -> impl Iterator<Item = Instance>
where
    IIter: Iterator<Item = Instance>,
{
    if let Some(iter) = iter {
        itertools::Either::Left(iter)
    } else {
        let auto_num = (0..len).map(|i| Instance(i as u64));
        itertools::Either::Right(auto_num)
    }
}

pub fn joined_iter<'a, C: Component>(
    primary: &'a ComponentWithInstances,
    component: &'a ComponentWithInstances,
) -> impl Iterator<Item = Option<C>> + 'a
where
    C: ArrowDeserialize + ArrowField<Type = C> + 'static,
    C::ArrayType: ArrowArray,
    for<'b> &'b C::ArrayType: IntoIterator,
{
    let prim_instance_iter = auto_instance(
        primary
            .instance_keys
            .as_ref()
            .map(|keys| arrow_array_deserialize_iterator::<Instance>(keys.as_ref()).unwrap()),
        primary.len(),
    );

    let mut component_instance_iter = auto_instance(
        component
            .instance_keys
            .as_ref()
            .map(|keys| arrow_array_deserialize_iterator::<Instance>(keys.as_ref()).unwrap()),
        primary.len(),
    );

    let component_value_iter =
        arrow_array_deserialize_iterator::<Option<C>>(component.values.as_ref()).unwrap();

    let next_component = component_instance_iter.next();

    ComponentJoinedIterator {
        prim_instance_iter,
        component_instance_iter,
        component_value_iter,
        next_component,
    }
}

/// Visit a component in a dataframe
/// See [`visit_components2`]
pub fn visit_component<C0: Component>(entity_view: &EntityView, mut visit: impl FnMut(&C0))
where
    C0: ArrowDeserialize + ArrowField<Type = C0> + 'static,
    C0::ArrayType: ArrowArray,
    for<'a> &'a C0::ArrayType: IntoIterator,
{
    let df: DataFrame = entity_view.clone().try_into().unwrap();
    if df.column(C0::name().as_str()).is_ok() {
        let c0_iter = iter_column::<C0>(&df);

        c0_iter.for_each(|c0_data| {
            if let Some(c0_data) = c0_data {
                visit(&c0_data);
            }
        });
    }
}

/// Visit all all of a complex component in a dataframe
/// The first component is the primary, while the remaining are optional
///
/// # Usage
/// ``` text
/// # use re_query::dataframe_util::df_builder2;
/// # use re_log_types::field_types::{ColorRGBA, Point2D};
/// use re_query::visit_components2;
///
/// let points = vec![
///     Some(Point2D { x: 1.0, y: 2.0 }),
///     Some(Point2D { x: 3.0, y: 4.0 }),
///     Some(Point2D { x: 5.0, y: 6.0 }),
///     Some(Point2D { x: 7.0, y: 8.0 }),
/// ];
///
/// let colors = vec![
///     None,
///     Some(ColorRGBA(0xff000000)),
///     Some(ColorRGBA(0x00ff0000)),
///     None,
/// ];
///
/// let df = df_builder2(&points, &colors).unwrap();
///
/// let mut points_out = Vec::<Option<Point2D>>::new();
/// let mut colors_out = Vec::<Option<ColorRGBA>>::new();
///
/// visit_components2(&df, |point: &Point2D, color: Option<&ColorRGBA>| {
///     points_out.push(Some(point.clone()));
///     colors_out.push(color.cloned());
/// });
///
/// assert_eq!(points, points_out);
/// assert_eq!(colors, colors_out);
/// ```
pub fn visit_components2<C0: Component, C1: Component>(
    entity_view: &EntityView,
    mut visit: impl FnMut(&C0, Option<&C1>),
) where
    C0: ArrowDeserialize + ArrowField<Type = C0> + 'static,
    C0::ArrayType: ArrowArray,
    for<'a> &'a C0::ArrayType: IntoIterator,
    C1: ArrowDeserialize + ArrowField<Type = C1> + 'static,
    C1::ArrayType: ArrowArray,
    for<'a> &'a C1::ArrayType: IntoIterator,
{
    let c0_iter =
        arrow_array_deserialize_iterator::<Option<C0>>(entity_view.primary.values.as_ref())
            .unwrap();
    //let c0_iter = joined_iter::<C0>(&entity_view.primary, &entity_view.primary);
    let c1_iter = joined_iter::<C1>(&entity_view.primary, &entity_view.components[0]);

    itertools::izip!(c0_iter, c1_iter).for_each(|(c0_data, c1_data)| {
        if let Some(c0_data) = c0_data {
            visit(&c0_data, c1_data.as_ref());
        }
    });
}

/// Visit all all of a complex component in a dataframe
/// See [`visit_components2`]
pub fn visit_components3<C0: Component, C1: Component, C2: Component>(
    entity_view: &EntityView,
    mut visit: impl FnMut(&C0, Option<&C1>, Option<&C2>),
) where
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
    let c0_iter = joined_iter::<C0>(&entity_view.primary, &entity_view.primary);
    let c1_iter = joined_iter::<C1>(&entity_view.primary, &entity_view.components[0]);
    let c2_iter = joined_iter::<C2>(&entity_view.primary, &entity_view.components[1]);

    itertools::izip!(c0_iter, c1_iter, c2_iter).for_each(|(c0_data, c1_data, c2_data)| {
        if let Some(c0_data) = c0_data {
            visit(&c0_data, c1_data.as_ref(), c2_data.as_ref());
        }
    });
}
