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
    msg_bundle::Component,
};

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

/// Visit a component in a dataframe
/// See [`visit_components2`]
pub fn visit_component<C0: Component>(df: &DataFrame, mut visit: impl FnMut(&C0))
where
    C0: ArrowDeserialize + ArrowField<Type = C0> + 'static,
    C0::ArrayType: ArrowArray,
    for<'a> &'a C0::ArrayType: IntoIterator,
{
    if df.column(C0::name().as_str()).is_ok() {
        let c0_iter = iter_column::<C0>(df);

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
/// ```
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
    df: &DataFrame,
    mut visit: impl FnMut(&C0, Option<&C1>),
) where
    C0: ArrowDeserialize + ArrowField<Type = C0> + 'static,
    C0::ArrayType: ArrowArray,
    for<'a> &'a C0::ArrayType: IntoIterator,
    C1: ArrowDeserialize + ArrowField<Type = C1> + 'static,
    C1::ArrayType: ArrowArray,
    for<'a> &'a C1::ArrayType: IntoIterator,
{
    // The primary column must exist or else we don't have anything to do
    if df.column(C0::name().as_str()).is_ok() {
        let c0_iter = iter_column::<C0>(df);
        let c1_iter = iter_column::<C1>(df);

        itertools::izip!(c0_iter, c1_iter).for_each(|(c0_data, c1_data)| {
            if let Some(c0_data) = c0_data {
                visit(&c0_data, c1_data.as_ref());
            }
        });
    }
}

/// Visit all all of a complex component in a dataframe
/// See [`visit_components2`]
pub fn visit_components3<C0: Component, C1: Component, C2: Component>(
    df: &DataFrame,
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
    // The primary column must exist or else we don't have anything to do
    if df.column(C0::name().as_str()).is_ok() {
        let c0_iter = iter_column::<C0>(df);
        let c1_iter = iter_column::<C1>(df);
        let c2_iter = iter_column::<C2>(df);

        itertools::izip!(c0_iter, c1_iter, c2_iter).for_each(|(c0_data, c1_data, c2_data)| {
            if let Some(c0_data) = c0_data {
                visit(&c0_data, c1_data.as_ref(), c2_data.as_ref());
            }
        });
    }
}
