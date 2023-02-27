use arrow2::{
    array::{Array, StructArray},
    datatypes::PhysicalType,
};
use itertools::Itertools;
use polars_core::prelude::*;
use re_arrow_store::ArrayExt;
use re_log_types::{
    component_types::InstanceKey,
    external::arrow2_convert::{
        deserialize::arrow_array_deserialize_iterator, field::ArrowField, serialize::ArrowSerialize,
    },
    msg_bundle::{Component, DeserializableComponent},
};

use crate::{
    entity_view::{ComponentWithInstances, EntityView},
    QueryError,
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
pub fn iter_column<'a, C: DeserializableComponent>(
    df: &'a DataFrame,
) -> impl Iterator<Item = Option<C>> + 'a
where
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

pub fn df_builder1<C0>(c0: &Vec<Option<C0>>) -> crate::Result<DataFrame>
where
    C0: Component + 'static,
    Option<C0>: ArrowSerialize + ArrowField<Type = Option<C0>>,
{
    use arrow2::array::MutableArray;
    use re_log_types::external::arrow2_convert::serialize::arrow_serialize_to_mutable_array;

    let array0 =
        arrow_serialize_to_mutable_array::<Option<C0>, Option<C0>, &Vec<Option<C0>>>(c0)?.as_box();

    let series0 = Series::try_from((C0::name().as_str(), array0.as_ref().clean_for_polars()));

    Ok(DataFrame::new(vec![series0?])?)
}

pub fn df_builder2<C0, C1>(c0: &Vec<Option<C0>>, c1: &Vec<Option<C1>>) -> crate::Result<DataFrame>
where
    C0: Component + 'static,
    Option<C0>: ArrowSerialize + ArrowField<Type = Option<C0>>,
    C1: Component + 'static,
    Option<C1>: ArrowSerialize + ArrowField<Type = Option<C1>>,
{
    use arrow2::array::MutableArray;
    use re_log_types::external::arrow2_convert::serialize::arrow_serialize_to_mutable_array;

    let array0 =
        arrow_serialize_to_mutable_array::<Option<C0>, Option<C0>, &Vec<Option<C0>>>(c0)?.as_box();
    let array1 =
        arrow_serialize_to_mutable_array::<Option<C1>, Option<C1>, &Vec<Option<C1>>>(c1)?.as_box();

    let series0 = Series::try_from((C0::name().as_str(), array0.as_ref().clean_for_polars()))?;
    let series1 = Series::try_from((C1::name().as_str(), array1.as_ref().clean_for_polars()))?;

    Ok(DataFrame::new(vec![series0, series1])?)
}

pub fn df_builder3<C0, C1, C2>(
    c0: &Vec<Option<C0>>,
    c1: &Vec<Option<C1>>,
    c2: &Vec<Option<C2>>,
) -> crate::Result<DataFrame>
where
    C0: Component + 'static,
    Option<C0>: ArrowSerialize + ArrowField<Type = Option<C0>>,
    C1: Component + 'static,
    Option<C1>: ArrowSerialize + ArrowField<Type = Option<C1>>,
    C2: Component + 'static,
    Option<C2>: ArrowSerialize + ArrowField<Type = Option<C2>>,
{
    use arrow2::array::MutableArray;
    use re_log_types::external::arrow2_convert::serialize::arrow_serialize_to_mutable_array;

    let array0 =
        arrow_serialize_to_mutable_array::<Option<C0>, Option<C0>, &Vec<Option<C0>>>(c0)?.as_box();
    let array1 =
        arrow_serialize_to_mutable_array::<Option<C1>, Option<C1>, &Vec<Option<C1>>>(c1)?.as_box();
    let array2 =
        arrow_serialize_to_mutable_array::<Option<C2>, Option<C2>, &Vec<Option<C2>>>(c2)?.as_box();

    let series0 = Series::try_from((C0::name().as_str(), array0.as_ref().clean_for_polars()))?;
    let series1 = Series::try_from((C1::name().as_str(), array1.as_ref().clean_for_polars()))?;
    let series2 = Series::try_from((C2::name().as_str(), array2.as_ref().clean_for_polars()))?;

    Ok(DataFrame::new(vec![series0, series1, series2])?)
}

