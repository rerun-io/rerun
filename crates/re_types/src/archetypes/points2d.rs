// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

#![allow(trivial_numeric_casts)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::map_flatten)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_cast)]

#[doc = "A 2D point cloud with positions and optional colors, radii, labels, etc."]
#[derive(Debug, Clone, PartialEq)]
pub struct Points2D {
    #[doc = "All the actual 2D points that make up the point cloud."]
    pub points: Vec<crate::components::Point2D>,

    #[doc = "Optional radii for the points, effectively turning them into circles."]
    pub radii: Option<Vec<crate::components::Radius>>,

    #[doc = "Optional colors for the points."]
    pub colors: Option<Vec<crate::components::Color>>,

    #[doc = "Optional text labels for the points."]
    pub labels: Option<Vec<crate::components::Label>>,

    #[doc = "An optional floating point value that specifies the 2D drawing order."]
    #[doc = "Objects with higher values are drawn on top of those with lower values."]
    #[doc = ""]
    #[doc = "The default for 2D points is 30.0."]
    pub draw_order: Option<crate::components::DrawOrder>,

    #[doc = "Optional class Ids for the points."]
    #[doc = ""]
    #[doc = "The class ID provides colors and labels if not specified explicitly."]
    pub class_ids: Option<Vec<crate::components::ClassId>>,

    #[doc = "Optional keypoint IDs for the points, identifying them within a class."]
    #[doc = ""]
    #[doc = "If keypoint IDs are passed in but no class IDs were specified, the class ID will"]
    #[doc = "default to 0."]
    #[doc = "This is useful to identify points within a single classification (which is identified"]
    #[doc = "with `class_id`)."]
    #[doc = "E.g. the classification might be 'Person' and the keypoints refer to joints on a"]
    #[doc = "detected skeleton."]
    pub keypoint_ids: Option<Vec<crate::components::KeypointId>>,

    #[doc = "Unique identifiers for each individual point in the batch."]
    pub instance_keys: Option<Vec<crate::components::InstanceKey>>,
}

impl Points2D {
    pub const REQUIRED_COMPONENTS: [crate::ComponentName; 1usize] =
        [crate::ComponentName::Borrowed("rerun.components.Point2D")];

    pub const RECOMMENDED_COMPONENTS: [crate::ComponentName; 2usize] = [
        crate::ComponentName::Borrowed("rerun.components.Radius"),
        crate::ComponentName::Borrowed("rerun.components.Color"),
    ];

    pub const OPTIONAL_COMPONENTS: [crate::ComponentName; 5usize] = [
        crate::ComponentName::Borrowed("rerun.components.Label"),
        crate::ComponentName::Borrowed("rerun.components.DrawOrder"),
        crate::ComponentName::Borrowed("rerun.components.ClassId"),
        crate::ComponentName::Borrowed("rerun.components.KeypointId"),
        crate::ComponentName::Borrowed("rerun.components.InstanceKey"),
    ];

    pub const ALL_COMPONENTS: [crate::ComponentName; 8usize] = [
        crate::ComponentName::Borrowed("rerun.components.Point2D"),
        crate::ComponentName::Borrowed("rerun.components.Radius"),
        crate::ComponentName::Borrowed("rerun.components.Color"),
        crate::ComponentName::Borrowed("rerun.components.Label"),
        crate::ComponentName::Borrowed("rerun.components.DrawOrder"),
        crate::ComponentName::Borrowed("rerun.components.ClassId"),
        crate::ComponentName::Borrowed("rerun.components.KeypointId"),
        crate::ComponentName::Borrowed("rerun.components.InstanceKey"),
    ];
}

impl crate::Archetype for Points2D {
    #[inline]
    fn name() -> crate::ArchetypeName {
        crate::ArchetypeName::Borrowed("rerun.archetypes.Points2D")
    }

    #[inline]
    fn required_components() -> Vec<crate::ComponentName> {
        Self::REQUIRED_COMPONENTS.to_vec()
    }

    #[inline]
    fn recommended_components() -> Vec<crate::ComponentName> {
        Self::RECOMMENDED_COMPONENTS.to_vec()
    }

    #[inline]
    fn optional_components() -> Vec<crate::ComponentName> {
        Self::OPTIONAL_COMPONENTS.to_vec()
    }

