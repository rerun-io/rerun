//! Visit the primary and joined components of an [`EntityView`]
//!
//! The function signature for the visitor must always use [`InstanceKey`] for
//! the first argument, the primary [`Component`] for the second argument,
//! and then any additional components as `Option`s.
//!
//! # Usage
//! ```
//! # use re_query::EntityView;
//! # use re_types::components::{Color, Point2D, InstanceKey};
//!
//! let instances = InstanceKey::from_iter(0..3);
//!
//! let points = [
//!     Point2D::new(1.0, 2.0),
//!     Point2D::new(3.0, 4.0),
//!     Point2D::new(5.0, 6.0),
//! ];
//!
//! let colors = [
//!     Color::from(0),
//!     Color::from(1),
//!     Color::from(2),
//! ];
//!
//! let entity_view = EntityView::from_native2(
//!     (&instances, &points),
//!     (&instances, &colors),
//! );
//!
//! let mut points_out = Vec::<Point2D>::new();
//! let mut colors_out = Vec::<Color>::new();
//!
//! entity_view
//!     .visit2(|_: InstanceKey, point: Point2D, color: Option<Color>| {
//!         points_out.push(point);
//!         colors_out.push(color.unwrap());
//!     })
//!     .ok()
//!     .unwrap();
//!
//! assert_eq!(points.as_slice(), points_out.as_slice());
//! assert_eq!(colors.as_slice(), colors_out.as_slice());
//! ```

use re_types::components::InstanceKey;
use re_types::Component;

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
                InstanceKey,
                Primary,
                $(Option<$CC>),*
            )

        ) -> crate::Result<()>
        where $(
            $CC: re_types::Component,
        )*
        $(
            &'a $CC: std::convert::Into<std::borrow::Cow<'a, $CC>> + 'a,
        )*
        {
            re_tracing::profile_function!();

            ::itertools::izip!(
                self.primary.instance_keys(),
                self.primary.values::<Primary>()?,
                $(
                    self.iter_component::<$CC>()?,
                )*
            ).for_each(
                |(instance_key, primary, $($cc,)*)| {
                    if let Some(primary) = primary {
                        visit(instance_key.into(), primary, $($cc,)*);
                    }
                }
            );

            Ok(())
        }
    );
}

impl<'a, Primary> EntityView<Primary>
where
    Primary: re_types::Component,
    &'a Primary: std::convert::Into<std::borrow::Cow<'a, Primary>> + 'a,
{
    create_visitor! {visit1; ;}
    create_visitor! {visit2; C1; _c1}
    create_visitor! {visit3; C1, C2; _c1, _c2}
    create_visitor! {visit4; C1, C2, C3; _c1, _c2, _c3}
    create_visitor! {visit5; C1, C2, C3, C4; _c1, _c2, _c3, _c4}
    create_visitor! {visit6; C1, C2, C3, C4, C5; _c1, _c2, _c3, _c4, _c5}
    create_visitor! {visit7; C1, C2, C3, C4, C5, C6; _c1, _c2, _c3, _c4, _c5, _c6}
}