impl ComponentWithInstances {
    pub fn as_df<C0>(&self) -> crate::Result<DataFrame>
    where
        C0: Component,
        Option<C0>: ArrowSerialize + ArrowField<Type = Option<C0>>,
        C0: DeserializableComponent,
        for<'a> &'a C0::ArrayType: IntoIterator,
    {
        if C0::name() != self.name {
            return Err(QueryError::TypeMismatch {
                actual: self.name,
                requested: C0::name(),
            });
        }

        let instance_keys: Vec<Option<InstanceKey>> =
            self.iter_instance_keys()?.map(Some).collect_vec();

        let values =
            arrow_array_deserialize_iterator::<Option<C0>>(self.values.as_ref())?.collect_vec();

        df_builder2::<InstanceKey, C0>(&instance_keys, &values)
    }
}

impl<Primary: DeserializableComponent + ArrowSerialize> EntityView<Primary>
where
    for<'a> &'a Primary::ArrayType: IntoIterator,
{
    pub fn as_df1(&self) -> crate::Result<DataFrame> {
        let instance_keys = self.primary.iter_instance_keys()?.map(Some).collect_vec();

        let primary_values =
            arrow_array_deserialize_iterator(self.primary.values.as_ref())?.collect_vec();

        df_builder2::<InstanceKey, Primary>(&instance_keys, &primary_values)
    }

    pub fn as_df2<C1>(&self) -> crate::Result<DataFrame>
    where
        C1: Clone + Component,
        Option<C1>: ArrowSerialize + ArrowField<Type = Option<C1>>,
        C1: DeserializableComponent,
        for<'a> &'a C1::ArrayType: IntoIterator,
    {
        let instance_keys = self.primary.iter_instance_keys()?.map(Some).collect_vec();

        let primary_values =
            arrow_array_deserialize_iterator(self.primary.values.as_ref())?.collect_vec();

        let c1_values = self.iter_component::<C1>()?.collect_vec();

        df_builder3::<InstanceKey, Primary, C1>(&instance_keys, &primary_values, &c1_values)
    }
}

#[test]
fn test_df_builder() {
    use re_log_types::component_types::{ColorRGBA, Point2D};

    let points = vec![
        Some(Point2D { x: 1.0, y: 2.0 }),
        Some(Point2D { x: 3.0, y: 4.0 }),
        Some(Point2D { x: 5.0, y: 6.0 }),
        Some(Point2D { x: 7.0, y: 8.0 }),
    ];

    let colors = vec![
        None,
        Some(ColorRGBA(0xff000000)),
        Some(ColorRGBA(0x00ff0000)),
        None,
    ];

    let df = df_builder2(&points, &colors).unwrap();
    // eprintln!("{df:?}");
    //
    // ┌───────────┬────────────┐
    // │ point2d   ┆ colorrgba  │
    // │ ---       ┆ ---        │
    // │ struct[2] ┆ u32        │
    // ╞═══════════╪════════════╡
    // │ {1.0,2.0} ┆ null       │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ {3.0,4.0} ┆ 4278190080 │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ {5.0,6.0} ┆ 16711680   │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ {7.0,8.0} ┆ null       │
    // └───────────┴────────────┘

    // Unnesting the struct makes it easier to validate the results.
    let df = df.unnest([Point2D::name()]).unwrap();
    // eprintln!("{df:?}");
    //
    // ┌─────┬─────┬────────────┐
    // │ x   ┆ y   ┆ colorrgba  │
    // │ --- ┆ --- ┆ ---        │
    // │ f32 ┆ f32 ┆ u32        │
    // ╞═════╪═════╪════════════╡
    // │ 1.0 ┆ 2.0 ┆ null       │
    // ├╌╌╌╌╌┼╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 3.0 ┆ 4.0 ┆ 4278190080 │
    // ├╌╌╌╌╌┼╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 5.0 ┆ 6.0 ┆ 16711680   │
    // ├╌╌╌╌╌┼╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 7.0 ┆ 8.0 ┆ null       │
    // └─────┴─────┴────────────┘

    let expected = df![
        "x" => &[1.0_f32, 3.0_f32, 5.0_f32, 7.0_f32],
        "y" => &[2.0_f32, 4.0_f32, 6.0_f32, 8.0_f32],
        ColorRGBA::name().as_str() => &[None, Some(0xff000000_u32), Some(0x00ff0000_u32), None ],
    ]
    .unwrap();

    assert_eq!(df, expected);
}
