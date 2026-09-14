use re_lenses_core::Selector;
use re_sdk_types::archetypes::{CoordinateFrame, DepthImage, Image};

use crate::{Lens, LensBuilderError, op};

use super::IMAGE_PLANE_SUFFIX;
use super::ros_image_helpers::{ImageKind, encoding_to_image_format_if, extract_image_buffer_if};

/// Creates a lens for `sensor_msgs/msg/Image` messages.
///
/// Emits either [`Image`] or [`DepthImage`] components based on encoding, suppressing the other.
pub fn image() -> Result<Lens, LensBuilderError> {
    Lens::derive("sensor_msgs.msg.Image:message")
        .to_component(
            CoordinateFrame::descriptor_frame(),
            Selector::parse(".header.frame_id")?
                .pipe(op::string_suffix_nonempty(IMAGE_PLANE_SUFFIX)),
        )
        .to_component(
            Image::descriptor_format(),
            Selector::parse(".")?.pipe(encoding_to_image_format_if(ImageKind::Color)),
        )
        .to_component(
            Image::descriptor_buffer(),
            Selector::parse(".")?.pipe(extract_image_buffer_if(ImageKind::Color)),
        )
        .to_component(
            DepthImage::descriptor_format(),
            Selector::parse(".")?.pipe(encoding_to_image_format_if(ImageKind::Depth)),
        )
        .to_component(
            DepthImage::descriptor_buffer(),
            Selector::parse(".")?.pipe(extract_image_buffer_if(ImageKind::Depth)),
        )
        .build()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::{
        Array as _, ArrayRef, ListArray, ListBuilder, StringArray, StructArray, UInt8Builder,
        UInt32Array,
    };
    use arrow::buffer::OffsetBuffer;
    use arrow::datatypes::{DataType, Field, Fields};
    use re_chunk::{Chunk, ChunkId};
    use re_sdk_types::ComponentDescriptor;

    use crate::{ChunkExt as _, default_runtime};

    use super::*;

    /// Test helper for creating a chunk of one-pixel ROS image messages with valid buffers.
    ///
    /// The buffer lengths must match the encoding because the image lens validates them while
    /// extracting the image data.
    fn image_chunk(encodings: &[&str]) -> Chunk {
        let len = encodings.len();
        let mut data = ListBuilder::new(UInt8Builder::new())
            .with_field(Arc::new(Field::new_list_field(DataType::UInt8, false)));
        for encoding in encodings {
            data.values().append_slice(match *encoding {
                "rgb8" => &[0, 0, 0], // One 8-bit red, green, and blue channel.
                "16UC1" => &[0, 0],   // One 16-bit unsigned single channel.
                _ => unreachable!("test helper only supports rgb8 and 16UC1"),
            });
            data.append(true);
        }
        let data = data.finish();
        let headers = StructArray::new(
            Fields::from(vec![Field::new("frame_id", DataType::Utf8, false)]),
            vec![Arc::new(StringArray::from(vec!["camera"; len]))],
            None,
        );
        let messages = StructArray::new(
            Fields::from(vec![
                Field::new("header", headers.data_type().clone(), false),
                Field::new("height", DataType::UInt32, false),
                Field::new("width", DataType::UInt32, false),
                Field::new("encoding", DataType::Utf8, false),
                Field::new("step", DataType::UInt32, false),
                Field::new("data", data.data_type().clone(), false),
            ]),
            vec![
                Arc::new(headers),
                Arc::new(UInt32Array::from(vec![1; len])),
                Arc::new(UInt32Array::from(vec![1; len])),
                Arc::new(StringArray::from(encodings.to_vec())),
                Arc::new(UInt32Array::from(vec![0; len])),
                Arc::new(data),
            ],
            None,
        );
        let values: ArrayRef = Arc::new(messages);
        let input = ListArray::new(
            Arc::new(Field::new_list_field(values.data_type().clone(), false)),
            OffsetBuffer::new(
                (0..=i32::try_from(len).expect("test image count must fit in i32"))
                    .collect::<Vec<_>>()
                    .into(),
            ),
            values,
            None,
        );

        Chunk::from_auto_row_ids(
            ChunkId::new(),
            "camera/image".into(),
            Default::default(),
            std::iter::once((
                ComponentDescriptor::partial("sensor_msgs.msg.Image:message"),
                input,
            ))
            .collect(),
        )
        .unwrap()
    }

    fn output_for(encodings: &[&str]) -> Chunk {
        let outputs = image_chunk(encodings)
            .apply_lenses(&[image().unwrap()], &default_runtime())
            .unwrap();
        assert_eq!(outputs.len(), 1);
        outputs.into_iter().next().unwrap()
    }

    /// Checks that color images emit only regular image components.
    #[test]
    fn suppresses_depth_outputs_for_color_images() {
        let output = output_for(&["rgb8", "rgb8"]);

        assert!(
            output
                .components()
                .contains_component("Image:format".into())
        );
        assert!(
            output
                .components()
                .contains_component("Image:buffer".into())
        );
        assert!(
            !output
                .components()
                .contains_component("DepthImage:format".into())
        );
        assert!(
            !output
                .components()
                .contains_component("DepthImage:buffer".into())
        );
    }

    /// Checks that depth images emit only depth image components.
    #[test]
    fn suppresses_color_outputs_for_depth_images() {
        let output = output_for(&["16UC1", "16UC1"]);

        assert!(
            !output
                .components()
                .contains_component("Image:format".into())
        );
        assert!(
            !output
                .components()
                .contains_component("Image:buffer".into())
        );
        assert!(
            output
                .components()
                .contains_component("DepthImage:format".into())
        );
        assert!(
            output
                .components()
                .contains_component("DepthImage:buffer".into())
        );
    }

    /// Checks that a chunk cannot mix regular and depth image encodings.
    #[test]
    fn rejects_mixed_image_kinds() {
        assert!(
            image_chunk(&["rgb8", "16UC1"])
                .apply_lenses(&[image().unwrap()], &default_runtime())
                .is_err()
        );
    }
}
