#[cfg(test)]
mod tests {
    use re_chunk::{Chunk, ChunkId};
    use re_data_loader::{DataLoaderSettings, LoadedData, loader_mcap::load_mcap};
    use re_mcap::{
        cdr::try_encode_message,
        layers::SelectedLayers,
        parsers::{dds::RepresentationIdentifier, ros2msg::{
            self,
            definitions::{
                geometry_msgs::{Quaternion, Vector3},
                sensor_msgs::{PointField, PointFieldDatatype},
            },
        }},
    };
    use serde::Serialize;

    fn make_header() -> ros2msg::definitions::std_msgs::Header {
        ros2msg::definitions::std_msgs::Header {
            stamp: ros2msg::definitions::builtin_interfaces::Time {
                sec: 123,
                nanosec: 45,
            },
            frame_id: "boo".to_owned(),
        }
    }

    fn generate_ros2_mcap<T: Serialize>(ros2_object: &T, schema_name: &str) -> Vec<u8> {
        let mut buffer = Vec::new();
        {
            let mut writer = mcap::Writer::new(std::io::Cursor::new(&mut buffer)).unwrap();

            // Register a schema
            let schema_encoding = "ros2msg";
            let schema_data = br#"ignored_schema_description"#;
            let schema_id = writer
                .add_schema(schema_name, schema_encoding, schema_data)
                .expect("Failed to add schema");

            // Register a channel
            let channel_topic = "/test/imu";
            let message_encoding = "cdr";
            let channel_id = writer
                .add_channel(
                    schema_id,
                    channel_topic,
                    message_encoding,
                    &std::collections::BTreeMap::new(),
                )
                .expect("Failed to add channel");

            let message_data =
                try_encode_message(&ros2_object, RepresentationIdentifier::CdrLittleEndian)
                    .expect("Failed to encode message");

            let timestamp = 1000000000;
            let message_header = mcap::records::MessageHeader {
                channel_id,
                sequence: 0,
                log_time: timestamp,
                publish_time: timestamp,
            };
            writer
                .write_to_known_channel(&message_header, &message_data)
                .expect("Failed to write message");

            writer.finish().expect("Failed to finish writer");
        }
        buffer
    }

    fn load_ros2_mcap<T: Serialize>(ros2_object: &T, schema_name: &str) -> Vec<Chunk> {
        let mcap_data = generate_ros2_mcap(&ros2_object, schema_name);
        let (tx, rx) = std::sync::mpsc::channel();
        let settings = DataLoaderSettings::recommended("test");
        load_mcap(&mcap_data, &settings, &tx, SelectedLayers::All).unwrap();
        drop(tx);

        // Collect chunks
        let mut chunks = vec![];
        let mut chunk_id = ChunkId::from_u128(123_456_789_123_456_789_123_456_789);
        while let Ok(res) = rx.recv() {
            if let LoadedData::Chunk(_, _, chunk) = res {
                let chunk = chunk.with_id(chunk_id).zeroed();
                chunks.push(chunk);
                chunk_id = chunk_id.next();
            }
        }
        chunks
    }

    #[test]
    fn test_ros2_mcap_imu() {
        let chunks = load_ros2_mcap(
            &ros2msg::definitions::sensor_msgs::Imu {
                header: make_header(),
                orientation: Quaternion::new(0.5, 0.1, 0.7, 1.125),
                orientation_covariance: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
                angular_velocity: Vector3::new(11.0, 12.0, 13.0),
                angular_velocity_covariance: [1.0; 9],
                linear_acceleration: Vector3::new(21.0, 22.0, 23.0),
                linear_acceleration_covariance: [2.0; 9],
            },
            "sensor_msgs/msg/Imu",
        );
        insta::assert_debug_snapshot!(chunks);
    }

    #[test]
    fn test_ros2_mcap_pointcloud2() {
        let height = 5u32;
        let width = 10u32;

        let points = (0..height * width)
            .map(|i| [i % width, i / width])
            .collect::<Vec<_>>();

        let fields = vec![
            PointField {
                name: "x".to_owned(),
                offset: 0,
                datatype: PointFieldDatatype::UInt32,
                count: 1,
            },
            PointField {
                name: "y".to_owned(),
                offset: std::mem::size_of::<u32>() as u32,
                datatype: PointFieldDatatype::UInt32,
                count: 1,
            },
        ];
        let data = points
            .iter()
            .flat_map(|p| [p[0].to_le_bytes(), p[1].to_le_bytes()])
            .flatten()
            .collect::<Vec<_>>();

        let chunks = load_ros2_mcap(
            &ros2msg::definitions::sensor_msgs::PointCloud2 {
                header: make_header(),
                height,
                width,
                fields,
                is_bigendian: false,
                point_step: std::mem::size_of::<[u32; 2]>() as u32,
                row_step: std::mem::size_of::<[u32; 2]>() as u32 * width,
                data,
                is_dense: true,
            },
            "sensor_msgs/msg/PointCloud2",
        );
        insta::assert_debug_snapshot!(chunks);
    }
}
