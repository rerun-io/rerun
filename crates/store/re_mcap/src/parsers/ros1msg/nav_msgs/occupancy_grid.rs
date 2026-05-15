use anyhow::ensure;
use re_chunk::{Chunk, ChunkId};
use re_sdk_types::archetypes::{CoordinateFrame, GridMap};
use re_sdk_types::components::{Colormap, RotationQuat, Translation3D};
use re_sdk_types::datatypes::{ChannelDatatype, ColorModel, ImageFormat, Quaternion};

use crate::parsers::decode::{MessageParser, ParserContext};
use crate::parsers::ros1msg::Ros1MessageParser;
use crate::parsers::ros1msg::definitions::nav_msgs;
use crate::parsers::ros1msg::wire::Ros1Reader;
use crate::util::TimestampCell;

pub struct OccupancyGridMessageParser {
    data: Vec<Vec<u8>>,
    formats: Vec<ImageFormat>,
    cell_sizes: Vec<f32>,
    translations: Vec<Translation3D>,
    quaternions: Vec<RotationQuat>,
    frame_ids: Vec<String>,
    colormaps: Vec<Colormap>,
}

impl Ros1MessageParser for OccupancyGridMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            data: Vec::with_capacity(num_rows),
            formats: Vec::with_capacity(num_rows),
            cell_sizes: Vec::with_capacity(num_rows),
            translations: Vec::with_capacity(num_rows),
            quaternions: Vec::with_capacity(num_rows),
            frame_ids: Vec::with_capacity(num_rows),
            colormaps: Vec::with_capacity(num_rows),
        }
    }
}

impl MessageParser for OccupancyGridMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let mut reader = Ros1Reader::new(&msg.data);
        let grid = nav_msgs::OccupancyGrid::read(&mut reader)?;
        reader.finish()?;

        ctx.add_timestamp_cell(TimestampCell::from_nanos_ros1(
            grid.header.stamp.as_nanos(),
            ctx.time_type(),
        ));

        let width = grid.info.width as usize;
        let height = grid.info.height as usize;
        let expected_len = width
            .checked_mul(height)
            .ok_or_else(|| anyhow::anyhow!("OccupancyGrid dimensions overflow"))?;
        ensure!(
            grid.data.len() == expected_len,
            "OccupancyGrid expected {expected_len} cells from {}x{}, got {}",
            grid.info.width,
            grid.info.height,
            grid.data.len()
        );

        self.data.push(ros_map_to_image_buffer(
            &grid.data,
            grid.info.width as usize,
            grid.info.height as usize,
        ));
        self.formats.push(ImageFormat::from_color_model(
            [grid.info.width, grid.info.height],
            ColorModel::L,
            ChannelDatatype::U8,
        ));
        self.cell_sizes.push(grid.info.resolution);
        self.translations.push(Translation3D::new(
            grid.info.origin.position.x as f32,
            grid.info.origin.position.y as f32,
            grid.info.origin.position.z as f32,
        ));
        self.quaternions.push(
            Quaternion::from_xyzw([
                grid.info.origin.orientation.x as f32,
                grid.info.origin.orientation.y as f32,
                grid.info.origin.orientation.z as f32,
                grid.info.origin.orientation.w as f32,
            ])
            .into(),
        );
        self.frame_ids.push(grid.header.frame_id);
        self.colormaps.push(Colormap::RvizMap);
        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>> {
        let Self {
            data,
            formats,
            cell_sizes,
            translations,
            quaternions,
            frame_ids,
            colormaps,
        } = *self;

        let mut components: Vec<_> = GridMap::update_fields()
            .with_many_data(data)
            .with_many_format(formats)
            .with_many_cell_size(cell_sizes)
            .with_many_translation(translations)
            .with_many_quaternion(quaternions)
            .with_many_colormap(colormaps)
            .columns_of_unit_batches()?
            .collect();

        components.extend(
            CoordinateFrame::update_fields()
                .with_many_frame(frame_ids)
                .columns_of_unit_batches()?,
        );

        Ok(vec![Chunk::from_auto_row_ids(
            ChunkId::new(),
            ctx.entity_path().clone(),
            ctx.build_timelines(),
            components.into_iter().collect(),
        )?])
    }
}

fn ros_map_to_image_buffer(data: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut image = Vec::with_capacity(data.len());
    for row in (0..height).rev() {
        let start = row * width;
        image.extend_from_slice(&data[start..start + width]);
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flips_ros_bottom_row_first_to_image_top_row_first() {
        assert_eq!(
            ros_map_to_image_buffer(&[1, 2, 3, 4, 5, 6], 3, 2),
            vec![4, 5, 6, 1, 2, 3]
        );
    }
}
