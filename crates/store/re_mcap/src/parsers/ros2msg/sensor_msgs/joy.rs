use re_chunk::external::arrow::array::{Float64Builder, ListBuilder, StringBuilder};
use re_chunk::{Chunk, ChunkId};
use re_sdk_types::archetypes::{CoordinateFrame, Scalars};

use super::super::Ros2MessageParser;
use super::super::definitions::sensor_msgs;
use crate::Error;
use crate::parsers::{MessageParser, ParserContext, cdr};

pub struct JoyMessageParser {
    axes: ListBuilder<Float64Builder>,
    buttons: ListBuilder<Float64Builder>,
    frame_ids: ListBuilder<StringBuilder>,
}

impl Ros2MessageParser for JoyMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            axes: ListBuilder::with_capacity(Float64Builder::new(), num_rows),
            buttons: ListBuilder::with_capacity(Float64Builder::new(), num_rows),
            frame_ids: ListBuilder::with_capacity(StringBuilder::new(), num_rows),
        }
    }
}

impl MessageParser for JoyMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let sensor_msgs::Joy {
            header,
            axes,
            buttons,
        } = cdr::try_decode_message::<sensor_msgs::Joy>(msg.data.as_ref())
            .map_err(|err| Error::Other(anyhow::anyhow!(err)))?;

        // add the sensor timestamp to the context, `log_time` and `publish_time` are added automatically
        ctx.add_timestamp_cell(crate::util::TimestampCell::guess_from_nanos_ros2(
            header.stamp.as_nanos() as u64,
        ));

        self.frame_ids.values().append_value(header.frame_id);
        self.frame_ids.append(true);

        // Convert f32 axes to f64 for Scalars
        for axis_value in &axes {
            self.axes.values().append_value(*axis_value as f64);
        }
        self.axes.append(true);

        // Convert i32 buttons to f64 for Scalars
        for button_value in &buttons {
            self.buttons.values().append_value(*button_value as f64);
        }
        self.buttons.append(true);

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>> {
        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let Self {
            mut axes,
            mut buttons,
            mut frame_ids,
        } = *self;

        let axes_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone() / "axes",
            timelines.clone(),
            std::iter::once((Scalars::descriptor_scalars(), axes.finish())).collect(),
        )?;

        let buttons_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone() / "buttons",
            timelines.clone(),
            std::iter::once((Scalars::descriptor_scalars(), buttons.finish())).collect(),
        )?;

        let frame_ids_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path,
            timelines,
            std::iter::once((CoordinateFrame::descriptor_frame(), frame_ids.finish())).collect(),
        )?;

        Ok(vec![axes_chunk, buttons_chunk, frame_ids_chunk])
    }
}
