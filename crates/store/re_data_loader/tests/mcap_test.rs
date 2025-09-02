#[cfg(test)]
mod tests {
    use arrow::array::Float64Array;
    use re_chunk::{ArchetypeName, Chunk, ComponentIdentifier};
    use re_data_loader::{DataLoaderSettings, LoadedData, loader_mcap::load_mcap};
    use re_mcap::{cdr::try_encode_message, layers::SelectedLayers, parsers::ros2msg};

    fn make_imu() -> ros2msg::definitions::sensor_msgs::Imu {
        let header = ros2msg::definitions::std_msgs::Header {
            stamp: ros2msg::definitions::builtin_interfaces::Time {
                sec: 123,
                nanosec: 45,
            },
            frame_id: "boo".to_owned(),
        };
        let orientation = ros2msg::definitions::geometry_msgs::Quaternion {
            x: std::f64::consts::PI,
            y: 0.0,
            z: 0.0,
            w: 1.125,
        };
        ros2msg::definitions::sensor_msgs::Imu {
            header,
            orientation,
            orientation_covariance: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
            angular_velocity: ros2msg::definitions::geometry_msgs::Vector3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            angular_velocity_covariance: [0.0; 9],
            linear_acceleration: ros2msg::definitions::geometry_msgs::Vector3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            linear_acceleration_covariance: [0.0; 9],
        }
    }

    fn generate_ros2_mcap(imu: &ros2msg::definitions::sensor_msgs::Imu) -> Vec<u8> {
        let mut buffer = Vec::new();
        {
            let mut writer = mcap::Writer::new(std::io::Cursor::new(&mut buffer)).unwrap();

            // Register a schema
            let schema_name = "sensor_msgs/msg/Imu";
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

            let message_data = try_encode_message(&imu).expect("Failed to encode message");

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

            writer.finish().unwrap();
        }
        buffer
    }

    fn assert_component(chunks: &[Chunk], archetype: &str, component: &str, value: &[f64]) {
        let archetype_name = ArchetypeName::new(archetype);
        let component_identifier = ComponentIdentifier::new(&format!("{archetype}:{component}"));

        for chunk in chunks {
            for (component_desc, rows) in chunk.components().iter() {
                if component_desc.archetype == Some(archetype_name)
                    && component_desc.component == component_identifier
                {
                    println!("Found component {archetype}:{component}");
                    // Only check the first row for now
                    let cell = rows
                        .iter()
                        .flatten()
                        .next()
                        .expect("Expected at least one row");
                    let float_array = cell
                        .as_any()
                        .downcast_ref::<Float64Array>()
                        .expect("Expected cell to be a Float64Array");
                    let cell_value = float_array.values().as_ref();
                    assert_eq!(cell_value, value);
                    return;
                }
            }
        }
        panic!("Component {archetype}:{component} not found");
    }

    #[test]
    fn test_load_generated_mcap_file() {
        env_logger::init();
        let imu = make_imu();

        let mcap_data = generate_ros2_mcap(&imu);
        let (tx, rx) = std::sync::mpsc::channel();
        let settings = DataLoaderSettings::recommended("test");
        load_mcap(&mcap_data, &settings, &tx, SelectedLayers::All).unwrap();
        drop(tx);

        // Collect chunks
        let mut chunks = vec![];
        while let Ok(res) = rx.recv() {
            if let LoadedData::Chunk(_, _, chunk) = res {
                chunks.push(chunk);
            }
        }

        println!("CHUNKS: {chunks:?}");

        // Assert chunk contents
        assert_component(
            &chunks,
            "sensor_msgs.msg.Imu",
            "orientation",
            &[
                imu.orientation.x,
                imu.orientation.y,
                imu.orientation.z,
                imu.orientation.w,
            ],
        );

        assert_component(
            &chunks,
            "sensor_msgs.msg.Imu",
            "orientation_covariance",
            &imu.orientation_covariance,
        );

        // FOUND A BUG! We don't load angular_velocity.
        assert_component(
            &chunks,
            "sensor_msgs.msg.Imu",
            "angular_velocity",
            &[
                imu.angular_velocity.x,
                imu.angular_velocity.y,
                imu.angular_velocity.z,
            ],
        );
    }
}
