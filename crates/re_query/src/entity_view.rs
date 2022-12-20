use std::collections::BTreeMap;

use arrow2::array::{Array, MutableArray};
use re_log_types::{
    external::arrow2_convert::{
        deserialize::{arrow_array_deserialize_iterator, ArrowArray, ArrowDeserialize},
        field::ArrowField,
        serialize::ArrowSerialize,
    },
    field_types::Instance,
    msg_bundle::Component,
    ComponentName,
};

/// A type-erased array of [`Component`] values and the corresponding [`Instance`] keys.
///
/// `instance_keys` must always be sorted if present. If not present we assume implicit
/// instance keys that are equal to the row-number.
///
/// See: [`crate::get_component_with_instances`]
#[derive(Clone, Debug)]
pub struct ComponentWithInstances {
    pub(crate) name: ComponentName,
    // TODO(jleibs): Remove optional once the store guarantees this will always exist
    pub(crate) instance_keys: Option<Box<dyn Array>>,
    pub(crate) values: Box<dyn Array>,
}

impl ComponentWithInstances {
    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.len() == 0
    }

    /// Iterate over the instance keys
    ///
    /// If the instance keys don't exist, generate them based on array-index position of the values
    pub fn iter_instance_keys(&self) -> crate::Result<impl Iterator<Item = Instance> + '_> {
        if let Some(keys) = &self.instance_keys {
            let iter = arrow_array_deserialize_iterator::<Instance>(keys.as_ref())?;
            Ok(itertools::Either::Left(iter))
        } else {
            let auto_num = (0..self.len()).map(|i| Instance(i as u64));
            Ok(itertools::Either::Right(auto_num))
        }
    }

    /// Iterate over the values and convert them to a native `Component`
    pub fn iter_values<C: Component>(&self) -> crate::Result<impl Iterator<Item = Option<C>> + '_>
    where
        C: ArrowDeserialize + ArrowField<Type = C> + 'static,
        C::ArrayType: ArrowArray,
        for<'a> &'a C::ArrayType: IntoIterator,
    {
        Ok(arrow_array_deserialize_iterator::<Option<C>>(
            self.values.as_ref(),
        )?)
    }

    /// Produce a `ComponentWithInstances` from native component types
    pub fn from_native<C>(
        instance_keys: Option<&Vec<Instance>>,
        values: &Vec<C>,
    ) -> crate::Result<ComponentWithInstances>
    where
        C: Component + 'static,
        C: ArrowSerialize + ArrowField<Type = C>,
    {
        use re_log_types::external::arrow2_convert::serialize::arrow_serialize_to_mutable_array;

        let instance_keys = if let Some(keys) = instance_keys {
            Some(
                arrow_serialize_to_mutable_array::<Instance, Instance, &Vec<Instance>>(keys)?
                    .as_box(),
            )
        } else {
            None
        };

        let values = arrow_serialize_to_mutable_array::<C, C, &Vec<C>>(values)?.as_box();

        Ok(ComponentWithInstances {
            name: C::name(),
            instance_keys,
            values,
        })
    }
}

/// Iterator over a single component joined onto a primary component
///
/// This is equivalent to a left join between one table made up of the
/// instance-keys from the primary component and another table with the
/// instance-keys and values of the iterated component.
///
/// For example
/// ```text
/// primary
/// +----------+
/// | instance |
/// +----------+
/// | key0     |
/// | key1     |
/// | Key2     |
///
/// component
/// +----------+-------+
/// | instance | value |
/// +----------+-------+
/// | key0     | val0  |
/// | Key2     | val2  |
///
/// SELECT value FROM LEFT JOIN primary.instance = component.instance;
///
/// output
/// +-------+
/// | value |
/// +-------+
/// | val0  |
/// | NULL  |
/// | val2  |
///
/// ```
struct ComponentJoinedIterator<IIter1, IIter2, VIter> {
    primary_instance_iter: IIter1,
    component_instance_iter: IIter2,
    component_value_iter: VIter,
    next_component_instance: Option<Instance>,
}

