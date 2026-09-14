use std::collections::hash_map::Entry;
use std::sync::Arc;

use arrow::array::FixedSizeListArray;
use re_chunk::{Chunk, ChunkId};
use re_ros_msg::MessageSchema;
use re_ros_msg::message_spec::{BuiltInType, Type};
use re_ros_msg::reflection::{CdrArrowDecoder, CdrDecodeError, MessageDecodePlan};
use re_sdk_types::reflection::ComponentDescriptorExt as _;
use re_sdk_types::{ArchetypeName, ComponentDescriptor};

use super::ros2::supports_ros2_cdr_channel;
use crate::parsers::{MessageParser, ParserContext};
use crate::{DecoderIdentifier, Error, MessageDecoder};

struct Ros2ReflectionMessageParser {
    decoder: CdrArrowDecoder,

    /// Set if the Arrow builders could not be returned to a row boundary after a failure.
    ///
    /// Only reachable through a plan/builder mismatch, which would be a bug on our side rather
    /// than bad data. Recorded so we never hand out a structurally invalid chunk.
    unrecoverable_builder_error: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum Ros2ReflectionError {
    #[error("Invalid message on channel {channel} for schema {schema}: {source:#}")]
    InvalidMessage {
        schema: String,
        channel: String,
        source: anyhow::Error,
    },
}

impl Ros2ReflectionMessageParser {
    fn new(num_rows: usize, decode_plan: Arc<MessageDecodePlan>) -> Self {
        Self {
            decoder: CdrArrowDecoder::new(decode_plan, num_rows),
            unrecoverable_builder_error: false,
        }
    }
}

impl MessageParser for Ros2ReflectionMessageParser {
    fn append(&mut self, _ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        re_tracing::profile_function!();

        anyhow::ensure!(
            !self.unrecoverable_builder_error,
            "ROS 2 reflection parser cannot decode after an unrecoverable Arrow builder error"
        );

        match self.decoder.decode_message(msg.data.as_ref()) {
            Ok(()) => Ok(()),

            // A single corrupt message must not cost us the rest of the channel. Its row is
            // already cancelled, so the next message can be decoded as usual.
            Err(CdrDecodeError::Message(source)) => Err(Ros2ReflectionError::InvalidMessage {
                schema: self.decoder.plan().schema_name().to_owned(),
                channel: msg.channel.topic.clone(),
                source,
            }
            .into()),

            Err(CdrDecodeError::Unrecoverable(err)) => {
                self.unrecoverable_builder_error = true;
                Err(anyhow::Error::new(err).context(format!(
                    "failed to restore the Arrow builders for ROS 2 channel {}",
                    msg.channel.topic
                )))
            }
        }
    }

    fn finalize(self: Box<Self>, mut ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        re_tracing::profile_function!();

        let Self {
            mut decoder,
            unrecoverable_builder_error,
        } = *self;

        anyhow::ensure!(
            !unrecoverable_builder_error,
            "ROS 2 reflection parser cannot finalize after an unrecoverable Arrow builder error"
        );

        // Cancelled rows are already gone from `messages`, so it stays aligned with the timelines
        // that `ctx` builds.
        let messages = decoder.finish();
        add_ros2_timestamps(&mut ctx, decoder.plan(), &messages);

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();
        let archetype_name =
            ArchetypeName::try_new(decoder.plan().schema_name().replace('/', "."))?;

        let message_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path,
            timelines,
            std::iter::once((
                ComponentDescriptor::partial("message").with_builtin_archetype(archetype_name),
                messages.into(),
            ))
            .collect(),
        )
        .map_err(Error::other)?;

        Ok(vec![message_chunk])
    }
}

/// True if any field (including inside arrays) is a `wstring`, which we can't decode.
///
/// Nested messages live in `dependencies` and are walked directly, so we don't recurse
/// into `Type::Complex`.
fn schema_uses_wstring(message_schema: &MessageSchema) -> bool {
    fn type_uses_wstring(ty: &Type) -> bool {
        match ty {
            Type::BuiltIn(BuiltInType::WString(_)) => true,
            Type::BuiltIn(_) | Type::Complex(_) => false,
            Type::Array { ty, .. } => type_uses_wstring(ty),
        }
    }

    std::iter::chain(
        std::iter::once(&message_schema.spec),
        &message_schema.dependencies,
    )
    .flat_map(|spec| &spec.fields)
    .any(|field| type_uses_wstring(&field.ty))
}

