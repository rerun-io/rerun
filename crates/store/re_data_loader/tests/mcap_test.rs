#[cfg(test)]
mod tests {
    use prost::Message;
    use prost_reflect::{
        DescriptorPool, DynamicMessage, FieldDescriptor, Kind, MessageDescriptor, Value,
    };
    use re_data_loader::{DataLoaderSettings, loader_mcap::load_mcap};
    use re_mcap::layers::SelectedLayers;

    fn generate_ros2_mcap() -> Vec<u8> {
        let mut buffer = Vec::new();
        {
            let mut writer = mcap::Writer::new(std::io::Cursor::new(&mut buffer)).unwrap();

            // Register a schema
            let schema_name = "TestProtoSchema";
            let schema_encoding = "protobuf";
            let schema_data = br#"
            message TestProtoSchema {
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

    fn generate_protobuf_mcap() -> Vec<u8> {
        let mut buffer = Vec::new();
        {
            let mut writer = mcap::Writer::new(std::io::Cursor::new(&mut buffer)).unwrap();

            // Register a schema
            let schema_name = "test-schema";
            let schema_encoding = "protobuf";
            // let schema_data = b"message TestProtoSchema { float64 x; float64 y; float64 z; }";
            let file_descriptor_set = prost_types::FileDescriptorSet {
                file: vec![prost_types::FileDescriptorProto {
                    name: Some("test-schema.proto".to_owned()),
                    syntax: Some("proto3".to_owned()),
                    message_type: vec![prost_types::DescriptorProto {
                        name: Some("TestProtoSchema".to_owned()),
                        field: vec![prost_types::FieldDescriptorProto {
                            name: Some("x".to_owned()),
                            number: Some(1),
                            r#type: Some(prost_types::field_descriptor_proto::Type::Double.into()),
                            ..Default::default()
                        }],
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
            };
            let schema_data = file_descriptor_set.encode_to_vec();

            let schema_id = writer
                .add_schema(schema_name, schema_encoding, &schema_data)
                .expect("Failed to add schema");

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

                // Simple CDR-encoded point data (x=i, y=i*2, z=i*3)
                let mut message_data = Vec::new();
                message_data.extend_from_slice(&(i as f64).to_le_bytes()); // x
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

            writer.finish().unwrap();
        }
        buffer
    }

    fn file_descriptor_proto_to_proto_text(
        file_desc_proto: &prost_types::FileDescriptorProto,
    ) -> String {
        // Create a descriptor pool and add the file
        let mut pool = DescriptorPool::new();
        pool.add_file_descriptor_proto(file_desc_proto.clone())
            .expect("Failed to decode file descriptor");

        // You can then access the file descriptor and extract information
        pool.get_file_by_name(file_desc_proto.name.as_ref().unwrap())
            .expect("Failed to get file descriptor");

        // Extract messages and build proto text manually
        let mut proto_content = String::new();

        if let Some(package) = &file_desc_proto.package {
            proto_content.push_str(&format!("package {};\n\n", package));
        }

        for message in &file_desc_proto.message_type {
            proto_content.push_str(&format!("message {} {{\n", message.name()));

            for (i, field) in message.field.iter().enumerate() {
                let field_type = match field.r#type() {
                    prost_types::field_descriptor_proto::Type::Double => "double",
                    prost_types::field_descriptor_proto::Type::Float => "float",
                    prost_types::field_descriptor_proto::Type::Int32 => "int32",
                    prost_types::field_descriptor_proto::Type::Int64 => "int64",
                    prost_types::field_descriptor_proto::Type::String => "string",
                    // Add more types as needed
                    _ => "unknown",
                };

                let label = match field.label() {
                    prost_types::field_descriptor_proto::Label::Optional => "optional",
                    prost_types::field_descriptor_proto::Label::Required => "required",
                    prost_types::field_descriptor_proto::Label::Repeated => "repeated",
                };

                proto_content.push_str(&format!(
                    "  {} {} {} = {};\n",
                    label,
                    field_type,
                    field.name(),
                    field.number()
                ));
            }

            proto_content.push_str("}\n\n");
        }

        proto_content
    }

    #[test]
    fn test_load_simple_protobuf_mcap_file() {
        let file = include_bytes!("assets/simple-protobuf.mcap");
        // let (tx, rx) = std::sync::mpsc::channel();
        // let settings = DataLoaderSettings::recommended("test");
        // load_mcap(file, &settings, &tx, SelectedLayers::All).unwrap();
        // drop(tx);
        // while let Ok(res) = rx.recv() {
        //     println!("res: {res:?}");
        // }

        let summary = mcap::read::Summary::read(file)
            .expect("Failed to read MCAP file")
            .unwrap();
        for (id, schema) in summary.schemas {
            if schema.encoding == "protobuf" {
                // let data = String::from_utf8_lossy(&schema.data);
                // println!("schema: {id}: {schema:?} {data}");

                let pool =
                    DescriptorPool::decode(schema.data.as_ref()).expect("Failed to decode schema");

                let message_descriptor = pool
                    .get_message_by_name(schema.name.as_str())
                    .expect("Failed to get message descriptor");

                let pkg = message_descriptor.parent_file_descriptor_proto();
                let mut set = prost_types::FileDescriptorSet {
                    file: vec![pkg.clone()],
                };
                let files: Vec<_> = pool
                    .files()
                    .filter(|f| f.file_descriptor_proto().name.as_ref() != pkg.name.as_ref())
                    .map(|f| f.file_descriptor_proto().clone())
                    .collect();
                set.file.extend(files);
                let pkg_data = set.encode_to_vec();
                println!("old: {:?}", schema.data);
                println!("new: {pkg_data:?}");

                let pool2 =
                    DescriptorPool::decode(pkg_data.as_ref()).expect("Failed to decode schema");
                let message_descriptor2 = pool2
                    .get_message_by_name(schema.name.as_str())
                    .expect("Failed to get message descriptor");

                println!("message_descriptor_1: {id}: {message_descriptor:#?}");
                println!("message_descriptor_2: {id}: {message_descriptor2:#?}");
            }
        }

        // let reader = mcap::read::LinearReader::new(file).expect("Failed to read MCAP file");
        // for record in reader.flatten() {
        //     // println!("record: {record:?}");
        //     match record {
        //         mcap::records::Record::Schema { header, data } => {
        //             println!("header: {header:?}");
        //             let data = String::from_utf8_lossy(&data);
        //             println!("data: {data}");
        //         }
        //         _ => {}
        //     }
        // if message.channel.topic == "pose" {
        //     println!("message: {message:?}");
        //     let data = String::from_utf8_lossy(&message.data);
        //     println!("message: {data}");
        // }
        // }

        // let messages = mcap::read::MessageStream::new(file).expect("Failed to read MCAP file");
        // for message in messages.flatten() {
        //     if message.channel.topic == "pose" {
        //         println!("message: {message:?}");
        //         let data = String::from_utf8_lossy(&message.data);
        //         println!("message: {data}");
        //     }
        // }
    }

    #[test]
    fn test_load_generated_mcap_file() {
        // let mcap_data = generate_ros2_mcap();
        let mcap_data = generate_protobuf_mcap();
        let (tx, rx) = std::sync::mpsc::channel();
        let settings = DataLoaderSettings::recommended("test");
        load_mcap(&mcap_data, &settings, &tx, SelectedLayers::All).unwrap();
        drop(tx);

        while let Ok(res) = rx.recv() {
            println!("Generated MCAP result: {res:?}");
        }
    }
}
