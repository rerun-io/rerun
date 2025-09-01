#[cfg(test)]
mod tests {
    use re_data_loader::{DataLoaderSettings, LoadedData, loader_mcap::load_mcap};
    use re_mcap::layers::SelectedLayers;

    fn generate_ros2_mcap() -> Vec<u8> {
        let mut buffer = Vec::new();
        {
            let mut writer = mcap::Writer::new(std::io::Cursor::new(&mut buffer)).unwrap();

            // Register a schema
            let schema_name = "sensor_msgs/msg/CameraInfo";
            let schema_encoding = "ros2msg";
            let schema_data = br#"
            message TestSchema {
                optional x double = 1;
                optional y double = 2;
                optional z double = 3;
            }"#;
            let schema_id = writer
                .add_schema(schema_name, schema_encoding, schema_data)
                .expect("Failed to add schema");

            // Register a channel
            let channel_topic = "/test/points";
            let message_encoding = "cdr";
            let channel_id = writer
                .add_channel(
                    schema_id,
                    channel_topic,
                    message_encoding,
                    &std::collections::BTreeMap::new(),
                )
                .expect("Failed to add channel");

            // Write some test messages
            for i in 0..5 {
                let timestamp = 1000000000 + i * 100000000; // nanoseconds

                // Simple CDR-encoded point data (x=i, y=i*2, z=i*3)
                let mut message_data = Vec::new();
                message_data.extend_from_slice(&(i as f64).to_le_bytes()); // x
                message_data.extend_from_slice(&((i * 2) as f64).to_le_bytes()); // y
                message_data.extend_from_slice(&((i * 3) as f64).to_le_bytes()); // z

                let message_header = mcap::records::MessageHeader {
                    channel_id,
                    sequence: i as u32,
                    log_time: timestamp,
                    publish_time: timestamp,
                };
                writer
                    .write_to_known_channel(&message_header, &message_data)
                    .expect("Failed to write message");
            }

            writer.finish().unwrap();
        }
        buffer
    }

    #[test]
    fn test_load_generated_mcap_file() {
        env_logger::init();
        
        let mcap_data = generate_ros2_mcap();
        let (tx, rx) = std::sync::mpsc::channel();
        let settings = DataLoaderSettings::recommended("test");
        load_mcap(&mcap_data, &settings, &tx, SelectedLayers::All).unwrap();
        drop(tx);

        while let Ok(res) = rx.recv() {
            match res {
                LoadedData::Chunk(_, _, chunk) => {
                    println!("Generated MCAP chunk: {:?}", chunk);
                }
                _ => {} // LoadedData::LogMsg(_, log_msg) => println!("Generated MCAP result: {log_msg:#?}"),
                        // LoadedData::ArrowMsg(_, _, _) => println!("Generated MCAP arrow msg"),
            }
        }
    }
}
