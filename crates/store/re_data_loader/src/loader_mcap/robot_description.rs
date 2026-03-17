use std::collections::HashMap;

/// Minimal ROS2 String message for CDR deserialization.
#[derive(serde::Deserialize)]
struct RosString {
    data: String,
}

/// Scans the MCAP for `robot_description` topics carrying `std_msgs/msg/String`,
/// extracts the URDF XML, and emits 3D visualization chunks.
///
/// Note that transforms are not extracted from the URDF in this context.
/// These are expected to be present in the MCAP as separate transform messages.
///
/// TODO(michael): this could be implemented as an `re_mcap` decoder if the
/// core URDF parsing logic is moved to a separate crate outside of `re_data_loader`.
pub(crate) fn extract_urdf_from_robot_descriptions(
    mcap_bytes: &[u8],
    summary: &mcap::Summary,
    emit: &mut dyn FnMut(re_chunk::Chunk),
) -> anyhow::Result<()> {
    let robot_desc_channels: Vec<u16> = summary
        .channels
        .values()
        .filter(|channel| {
            channel.topic.contains("robot_description")
                && channel.schema.as_ref().is_some_and(|schema| {
                    schema.name == "std_msgs/msg/String" && schema.encoding == "ros2msg"
                })
        })
        .map(|channel| channel.id)
        .collect();

    if robot_desc_channels.is_empty() {
        return Ok(());
    }

    re_log::debug!(
        "Found {} robot_description channel(s), scanning messages…",
        robot_desc_channels.len()
    );

    let mut urdf_by_channel: HashMap<u16, String> = HashMap::new();

    for msg in mcap::MessageStream::new(mcap_bytes)? {
        let msg = msg?;
        if robot_desc_channels.contains(&msg.channel.id)
            && let Ok(decoded) = re_mcap::cdr::try_decode_message::<RosString>(&msg.data)
        {
            urdf_by_channel.insert(msg.channel.id, decoded.data);
        }
    }

    for urdf_xml in urdf_by_channel.into_values() {
        match crate::loader_urdf::build_urdf_chunks_from_xml(
            &urdf_xml,
            &None,
            &re_log_types::TimePoint::STATIC,
            false,
        ) {
            Ok(chunks) => {
                re_log::debug!(
                    "URDF extraction produced {} chunks from robot_description.",
                    chunks.len()
                );
                for chunk in chunks {
                    emit(chunk);
                }
            }
            Err(err) => {
                re_log::warn_once!("Failed to parse URDF from robot_description topic: {err}");
            }
        }
    }

    Ok(())
}