    #[inline]
    fn try_to_arrow(
        &self,
    ) -> crate::SerializationResult<
        Vec<(::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>,
    > {
        use crate::Component as _;
        Ok([
            {
                Some({
                    let array =
                        <crate::components::Point2D>::try_to_arrow(self.points.iter(), None);
                    array.map(|array| {
                        let datatype = array.data_type().clone();
                        (
                            ::arrow2::datatypes::Field::new("points", datatype, false),
                            array,
                        )
                    })
                })
                .transpose()?
            },
            {
                self.radii
                    .as_ref()
                    .map(|many| {
                        let array = <crate::components::Radius>::try_to_arrow(many.iter(), None);
                        array.map(|array| {
                            let datatype = array.data_type().clone();
                            (
                                ::arrow2::datatypes::Field::new("radii", datatype, false),
                                array,
                            )
                        })
                    })
                    .transpose()?
            },
            {
                self.colors
                    .as_ref()
                    .map(|many| {
                        let array = <crate::components::Color>::try_to_arrow(many.iter(), None);
                        array.map(|array| {
                            let datatype = array.data_type().clone();
                            (
                                ::arrow2::datatypes::Field::new("colors", datatype, false),
                                array,
                            )
                        })
                    })
                    .transpose()?
            },
            {
                self.labels
                    .as_ref()
                    .map(|many| {
                        let array = <crate::components::Label>::try_to_arrow(many.iter(), None);
                        array.map(|array| {
                            let datatype = array.data_type().clone();
                            (
                                ::arrow2::datatypes::Field::new("labels", datatype, false),
                                array,
                            )
                        })
                    })
                    .transpose()?
            },
            {
                self.draw_order
                    .as_ref()
                    .map(|single| {
                        let array = <crate::components::DrawOrder>::try_to_arrow([single], None);
                        array.map(|array| {
                            let datatype = array.data_type().clone();
                            (
                                ::arrow2::datatypes::Field::new("draw_order", datatype, false),
                                array,
                            )
                        })
                    })
                    .transpose()?
            },
            {
                self.class_ids
                    .as_ref()
                    .map(|many| {
                        let array = <crate::components::ClassId>::try_to_arrow(many.iter(), None);
                        array.map(|array| {
                            let datatype = array.data_type().clone();
                            (
                                ::arrow2::datatypes::Field::new("class_ids", datatype, false),
                                array,
                            )
                        })
                    })
                    .transpose()?
            },
            {
                self.keypoint_ids
                    .as_ref()
                    .map(|many| {
                        let array =
                            <crate::components::KeypointId>::try_to_arrow(many.iter(), None);
                        array.map(|array| {
                            let datatype = array.data_type().clone();
                            (
                                ::arrow2::datatypes::Field::new("keypoint_ids", datatype, false),
                                array,
                            )
                        })
                    })
                    .transpose()?
            },
            {
                self.instance_keys
                    .as_ref()
                    .map(|many| {
                        let array =
                            <crate::components::InstanceKey>::try_to_arrow(many.iter(), None);
                        array.map(|array| {
                            let datatype = array.data_type().clone();
                            (
                                ::arrow2::datatypes::Field::new("instance_keys", datatype, false),
                                array,
                            )
                        })
                    })
                    .transpose()?
            },
        ]
        .into_iter()
        .flatten()
        .collect())
    }

    #[inline]
    fn try_from_arrow(
        data: impl IntoIterator<Item = (::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>,
    ) -> crate::DeserializationResult<Self> {
        use crate::Component as _;
        let arrays_by_name: ::std::collections::HashMap<_, _> = data
            .into_iter()
            .map(|(field, array)| (field.name, array))
            .collect();
        let points = {
            let array = arrays_by_name.get("points").ok_or_else(|| {
                crate::DeserializationError::MissingData {
                    datatype: ::arrow2::datatypes::DataType::Null,
                }
            })?;
            <crate::components::Point2D>::try_from_arrow_opt(&**array)?
                .into_iter()
                .map(|v| {
                    v.ok_or_else(|| crate::DeserializationError::MissingData {
                        datatype: ::arrow2::datatypes::DataType::Null,
                    })
                })
                .collect::<crate::DeserializationResult<Vec<_>>>()?
        };
        let radii = if let Some(array) = arrays_by_name.get("radii") {
            Some(
                <crate::components::Radius>::try_from_arrow_opt(&**array)?
                    .into_iter()
                    .map(|v| {
                        v.ok_or_else(|| crate::DeserializationError::MissingData {
                            datatype: ::arrow2::datatypes::DataType::Null,
                        })
                    })
                    .collect::<crate::DeserializationResult<Vec<_>>>()?,
            )
        } else {
            None
        };
        let colors = if let Some(array) = arrays_by_name.get("colors") {
            Some(
                <crate::components::Color>::try_from_arrow_opt(&**array)?
                    .into_iter()
                    .map(|v| {
                        v.ok_or_else(|| crate::DeserializationError::MissingData {
                            datatype: ::arrow2::datatypes::DataType::Null,
                        })
                    })
                    .collect::<crate::DeserializationResult<Vec<_>>>()?,
            )
        } else {
            None
        };
        let labels = if let Some(array) = arrays_by_name.get("labels") {
            Some(
                <crate::components::Label>::try_from_arrow_opt(&**array)?
                    .into_iter()
                    .map(|v| {
                        v.ok_or_else(|| crate::DeserializationError::MissingData {
                            datatype: ::arrow2::datatypes::DataType::Null,
                        })
                    })
                    .collect::<crate::DeserializationResult<Vec<_>>>()?,
            )
        } else {
            None
        };
        let draw_order = if let Some(array) = arrays_by_name.get("draw_order") {
            Some(
                <crate::components::DrawOrder>::try_from_arrow_opt(&**array)?
                    .into_iter()
                    .next()
                    .flatten()
                    .ok_or_else(|| crate::DeserializationError::MissingData {
                        datatype: ::arrow2::datatypes::DataType::Null,
                    })?,
            )
        } else {
            None
        };
        let class_ids = if let Some(array) = arrays_by_name.get("class_ids") {
            Some(
                <crate::components::ClassId>::try_from_arrow_opt(&**array)?
                    .into_iter()
                    .map(|v| {
                        v.ok_or_else(|| crate::DeserializationError::MissingData {
                            datatype: ::arrow2::datatypes::DataType::Null,
                        })
                    })
                    .collect::<crate::DeserializationResult<Vec<_>>>()?,
            )
        } else {
            None
        };
        let keypoint_ids = if let Some(array) = arrays_by_name.get("keypoint_ids") {
            Some(
                <crate::components::KeypointId>::try_from_arrow_opt(&**array)?
                    .into_iter()
                    .map(|v| {
                        v.ok_or_else(|| crate::DeserializationError::MissingData {
                            datatype: ::arrow2::datatypes::DataType::Null,
                        })
                    })
                    .collect::<crate::DeserializationResult<Vec<_>>>()?,
            )
        } else {
            None
        };
        let instance_keys = if let Some(array) = arrays_by_name.get("instance_keys") {
            Some(
                <crate::components::InstanceKey>::try_from_arrow_opt(&**array)?
                    .into_iter()
                    .map(|v| {
                        v.ok_or_else(|| crate::DeserializationError::MissingData {
                            datatype: ::arrow2::datatypes::DataType::Null,
                        })
                    })
                    .collect::<crate::DeserializationResult<Vec<_>>>()?,
            )
        } else {
            None
        };
        Ok(Self {
            points,
            radii,
            colors,
            labels,
            draw_order,
            class_ids,
            keypoint_ids,
            instance_keys,
        })
    }
}

impl Points2D {
    pub fn new(points: impl IntoIterator<Item = impl Into<crate::components::Point2D>>) -> Self {
        Self {
            points: points.into_iter().map(Into::into).collect(),
            radii: None,
            colors: None,
            labels: None,
            draw_order: None,
            class_ids: None,
            keypoint_ids: None,
            instance_keys: None,
        }
    }

    pub fn with_radii(
        mut self,
        radii: impl IntoIterator<Item = impl Into<crate::components::Radius>>,
    ) -> Self {
        self.radii = Some(radii.into_iter().map(Into::into).collect());
        self
    }

    pub fn with_colors(
        mut self,
        colors: impl IntoIterator<Item = impl Into<crate::components::Color>>,
    ) -> Self {
        self.colors = Some(colors.into_iter().map(Into::into).collect());
        self
    }

    pub fn with_labels(
        mut self,
        labels: impl IntoIterator<Item = impl Into<crate::components::Label>>,
    ) -> Self {
        self.labels = Some(labels.into_iter().map(Into::into).collect());
        self
    }

    pub fn with_draw_order(mut self, draw_order: impl Into<crate::components::DrawOrder>) -> Self {
        self.draw_order = Some(draw_order.into());
        self
    }

    pub fn with_class_ids(
        mut self,
        class_ids: impl IntoIterator<Item = impl Into<crate::components::ClassId>>,
    ) -> Self {
        self.class_ids = Some(class_ids.into_iter().map(Into::into).collect());
        self
    }

    pub fn with_keypoint_ids(
        mut self,
        keypoint_ids: impl IntoIterator<Item = impl Into<crate::components::KeypointId>>,
    ) -> Self {
        self.keypoint_ids = Some(keypoint_ids.into_iter().map(Into::into).collect());
        self
    }

    pub fn with_instance_keys(
        mut self,
        instance_keys: impl IntoIterator<Item = impl Into<crate::components::InstanceKey>>,
    ) -> Self {
        self.instance_keys = Some(instance_keys.into_iter().map(Into::into).collect());
        self
    }
}
