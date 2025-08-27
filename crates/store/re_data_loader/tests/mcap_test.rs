#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_data_loader::{DataLoaderSettings, loader_mcap::load_mcap};
    use re_mcap::layers::SelectedLayers;

    fn generate_test_mcap() -> Vec<u8> {
        let mut buffer = Vec::new();
        {
            let mut writer = mcap::Writer::new(std::io::Cursor::new(&mut buffer)).unwrap();

            // Write header
            let header = mcap::records::Header {
                profile: "test".to_string(),
                library: "rerun_test".to_string(),
            };
            // writer.write_private_record(&header).unwrap();

            // Register a schema
            let schema = mcap::Schema {
                id: 42,
                name: "geometry_msgs/Point".to_string(),
                encoding: "ros2msg".to_string(),
                data: "float64 x\nfloat64 y\nfloat64 z".as_bytes().to_vec().into(),
            };
            // let schema_id = writer.write_schema(&schema).unwrap();
            // let schema_id = writer.write(&schema).unwrap();

            // Register a channel
            let channel = mcap::Channel {
                topic: "/test/points".to_string(),
                message_encoding: "cdr".to_string(),
                metadata: std::collections::BTreeMap::new(),
                id: 43,
                schema: Some(Arc::new(schema)),
                // schema_id: 42,
            };
            let channel_id = writer
                .add_channel(
                    43,
                    "/test/points",
                    "cdr",
                    &std::collections::BTreeMap::new(),
                )
                .unwrap();
            let channel = Arc::new(channel);

            // Write some test messages
            for i in 0..5 {
                let timestamp = 1000000000 + i * 100000000; // nanoseconds

                // Simple CDR-encoded point data (x=i, y=i*2, z=i*3)
                let mut message_data = Vec::new();
                message_data.extend_from_slice(&(i as f64).to_le_bytes()); // x
                message_data.extend_from_slice(&((i * 2) as f64).to_le_bytes()); // y
                message_data.extend_from_slice(&((i * 3) as f64).to_le_bytes()); // z

                let message = mcap::Message {
                    channel: channel.clone(),
                    sequence: i as u32,
                    log_time: timestamp,
                    publish_time: timestamp,
                    data: message_data.into(),
                };
                writer.write(&message).unwrap();
            }

            writer.finish().unwrap();
        }
        buffer
    }

    #[test]
    fn test_load_simple_protobuf_mcap_file() {
        let file = include_bytes!("assets/simple-protobuf.mcap");
        let (tx, rx) = std::sync::mpsc::channel();
        let settings = DataLoaderSettings::recommended("test");
        load_mcap(file, &settings, &tx, SelectedLayers::All).unwrap();
        let res = rx.recv().unwrap();
        println!("res: {:#?}", res);
    }

    #[test]
    fn test_load_generated_mcap_file() {
        let mcap_data = generate_test_mcap();
        let (tx, rx) = std::sync::mpsc::channel();
        let settings = DataLoaderSettings::recommended("test");
        load_mcap(&mcap_data, &settings, &tx, SelectedLayers::All).unwrap();
        let res = rx.recv().unwrap();
        println!("Generated MCAP result: {:#?}", res);
    }
}
