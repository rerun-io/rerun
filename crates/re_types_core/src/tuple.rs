//! Implements [`AsComponents`] for tuples.

use crate::{AsComponents, MaybeOwnedComponentBatch};

macro_rules! impl_as_components_for_tuples {
    // This recursive macro trick is from <https://stackoverflow.com/questions/55553281/>.
    ( $_dropped:literal, $( $name:literal, )* ) => {
        paste::paste! {
            /// A tuple of bundles may act as a bundle.
            impl<$( [<B $name>] : AsComponents ),*>
                AsComponents for ($( [<B $name>], )*)
            {
                fn as_component_batches(
                    &self,
                ) -> Vec<MaybeOwnedComponentBatch<'_>> {
                    #[allow(unused_mut)]
                    let mut vector = Vec::new();
                    $(
                        vector.extend(self.$name.as_component_batches());
                    )*
                    vector
                }

                fn num_instances(&self) -> usize {
                    0 $( .max(self.$name.num_instances()) )*
                }

                fn to_arrow(
                    &self
                ) -> crate::SerializationResult<
                    Vec<(::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>,
                > {
                    #[allow(unused_mut)]
                    let mut vector = Vec::new();
                    $(
                        vector.extend(self.$name.to_arrow()?);
                    )*
                    Ok(vector)
                }
            }
        }
        impl_as_components_for_tuples!($( $name, )*);
    };
    () => {};
}

impl_as_components_for_tuples!(9999, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,);
