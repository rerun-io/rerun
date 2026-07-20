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
/// core URDF parsing logic is moved to a separate crate outside of `re_importer`.
pub(crate) fn extract_urdf_from_robot_descriptions(
    mcap_bytes: &[u8],
    summary: &mcap::Summary,
    topic_filter: &re_mcap::TopicFilter,
    recover: bool,
    emit: &(dyn Fn(re_chunk::Chunk) + Send + Sync),
) -> anyhow::Result<()> {
    let robot_desc_channels: Vec<u16> = summary
        .channels
        .values()
        .filter(|channel| {
            topic_filter.matches(&channel.topic)
                && channel.topic.contains("robot_description")
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

    // In recover mode, tolerate a truncated tail: don't require the end magic, and stop at the
    // first read error rather than failing, keeping the URDF collected before the tail.
    let messages = if recover {
        mcap::MessageStream::new_with_options(
            mcap_bytes,
            mcap::read::Options::IgnoreEndMagic.into(),
        )?
    } else {
        mcap::MessageStream::new(mcap_bytes)?
    };

    for msg in messages {
        let msg = match msg {
            Ok(msg) => msg,
            Err(err) if recover => {
                re_log::warn!("Stopping URDF scan at a truncated/corrupt MCAP tail: {err}");
                break;
            }
            Err(err) => return Err(err.into()),
        };
        if robot_desc_channels.contains(&msg.channel.id)
            && let Ok(decoded) = re_mcap::cdr::try_decode_message::<RosString>(&msg.data)
        {
            urdf_by_channel.insert(msg.channel.id, decoded.data);
        }
    }

    for urdf_xml in urdf_by_channel.into_values() {
        match crate::importer_urdf::build_urdf_chunks_from_xml(
            &urdf_xml,
            None,
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    /// Encodes `s` as a CDR little-endian `std_msgs/msg/String` message body (4-byte encapsulation
    /// header + null-terminated, length-prefixed string), matching what `re_cdr` decodes.
    fn cdr_string_message(s: &str) -> Vec<u8> {
        let mut payload = vec![0x00, 0x01, 0x00, 0x00]; // CDR_LE representation id + options
        let bytes = s.as_bytes();
        payload.extend_from_slice(&(bytes.len() as u32 + 1).to_le_bytes());
        payload.extend_from_slice(bytes);
        payload.push(0); // null terminator (included in the length above)
        payload
    }

    /// A `robot_description` message written before the tail was cut must still yield its URDF
    /// under recovery, rather than being lost when the scan hits the truncated tail.
    #[test]
    fn recover_urdf_survives_truncated_tail() {
        const URDF: &str = r#"<robot name="r"><link name="base"><visual><geometry><box size="1 1 1"/></geometry></visual></link></robot>"#;

        // Write a healthy single-chunk MCAP with one `/robot_description` `std_msgs/msg/String`.
        let cursor = Cursor::new(Vec::new());
        let mut writer = mcap::Writer::new(cursor).expect("writer");
        let schema_id = writer
            .add_schema("std_msgs/msg/String", "ros2msg", b"string data")
            .expect("schema");
        let channel_id = writer
            .add_channel(schema_id, "/robot_description", "cdr", &Default::default())
            .expect("channel");
        writer
            .write_to_known_channel(
                &mcap::records::MessageHeader {
                    channel_id,
                    sequence: 0,
                    log_time: 1,
                    publish_time: 1,
                },
                &cdr_string_message(URDF),
            )
            .expect("message");
        writer.flush().expect("flush");
        writer.finish().expect("finish");
        let buffer = writer.into_inner().into_inner();

        // Truncate at the summary section: the data section (chunk + message index) survives.
        let footer = mcap::read::footer(&buffer).expect("footer");
        let truncated = &buffer[..footer.summary_start as usize];

        let summary = re_mcap::read_or_reconstruct_summary(truncated, true).expect("reconstruct");

        let chunks = parking_lot::Mutex::new(Vec::new());
        super::extract_urdf_from_robot_descriptions(
            truncated,
            &summary,
            &re_mcap::TopicFilter::default(),
            true, // recover
            &|chunk| chunks.lock().push(chunk),
        )
        .expect("URDF extraction should not error on a truncated file in recover mode");

        assert!(
            !chunks.lock().is_empty(),
            "expected URDF chunks to be emitted from the truncated file"
        );
    }
}
