mod foxglove;
mod helpers;
mod image_helpers;
mod ros2msg;

pub use crate::FOXGLOVE_LENSES_IDENTIFIER;

use re_lenses::{LensBuilderError, Lenses, OutputMode};
use re_log_types::TimeType;
use re_mcap::{DecoderIdentifier, SelectedDecoders};

const ROS2MSG_DECODER_IDENTIFIER: &str = "ros2msg";

pub fn mcap_lenses(
    selected_decoders: &SelectedDecoders,
    time_type: TimeType,
) -> Result<Option<Lenses>, LensBuilderError> {
    let mut lenses = Lenses::new(OutputMode::ForwardUnmatched);

    let has_foxglove_lenses = if selected_decoders
        .contains(&DecoderIdentifier::from(crate::FOXGLOVE_LENSES_IDENTIFIER))
    {
        foxglove::add_foxglove_lenses(&mut lenses, time_type)?;
        true
    } else {
        false
    };

    let has_ros2msg_lenses =
        if selected_decoders.contains(&DecoderIdentifier::from(ROS2MSG_DECODER_IDENTIFIER)) {
            ros2msg::add_ros2msg_lenses(&mut lenses, time_type)?;
            true
        } else {
            false
        };

    Ok((has_foxglove_lenses || has_ros2msg_lenses).then_some(lenses))
}
