#[cfg(test)]
mod tests {
    use std::fs;

    use prost::Message;
    use prost_reflect::{
        DescriptorPool, DynamicMessage, FieldDescriptor, Kind, MessageDescriptor, Value,
    };
    use prost_types::FileDescriptorSet;
    use re_data_loader::{DataLoaderSettings, loader_mcap::load_mcap};
    use re_mcap::layers::SelectedLayers;

    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct TestProtoSchema {
        #[prost(double, tag = "1")]
        pub x: f64,
        #[prost(double, tag = "2")]
        pub y: f64,
        #[prost(double, tag = "3")]
        pub z: f64,
    }

    fn generate_proto_mcap() -> Vec<u8> {
        let mut buffer = Vec::new();
        {
            let mut writer = mcap::Writer::new(std::io::Cursor::new(&mut buffer))
                .expect("Failed to create MCAP writer");

            // Register a schema
            let schema_name = "TestProtoSchema";
            let schema_encoding = "protobuf";
            // let schema_data = br#"
            // syntax = "proto3";
            // message TestProtoSchema {
            //     double x = 1;
            //     double y = 2;
            //     double z = 3;
            // }"#;

            let descriptor_data =
                fs::read("tests/assets/file_descriptor_set").expect("Failed to read descriptors");
            let file_descriptor_set = FileDescriptorSet::decode(&descriptor_data[..])
                .expect("Failed to decode descriptors");
            let schema_data = file_descriptor_set.encode_to_vec();

            let schema_id = writer
                .add_schema(schema_name, schema_encoding, &schema_data)
                .expect("Failed to add schema");

            // let mut schema_raw = (schema_data.len() as u32).to_le_bytes().to_vec();
            // // schema_raw.extend_from_slice(schema_data);

            // let schema_id = writer
            //     .add_schema(schema_name, schema_encoding, schema_data)
            // .expect("Failed to add schema");

            // Register a channel
            let channel_topic = "/test/points";
            let message_encoding = "protobuf";
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

                let test_proto_schema = TestProtoSchema {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                };
                let mut message_data = Vec::new();
                test_proto_schema
                    .encode(&mut message_data)
                    .expect("Failed to encode message");

                // // Simple CDR-encoded point data (x=i, y=i*2, z=i*3)
                // let mut message_data = Vec::new();
                // message_data.extend_from_slice(&(i as f64).to_le_bytes()); // x
                // message_data.extend_from_slice(&((i * 2) as f64).to_le_bytes()); // y
                // message_data.extend_from_slice(&((i * 3) as f64).to_le_bytes()); // z

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

            writer.finish().expect("Failed to finish writer");
        }
        buffer
    }

    #[test]
    fn test_load_generated_mcap_file() {
        // let mcap_data = generate_ros2_mcap();
        let mcap_data = generate_proto_mcap();
        let (tx, rx) = std::sync::mpsc::channel();
        let settings = DataLoaderSettings::recommended("test");
        load_mcap(&mcap_data, &settings, &tx, SelectedLayers::All).expect("Failed to load MCAP");
        drop(tx);

        while let Ok(res) = rx.recv() {
            println!("Generated MCAP result: {res:?}");
        }
    }
}
