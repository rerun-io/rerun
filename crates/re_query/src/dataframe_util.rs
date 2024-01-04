use arrow2::{
    array::{Array, StructArray},
    datatypes::PhysicalType,
};
use polars_core::prelude::*;
use re_data_store::ArrayExt;
use re_types_core::{components::InstanceKey, Archetype, Component, Loggable};

use crate::{ArchetypeView, ComponentWithInstances, QueryError};

/// Make it so that our arrays can be deserialized again by `polars`.
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
                C::arrow_datatype(),
                phys_arrow.clone().into_data().1,
                validity.cloned(),
            );
            Box::new(fixed_arrow)
        }
        _ => array.to_boxed(),
    }
}

/// Iterator for a single column in a dataframe as the rust-native Component type
pub fn iter_column<'a, C: Component + 'a>(
    df: &'a DataFrame,
) -> impl Iterator<Item = Option<C>> + 'a {
    let res = match df.column(C::name().as_ref()) {
        Ok(col) => itertools::Either::Left(col.chunks().iter().flat_map(|array| {
            let fixed_array = fix_polars_nulls::<C>(array.as_ref());
            C::from_arrow_opt(fixed_array.as_ref()).unwrap()
        })),
        Err(_) => itertools::Either::Right(std::iter::repeat_with(|| None).take(df.height())),
    };
    res.into_iter()
}

pub fn df_builder1<'a, C0>(c0: &'a [Option<C0>]) -> crate::Result<DataFrame>
where
    C0: re_types_core::Component + Clone + 'a,
    &'a C0: Into<::std::borrow::Cow<'a, C0>>,
{
    let array0 = C0::to_arrow_opt(c0.iter().map(|c| c.as_ref()))?;

    let series0 = Series::try_from((C0::name().as_ref(), array0.as_ref().clean_for_polars()))?;

    Ok(DataFrame::new(vec![series0])?)
}

pub fn df_builder2<'a, C0, C1>(
    c0: &'a [Option<C0>],
    c1: &'a [Option<C1>],
) -> crate::Result<DataFrame>
where
    C0: re_types_core::Component + Clone + 'a,
    C1: re_types_core::Component + Clone + 'a,
    &'a C0: Into<::std::borrow::Cow<'a, C0>>,
    &'a C1: Into<::std::borrow::Cow<'a, C1>>,
{
    let array0 = C0::to_arrow_opt(c0.iter().map(|c| c.as_ref()))?;
    let array1 = C1::to_arrow_opt(c1.iter().map(|c| c.as_ref()))?;

    let series0 = Series::try_from((C0::name().as_ref(), array0.as_ref().clean_for_polars()))?;
    let series1 = Series::try_from((C1::name().as_ref(), array1.as_ref().clean_for_polars()))?;

    Ok(DataFrame::new(vec![series0, series1])?)
}

pub fn df_builder3<'a, C0, C1, C2>(
    c0: &'a [Option<C0>],
    c1: &'a [Option<C1>],
    c2: &'a [Option<C2>],
) -> crate::Result<DataFrame>
where
    C0: re_types_core::Component + Clone + 'a,
    C1: re_types_core::Component + Clone + 'a,
    C2: re_types_core::Component + Clone + 'a,
    &'a C0: Into<::std::borrow::Cow<'a, C0>>,
    &'a C1: Into<::std::borrow::Cow<'a, C1>>,
    &'a C2: Into<::std::borrow::Cow<'a, C2>>,
{
    let array0 = C0::to_arrow_opt(c0.iter().map(|c| c.as_ref()))?;
    let array1 = C1::to_arrow_opt(c1.iter().map(|c| c.as_ref()))?;
    let array2 = C2::to_arrow_opt(c2.iter().map(|c| c.as_ref()))?;

    let series0 = Series::try_from((C0::name().as_ref(), array0.as_ref().clean_for_polars()))?;
    let series1 = Series::try_from((C1::name().as_ref(), array1.as_ref().clean_for_polars()))?;
    let series2 = Series::try_from((C2::name().as_ref(), array2.as_ref().clean_for_polars()))?;

    Ok(DataFrame::new(vec![series0, series1, series2])?)
}

