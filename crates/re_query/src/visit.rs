//! Visit the primary and joined components of an [`EntityView`]
//!
//! The function signature for the visitor must always use [`Instance`] for
//! the first argument, the primary [`Component`] for the second argument,
//! and then any additional components as `Option`s.
//!
//! # Usage
//! ```
//! # use re_query::EntityView;
//! # use re_log_types::field_types::{ColorRGBA, Instance, Point2D};
//!
//! let points = vec![
//!     Point2D { x: 1.0, y: 2.0 },
//!     Point2D { x: 3.0, y: 4.0 },
//!     Point2D { x: 5.0, y: 6.0 },
//! ];
//!
//! let colors = vec![
//!     ColorRGBA(0),
//!     ColorRGBA(1),
//!     ColorRGBA(2),
//! ];
//!
//! let entity_view = EntityView::from_native2(
//!     (None, &points),
//!     (None, &colors),
//! )
//! .unwrap();
//!
//! let mut points_out = Vec::<Point2D>::new();
//! let mut colors_out = Vec::<ColorRGBA>::new();
//!
//! entity_view
//!     .visit2(|_: Instance, point: Point2D, color: Option<ColorRGBA>| {
//!         points_out.push(point);
//!         colors_out.push(color.unwrap());
//!     })
//!     .ok()
//!     .unwrap();
//!
//! assert_eq!(points, points_out);
//! assert_eq!(colors, colors_out);
//! ```

use re_log_types::{
    component_types::Instance,
    external::arrow2_convert::{
        deserialize::{ArrowArray, ArrowDeserialize},
        field::ArrowField,
        serialize::ArrowSerialize,
    },
    msg_bundle::Component,
};

use crate::EntityView;

macro_rules! create_visitor {

    // $name: The name of the visit function to create
    // $CC: List of names of the component types, e.g., C1, C2
    // $cc: List of the names of the component variables, e.g., _c1, _c2
    ($name:ident; $($CC:ident),* ; $($cc:ident),*) => (

        #[doc = "Visit the primary component of an [`EntityView`]. See [`crate::visit`]"]
        pub fn $name < $( $CC: Component, )* >(
            &self,
            mut visit: impl FnMut(
                Instance,
                Primary,
                $(Option<$CC>),*
            )

        ) -> crate::Result<()>
        where $(
            $CC: Clone + ArrowDeserialize + ArrowField<Type = $CC> + 'static,
            $CC::ArrayType: ArrowArray,
            for<'a> &'a $CC::ArrayType: IntoIterator,
        )*
        {
            crate::profile_function!();

            ::itertools::izip!(
                self.primary.iter_instance_keys()?,
                self.primary.iter_values::<Primary>()?,
                $(
                    self.iter_component::<$CC>()?,
                )*
            ).for_each(
                |(instance, primary, $($cc,)*)| {
                    if let Some(primary) = primary {
                        visit(instance, primary, $($cc,)*);
                    }
                }
            );

            Ok(())
        }
    );
}

impl<Primary> EntityView<Primary>
where
    Primary: Component + ArrowSerialize + ArrowDeserialize + ArrowField<Type = Primary> + 'static,
    Primary::ArrayType: ArrowArray,
    for<'a> &'a Primary::ArrayType: IntoIterator,
{
    create_visitor! {visit1; ;}
    create_visitor! {visit2; C1; _c1}
    create_visitor! {visit3; C1, C2; _c1, _c2}
    create_visitor! {visit4; C1, C2, C3; _c1, _c2, _c3}
    create_visitor! {visit5; C1, C2, C3, C4; _c1, _c2, _c3, _c4}
    create_visitor! {visit6; C1, C2, C3, C4, C5; _c1, _c2, _c3, _c4, _c5}
    create_visitor! {visit7; C1, C2, C3, C4, C5, C6; _c1, _c2, _c3, _c4, _c5, _c6}
}
