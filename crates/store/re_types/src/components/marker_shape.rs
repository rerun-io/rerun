// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/components/marker_shape.fbs".

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::map_flatten)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Component**: The visual appearance of a point in e.g. a 2D plot.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum MarkerShape {
    /// `⏺`
    #[default]
    Circle = 1,

    /// `◆`
    Diamond = 2,

    /// `◼️`
    Square = 3,

    /// `x`
    Cross = 4,

    /// `+`
    Plus = 5,

    /// `▲`
    Up = 6,

    /// `▼`
    Down = 7,

    /// `◀`
    Left = 8,

    /// `▶`
    Right = 9,

    /// `*`
    Asterisk = 10,
}

impl ::re_types_core::reflection::Enum for MarkerShape {
    #[inline]
    fn variants() -> &'static [Self] {
        &[
            Self::Circle,
            Self::Diamond,
            Self::Square,
            Self::Cross,
            Self::Plus,
            Self::Up,
            Self::Down,
            Self::Left,
            Self::Right,
            Self::Asterisk,
        ]
    }

    #[inline]
    fn docstring_md(self) -> &'static str {
        match self {
            Self::Circle => "`⏺`",
            Self::Diamond => "`◆`",
            Self::Square => "`◼\u{fe0f}`",
            Self::Cross => "`x`",
            Self::Plus => "`+`",
            Self::Up => "`▲`",
            Self::Down => "`▼`",
            Self::Left => "`◀`",
            Self::Right => "`▶`",
            Self::Asterisk => "`*`",
        }
    }
}

impl ::re_types_core::SizeBytes for MarkerShape {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

impl std::fmt::Display for MarkerShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Circle => write!(f, "Circle"),
            Self::Diamond => write!(f, "Diamond"),
            Self::Square => write!(f, "Square"),
            Self::Cross => write!(f, "Cross"),
            Self::Plus => write!(f, "Plus"),
            Self::Up => write!(f, "Up"),
            Self::Down => write!(f, "Down"),
            Self::Left => write!(f, "Left"),
            Self::Right => write!(f, "Right"),
            Self::Asterisk => write!(f, "Asterisk"),
        }
    }
}

::re_types_core::macros::impl_into_cow!(MarkerShape);

impl ::re_types_core::Loggable for MarkerShape {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.components.MarkerShape".into()
    }

    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        #![allow(clippy::wildcard_imports)]
        use arrow2::datatypes::*;
        DataType::UInt8
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        #![allow(clippy::wildcard_imports)]
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, datatypes::*};
        Ok({
            let (somes, data0): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    let datum = datum.map(|datum| *datum as u8);
                    (datum.is_some(), datum)
                })
                .unzip();
            let data0_bitmap: Option<arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            PrimitiveArray::new(
                Self::arrow_datatype(),
                data0.into_iter().map(|v| v.unwrap_or_default()).collect(),
                data0_bitmap,
            )
            .boxed()
        })
    }

    fn from_arrow_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        #![allow(clippy::wildcard_imports)]
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, buffer::*, datatypes::*};
        Ok(arrow_data
            .as_any()
            .downcast_ref::<UInt8Array>()
            .ok_or_else(|| {
                let expected = Self::arrow_datatype();
                let actual = arrow_data.data_type().clone();
                DeserializationError::datatype_mismatch(expected, actual)
            })
            .with_context("rerun.components.MarkerShape#enum")?
            .into_iter()
            .map(|opt| opt.copied())
            .map(|typ| match typ {
                Some(1) => Ok(Some(Self::Circle)),
                Some(2) => Ok(Some(Self::Diamond)),
                Some(3) => Ok(Some(Self::Square)),
                Some(4) => Ok(Some(Self::Cross)),
                Some(5) => Ok(Some(Self::Plus)),
                Some(6) => Ok(Some(Self::Up)),
                Some(7) => Ok(Some(Self::Down)),
                Some(8) => Ok(Some(Self::Left)),
                Some(9) => Ok(Some(Self::Right)),
                Some(10) => Ok(Some(Self::Asterisk)),
                None => Ok(None),
                Some(invalid) => Err(DeserializationError::missing_union_arm(
                    Self::arrow_datatype(),
                    "<invalid>",
                    invalid as _,
                )),
            })
            .collect::<DeserializationResult<Vec<Option<_>>>>()
            .with_context("rerun.components.MarkerShape")?)
    }
}