/// Builds the decode plan for one `ros2msg` MCAP schema.
///
/// Returns `Ok(None)` for schemas that reflection cannot handle. Those channels stay unregistered
/// and fall back to the raw decoder, rather than failing the whole file.
fn decode_plan_from_schema(
    schema: &mcap::Schema<'_>,
) -> Result<Option<Arc<MessageDecodePlan>>, Error> {
    let schema_content = String::from_utf8_lossy(schema.data.as_ref());
    let message_schema = MessageSchema::parse(&schema.name, &schema_content).map_err(|err| {
        Error::InvalidSchema {
            schema: schema.name.clone(),
            source: err,
        }
    })?;

    if schema_uses_wstring(&message_schema) {
        // `wstring` is UTF-16 on the wire, so decoding it would corrupt the rest of the message.
        re_log::warn_once!(
            "ROS 2 schema '{}' uses `wstring`, which reflection cannot decode. Keeping its channels as raw data.",
            schema.name
        );
        return Ok(None);
    }

    match MessageDecodePlan::from_schema(&message_schema) {
        Ok(decode_plan) => Ok(Some(Arc::new(decode_plan))),
        Err(err) => {
            // An unresolvable schema — e.g. one whose dependencies are missing from the MCAP.
            re_log::warn_once!(
                "ROS 2 schema '{}' cannot be resolved by reflection. Keeping its channels as raw data: {err:#}",
                schema.name
            );
            Ok(None)
        }
    }
}

/// Provides reflection-based conversion of ROS 2-encoded MCAP messages.
///
/// This decoder dynamically parses ROS 2 messages at runtime, allowing for
/// a direct arrow representation of the messages fields, similar to the protobuf decoder.
#[derive(Debug, Default)]
pub struct McapRos2ReflectionDecoder {
    plans_per_channel: ahash::HashMap<u16, Arc<MessageDecodePlan>>,
}

impl MessageDecoder for McapRos2ReflectionDecoder {
    fn identifier() -> DecoderIdentifier {
        "ros2_reflection".into()
    }

    fn init(&mut self, summary: &mcap::Summary) -> Result<(), Error> {
        // Channels routinely share one schema, so key the work on the MCAP schema ID and hand the
        // resulting plan out by reference.
        let mut plans_per_schema: ahash::HashMap<u16, Option<Arc<MessageDecodePlan>>> =
            ahash::HashMap::default();

        for channel in summary.channels.values() {
            let Some(schema) = channel.schema.as_ref() else {
                continue;
            };

            if schema.encoding.as_str() != "ros2msg" {
                continue;
            }

            let decode_plan = match plans_per_schema.entry(schema.id) {
                Entry::Occupied(entry) => entry.get().clone(),
                Entry::Vacant(entry) => entry.insert(decode_plan_from_schema(schema)?).clone(),
            };

            if let Some(decode_plan) = decode_plan {
                self.plans_per_channel.insert(channel.id, decode_plan);
            }
        }

        Ok(())
    }

    /// Claims _any_ ROS 2 channel that is supported by reflection.
    ///
    /// Note: if semantic parsing is enabled, [`crate::decoders::DecoderRegistry::plan`]
    /// takes care of selecting [`crate::decoders::ros2::McapRos2Decoder`] instead for
    /// schemas supported by semantic parsing.
    fn supports_channel(&self, channel: &mcap::Channel<'_>) -> bool {
        let Some(schema) = channel.schema.as_ref() else {
            return false;
        };

        if schema.encoding.as_str() != "ros2msg" {
            return false;
        }

        // First check if we have parsed the schema successfully
        if !self.plans_per_channel.contains_key(&channel.id) {
            return false;
        }

        supports_ros2_cdr_channel(channel)
    }

    fn message_parser(
        &self,
        channel: &mcap::Channel<'_>,
        num_rows: usize,
    ) -> Option<Box<dyn MessageParser>> {
        let decode_plan = Arc::clone(self.plans_per_channel.get(&channel.id)?);
        Some(Box::new(Ros2ReflectionMessageParser::new(
            num_rows,
            decode_plan,
        )))
    }
}