impl<IIter1, IIter2, VIter, C> Iterator for ComponentJoinedIterator<IIter1, IIter2, VIter>
where
    IIter1: Iterator<Item = Instance>,
    IIter2: Iterator<Item = Instance>,
    VIter: Iterator<Item = Option<C>>,
{
    type Item = Option<C>;

    fn next(&mut self) -> Option<Option<C>> {
        // For each value of primary_instance_iter we must find a result
        if let Some(primary_key) = self.primary_instance_iter.next() {
            loop {
                match &self.next_component_instance {
                    // If we have a next component key, we either...
                    Some(instance_key) => {
                        match primary_key.0.cmp(&instance_key.0) {
                            // Return a None if the primary_key hasn't reached it yet
                            std::cmp::Ordering::Less => break Some(None),
                            // Return the value if the keys match
                            std::cmp::Ordering::Equal => {
                                self.next_component_instance = self.component_instance_iter.next();
                                break self.component_value_iter.next();
                            }
                            // Skip this component if the key is behind the primary key
                            std::cmp::Ordering::Greater => {
                                _ = self.component_value_iter.next();
                                self.next_component_instance = self.component_instance_iter.next();
                            }
                        }
                    }
                    // Otherwise, we ran out of component elements. Just return
                    // None until the primary iter ends.
                    None => break Some(None),
                };
            }
        } else {
            None
        }
    }
}

/// A view of an entity at a particular point in time returned by [`crate::get_component_with_instances`]
///
/// `EntityView` has a special `primary` component which determines the length of an entity
/// batch. When iterating over individual components, they will be implicitly joined onto
/// the primary component using instance keys.
#[derive(Clone, Debug)]
pub struct EntityView {
    pub(crate) primary: ComponentWithInstances,
    pub(crate) components: BTreeMap<ComponentName, ComponentWithInstances>,
}
impl EntityView {
    /// Iterate over the values of a `Component`.
    ///
    /// Always produces an iterator of length `self.primary.len()`
    pub fn iter_component<C: Component>(
        &self,
    ) -> crate::Result<impl Iterator<Item = Option<C>> + '_>
    where
        C: ArrowDeserialize + ArrowField<Type = C> + 'static,
        C::ArrayType: ArrowArray,
        for<'b> &'b C::ArrayType: IntoIterator,
    {
        let component = self.components.get(&C::name());

        if let Some(component) = component {
            let prim_instance_iter = self.primary.iter_instance_keys()?;

            let mut component_instance_iter = component.iter_instance_keys()?;

            let component_value_iter =
                arrow_array_deserialize_iterator::<Option<C>>(component.values.as_ref())?;

            let next_component = component_instance_iter.next();

            Ok(itertools::Either::Left(ComponentJoinedIterator {
                primary_instance_iter: prim_instance_iter,
                component_instance_iter,
                component_value_iter,
                next_component_instance: next_component,
            }))
        } else {
            let nulls = (0..self.primary.values.len()).map(|_| None);
            Ok(itertools::Either::Right(nulls))
        }
    }

    /// Helper function to produce an `EntityView` from rust-native `field_types`
    pub fn from_native<C0>(c0: (Option<&Vec<Instance>>, &Vec<C0>)) -> crate::Result<EntityView>
    where
        C0: Component + 'static,
        C0: ArrowSerialize + ArrowField<Type = C0>,
    {
        let primary = ComponentWithInstances::from_native(c0.0, c0.1)?;

        Ok(EntityView {
            primary,
            components: Default::default(),
        })
    }

    /// Helper function to produce an `EntityView` from rust-native `field_types`
    pub fn from_native2<C0, C1>(
        c0: (Option<&Vec<Instance>>, &Vec<C0>),
        c1: (Option<&Vec<Instance>>, &Vec<C1>),
    ) -> crate::Result<EntityView>
    where
        C0: Component + 'static,
        C0: ArrowSerialize + ArrowField<Type = C0>,
        C1: Component + 'static,
        C1: ArrowSerialize + ArrowField<Type = C1>,
    {
        let primary = ComponentWithInstances::from_native(c0.0, c0.1)?;
        let component_c1 = ComponentWithInstances::from_native(c1.0, c1.1)?;

        let components = [(component_c1.name, component_c1)].into();

        Ok(EntityView {
            primary,
            components,
        })
    }
}
