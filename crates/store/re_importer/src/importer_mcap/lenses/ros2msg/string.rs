use re_lenses::{Lens, LensBuilderError};
use re_lenses_core::Selector;
use re_log_types::TimeType;
use re_sdk_types::archetypes::TextDocument;

/// Creates a lens for `std_msgs/msg/String` messages.
///
/// The message has no header, so no ROS 2 timestamp or frame is extracted.
pub fn string(_time_type: TimeType) -> Result<Lens, LensBuilderError> {
    Lens::derive("std_msgs.msg.String:message")
        .to_component(TextDocument::descriptor_text(), Selector::parse(".data")?)
        .build()
}
