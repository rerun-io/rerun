// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/components/rotation3d.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Component**: A 3D rotation, represented either by a quaternion or a rotation around axis.
#[derive(Clone, Debug, PartialEq)]
pub struct Rotation3D(
    /// Representation of the rotation.
    pub crate::datatypes::Rotation3D,
);

impl<T: Into<crate::datatypes::Rotation3D>> From<T> for Rotation3D {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::datatypes::Rotation3D> for Rotation3D {
    #[inline]
    fn borrow(&self) -> &crate::datatypes::Rotation3D {
        &self.0
    }
}

impl std::ops::Deref for Rotation3D {
    type Target = crate::datatypes::Rotation3D;

    #[inline]
    fn deref(&self) -> &crate::datatypes::Rotation3D {
        &self.0
    }
}

::re_types_core::macros::impl_into_cow!(Rotation3D);

impl ::re_types_core::Loggable for Rotation3D {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.components.Rotation3D".into()
    }

    #[allow(clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::*;
        DataType::Union(
            vec![
                Field {
                    name: "_null_markers".to_owned(),
                    data_type: DataType::Null,
                    is_nullable: true,
                    metadata: [].into(),
                },
                Field {
                    name: "Quaternion".to_owned(),
                    data_type: <crate::datatypes::Quaternion>::arrow_datatype(),
                    is_nullable: false,
                    metadata: [].into(),
                },
                Field {
                    name: "AxisAngle".to_owned(),
                    data_type: <crate::datatypes::RotationAxisAngle>::arrow_datatype(),
                    is_nullable: false,
                    metadata: [].into(),
                },
            ],
            Some(vec![0i32, 1i32, 2i32]),
            UnionMode::Dense,
        )
    }

    #[allow(clippy::wildcard_imports)]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, datatypes::*};
        Ok({
            let (somes, data0): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    let datum = datum.map(|datum| {
                        let Self(data0) = datum.into_owned();
                        data0
                    });
                    (datum.is_some(), datum)
                })
                .unzip();
            let data0_bitmap: Option<arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            {
                _ = data0_bitmap;
                crate::datatypes::Rotation3D::to_arrow_opt(data0)?
            }
        })
    }

    #[allow(clippy::wildcard_imports)]
    fn from_arrow_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, buffer::*, datatypes::*};
        Ok(crate::datatypes::Rotation3D::from_arrow_opt(arrow_data)
            .with_context("rerun.components.Rotation3D#repr")?
            .into_iter()
            .map(|v| v.ok_or_else(DeserializationError::missing_data))
            .map(|res| res.map(|v| Some(Self(v))))
            .collect::<DeserializationResult<Vec<Option<_>>>>()
            .with_context("rerun.components.Rotation3D#repr")
            .with_context("rerun.components.Rotation3D")?)
    }
}