impl ComponentWithInstances {
    pub fn as_df<'a, C0: Component + 'a>(&'a self) -> crate::Result<DataFrame> {
        if C0::name() != self.name() {
            return Err(QueryError::TypeMismatch {
                actual: self.name(),
                requested: C0::name(),
            });
        }

        let array0 = self.instance_keys.as_arrow_ref();
        let array1 = self.values.as_arrow_ref();

        let series0 = Series::try_from((
            InstanceKey::name().as_ref(),
            array0.as_ref().clean_for_polars(),
        ))?;
        let series1 = Series::try_from((C0::name().as_ref(), array1.as_ref().clean_for_polars()))?;

        Ok(DataFrame::new(vec![series0, series1])?)
    }
}

impl<A: Archetype> ArchetypeView<A> {
    pub fn as_df1<
        'a,
        C1: re_types_core::Component + Clone + Into<::std::borrow::Cow<'a, C1>> + 'a,
    >(
        &self,
    ) -> crate::Result<DataFrame> {
        let array0 = InstanceKey::to_arrow(self.iter_instance_keys())?;
        let array1 = C1::to_arrow_opt(self.iter_optional_component::<C1>()?)?;

        let series0 = Series::try_from((
            InstanceKey::name().as_ref(),
            array0.as_ref().clean_for_polars(),
        ))?;
        let series1 = Series::try_from((C1::name().as_ref(), array1.as_ref().clean_for_polars()))?;

        Ok(DataFrame::new(vec![series0, series1])?)
    }

    pub fn as_df2<
        'a,
        C1: re_types_core::Component + Clone + Into<::std::borrow::Cow<'a, C1>> + 'a,
        C2: re_types_core::Component + Clone + Into<::std::borrow::Cow<'a, C2>> + 'a,
    >(
        &self,
    ) -> crate::Result<DataFrame> {
        let array0 = InstanceKey::to_arrow(self.iter_instance_keys())?;
        let array1 = C1::to_arrow_opt(self.iter_optional_component::<C1>()?)?;
        let array2 = C2::to_arrow_opt(self.iter_optional_component::<C2>()?)?;

        let series0 = Series::try_from((
            InstanceKey::name().as_ref(),
            array0.as_ref().clean_for_polars(),
        ))?;
        let series1 = Series::try_from((C1::name().as_ref(), array1.as_ref().clean_for_polars()))?;
        let series2 = Series::try_from((C2::name().as_ref(), array2.as_ref().clean_for_polars()))?;

        Ok(DataFrame::new(vec![series0, series1, series2])?)
    }
}

#[test]
fn test_df_builder() {
    use re_types::components::{Color, Radius};

    let radii = vec![
        Some(Radius(1.0)),
        Some(Radius(3.0)),
        Some(Radius(5.0)),
        Some(Radius(7.0)),
    ];

    let colors = vec![
        None,
        Some(Color::from(0xff000000)),
        Some(Color::from(0x00ff0000)),
        None,
    ];

    let df = df_builder2(&radii, &colors).unwrap();
    eprintln!("{df:?}");
    //
    // ┌──────────────┬─────────────────┐
    // │ Radius       ┆ Color           │
    // │ ---          ┆ ---             │
    // │ f32          ┆ u32             │
    // ╞══════════════╪═════════════════╡
    // │ 1.0          ┆ null            │
    // │ 3.0          ┆ 4278190080      │
    // │ 5.0          ┆ 16711680        │
    // │ 7.0          ┆ null            │
    // └──────────────┴─────────────────┘

    let expected = df![
        Radius::name().as_ref() => &[1f32, 3f32,  5f32, 7f32],
        Color::name().as_ref() => &[None, Some(0xff000000_u32), Some(0x00ff0000_u32), None],
    ]
    .unwrap();

    assert_eq!(df, expected);
}