/// Adds one ROS timestamp timeline row for each decoded Arrow message row.
fn add_ros2_timestamps(
    ctx: &mut ParserContext,
    plan: &MessageDecodePlan,
    messages: &FixedSizeListArray,
) {
    // `Chunk::from_auto_row_ids` indexes every timeline by the message's row count, so a
    // timestamp we cannot read drops the `ros2_timestamp` timeline rather than shortening it.
    let nanos = match plan.timestamp_nanos(messages) {
        Ok(Some(nanos)) => nanos,

        // This message doesn't carry a header or top-level timestamp.
        Ok(None) => return,

        Err(err) => {
            re_log::warn_once!(
                "{err}, so the `ros2_timestamp` timeline is dropped.\nMCAP channel: {}",
                ctx.channel_topic()
            );
            return;
        }
    };

    let time_type = ctx.time_type();
    for nanos in nanos {
        ctx.add_timestamp_cell(crate::util::TimestampCell::from_nanos_ros2(
            nanos, time_type,
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, collections::BTreeMap};

    use arrow::array::{Int32Array, StructArray};
    use re_arrow_util::ArrowArrayDowncastRef as _;
    use re_chunk::EntityPath;
    use re_log_types::TimeType;
    use re_ros_msg::MessageSchema;

    use super::*;

    #[test]
    fn detects_wstring_in_schema() {
        let plain = MessageSchema::parse("test/Msg", "string s\nint32 n\n").unwrap();
        assert!(!schema_uses_wstring(&plain));

        let scalar = MessageSchema::parse("test/Msg", "wstring w\n").unwrap();
        assert!(schema_uses_wstring(&scalar));

        let array = MessageSchema::parse("test/Msg", "wstring[] w\n").unwrap();
        assert!(schema_uses_wstring(&array));

        let nested = MessageSchema::parse(
            "test/Outer",
            r#"
test/Inner inner

================================================================================
MSG: test/Inner
wstring w
"#,
        )
        .unwrap();
        assert!(schema_uses_wstring(&nested));
    }

    /// Checks that a corrupt timestamp drops the whole `ros2_timestamp` timeline
    /// instead of leaving an incomplete (invalid) timeline.
    #[test]
    fn unrepresentable_stamp_drops_the_ros2_timestamp_timeline() {
        let schema = MessageSchema::parse(
            "test/Message",
            r#"
builtin_interfaces/Time stamp

================================================================================
MSG: builtin_interfaces/Time
int32 sec
uint32 nanosec
"#,
        )
        .unwrap();
        let plan = Arc::new(MessageDecodePlan::from_schema(&schema).unwrap());
        let mut parser = Ros2ReflectionMessageParser::new(2, plan);
        let mut ctx = ParserContext::new(EntityPath::from("/test"), "/test", TimeType::TimestampNs);
        let channel = Arc::new(mcap::Channel {
            id: 1,
            topic: "/test".to_owned(),
            schema: None,
            message_encoding: "cdr".to_owned(),
            metadata: BTreeMap::new(),
        });

        let message = |sec: i32| mcap::Message {
            channel: Arc::clone(&channel),
            sequence: 0,
            log_time: 0,
            publish_time: 0,
            data: Cow::Owned(
                [
                    &[0x00, 0x01, 0x00, 0x00][..], // CDR LE header
                    &sec.to_le_bytes(),
                    &0_u32.to_le_bytes(),
                ]
                .concat(),
            ),
        };

        // One stamp that converts, and one that `u64::try_from` rejects.
        parser.append(&mut ctx, &message(2)).unwrap();
        parser.append(&mut ctx, &message(-1)).unwrap();

        let chunks = Box::new(parser).finalize(ctx).unwrap();
        let chunk = chunks.first().expect("missing chunk");

        assert_eq!(chunk.num_rows(), 2);
        assert!(
            !chunk
                .timelines()
                .values()
                .any(|time_column| time_column.name() == "ros2_timestamp"),
            "an unrepresentable stamp must not leave a partial timeline"
        );
    }

    /// Checks that a corrupt message costs only its own row, not the rest of the channel.
    ///
    /// The corrupt messages here are truncated mid-row, so decoding fails only after the first
    /// field has already been written into the Arrow builders.
    #[test]
    fn decode_failure_drops_only_the_corrupt_rows() {
        let schema = MessageSchema::parse(
            "test/Message",
            r#"
int32 first
test/Inner inner

================================================================================
MSG: test/Inner
int32 second
float64[] values
"#,
        )
        .unwrap();
        let plan = Arc::new(MessageDecodePlan::from_schema(&schema).unwrap());
        let mut parser = Ros2ReflectionMessageParser::new(10, Arc::clone(&plan));
        let mut ctx = ParserContext::new(EntityPath::from("/test"), "/test", TimeType::TimestampNs);
        let channel = Arc::new(mcap::Channel {
            id: 1,
            topic: "/test".to_owned(),
            schema: None,
            message_encoding: "cdr".to_owned(),
            metadata: BTreeMap::new(),
        });
        let message = |parts: &[&[u8]]| mcap::Message {
            channel: Arc::clone(&channel),
            sequence: 0,
            log_time: 0,
            publish_time: 0,
            data: Cow::Owned(parts.concat()),
        };

        // Alternate valid and truncated messages, and only count a timepoint for the valid ones —
        // exactly what `McapChunkDecoder::decode_next` does.
        for i in 0..10_i32 {
            let result = if i % 2 == 0 {
                parser.append(
                    &mut ctx,
                    &message(&[
                        &[0x00, 0x01, 0x00, 0x00],
                        &i.to_le_bytes(),
                        &(i * 100).to_le_bytes(),
                        &1_u32.to_le_bytes(),
                        &0_u32.to_le_bytes(), // padding to the f64 alignment
                        &1.5_f64.to_le_bytes(),
                    ]),
                )
            } else {
                // Truncated right after `first`, so `inner` is only partially decoded.
                parser.append(
                    &mut ctx,
                    &message(&[&[0x00, 0x01, 0x00, 0x00][..], &i.to_le_bytes()]),
                )
            };

            assert_eq!(
                result.is_ok(),
                i % 2 == 0,
                "unexpected result for message {i}"
            );
            if result.is_ok() {
                ctx.add_timepoint(re_log_types::TimePoint::default().with(
                    re_log_types::Timeline::new("log_time", TimeType::TimestampNs),
                    i as i64,
                ));
            }
        }

        let chunks = Box::new(parser)
            .finalize(ctx)
            .expect("a corrupt message must not prevent finalization");
        let chunk = chunks.first().expect("missing chunk");

        // 10 messages, 5 of them corrupt, so 5 surviving rows.
        assert_eq!(chunk.num_rows(), 5);
        for time_column in chunk.timelines().values() {
            assert_eq!(time_column.num_rows(), 5, "timeline {}", time_column.name());
        }

        // The surviving rows are the valid ones, in order and with their values intact.
        let messages = chunk
            .components()
            .iter()
            .next()
            .expect("missing message column")
            .1;
        let messages = messages
            .list_array
            .values()
            .try_downcast_array_ref::<StructArray>()
            .expect("messages should be structs");
        let first = messages
            .column_by_name("first")
            .unwrap()
            .try_downcast_array_ref::<Int32Array>()
            .unwrap();
        assert_eq!(first.values(), &[0, 2, 4, 6, 8]);
    }

    /// Checks that a `header.stamp` reaches the finished chunk as a `ros2_timestamp` timeline,
    /// surviving the row filtering in `drop_rows`.
    #[test]
    fn header_stamp_becomes_the_ros2_timestamp_timeline() {
        let schema = MessageSchema::parse(
            "test/Message",
            r#"
std_msgs/Header header
int32 value

================================================================================
MSG: std_msgs/Header
builtin_interfaces/Time stamp
string frame_id

================================================================================
MSG: builtin_interfaces/Time
int32 sec
uint32 nanosec
"#,
        )
        .unwrap();
        let plan = Arc::new(MessageDecodePlan::from_schema(&schema).unwrap());
        let mut parser = Ros2ReflectionMessageParser::new(2, plan);
        let mut ctx = ParserContext::new(EntityPath::from("/test"), "/test", TimeType::TimestampNs);
        let channel = Arc::new(mcap::Channel {
            id: 1,
            topic: "/test".to_owned(),
            schema: None,
            message_encoding: "cdr".to_owned(),
            metadata: BTreeMap::new(),
        });

        let message = |sec: i32, nanosec: u32, value: i32| mcap::Message {
            channel: Arc::clone(&channel),
            sequence: 0,
            log_time: 0,
            publish_time: 0,
            data: Cow::Owned(
                [
                    &[0x00, 0x01, 0x00, 0x00][..], // CDR LE header
                    &sec.to_le_bytes(),
                    &nanosec.to_le_bytes(),
                    &0_u32.to_le_bytes(), // empty `frame_id`
                    &value.to_le_bytes(),
                ]
                .concat(),
            ),
        };

        parser
            .append(&mut ctx, &message(2, 500_000_000, 7))
            .unwrap();
        parser
            .append(&mut ctx, &message(3, 500_000_000, 8))
            .unwrap();

        let chunks = Box::new(parser).finalize(ctx).unwrap();
        let chunk = chunks.first().expect("missing chunk");

        let timeline = chunk
            .timelines()
            .values()
            .find(|time_column| time_column.name() == "ros2_timestamp")
            .expect("the header stamp should produce a `ros2_timestamp` timeline");

        assert_eq!(timeline.times_raw(), &[2_500_000_000, 3_500_000_000]);
        assert_eq!(timeline.num_rows(), chunk.num_rows());
    }
}
