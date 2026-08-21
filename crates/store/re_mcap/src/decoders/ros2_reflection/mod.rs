mod cdr_to_arrow;
mod decode_plan;
mod timestamp;

use std::collections::hash_map::Entry;
use std::sync::Arc;

use re_chunk::{Chunk, ChunkId};
use re_ros_msg::MessageSchema;
use re_ros_msg::message_spec::{BuiltInType, Type};
use re_sdk_types::reflection::ComponentDescriptorExt as _;
use re_sdk_types::{ArchetypeName, ComponentDescriptor};

use super::ros2::supports_ros2_cdr_channel;
use crate::parsers::{MessageParser, ParserContext};
use crate::{DecoderIdentifier, Error, MessageDecoder};

use self::cdr_to_arrow::{CdrArrowDecoder, CdrDecodeError};
use self::decode_plan::MessageDecodePlan;
use self::timestamp::add_ros2_timestamps;

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

    #[error("Failed to downcast builder to expected type: {0}")]
    Downcast(&'static str),
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
        add_ros2_timestamps(&mut ctx, decoder.plan().timestamp_location(), &messages);

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

#[cfg(test)]
mod tests {
    use std::assert_matches;
    use std::{borrow::Cow, collections::BTreeMap};

    use arrow::array::{
        Array as _, Float32Array, Float64Array, Int32Array, ListArray, StructArray, UInt16Array,
    };
    use re_chunk::EntityPath;
    use re_log_types::TimeType;
    use re_ros_msg::MessageSchema;

    use super::timestamp::{TimestampLocation, timestamp_columns, timestamp_nanos};
    use super::*;

    /// Decodes one CDR message and returns its decoded struct row.
    fn decode_one(plan: &Arc<MessageDecodePlan>, data: &[u8]) -> StructArray {
        let mut decoder = CdrArrowDecoder::new(Arc::clone(plan), 1);
        decoder.decode_message(data).expect("failed to decode");
        decoder
            .finish()
            .values()
            .as_any()
            .downcast_ref::<StructArray>()
            .expect("messages should be structs")
            .clone()
    }

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

    #[test]
    fn decodes_directly_into_arrow() {
        let schema = MessageSchema::parse(
            "test/Outer",
            r#"
builtin_interfaces/Time stamp
int32 number
float32[] values
test/Inner inner

================================================================================
MSG: builtin_interfaces/Time
int32 sec
uint32 nanosec

================================================================================
MSG: test/Inner
uint16 value
"#,
        )
        .unwrap();
        let plan = Arc::new(MessageDecodePlan::from_schema(&schema).unwrap());

        let array = decode_one(
            &plan,
            &[
                0x00, 0x01, 0x00, 0x00, // CDR LE header
                2, 0, 0, 0, // stamp.sec
                0x00, 0x65, 0xCD, 0x1D, // stamp.nanosec
                7, 0, 0, 0, // number
                2, 0, 0, 0, // values length
                0, 0, 0xC0, 0x3F, // values[0] = 1.5
                0, 0, 0x20, 0x40, // values[1] = 2.5
                9, 0, // inner.value
            ],
        );
        let (secs, nanosecs) = timestamp_columns(plan.timestamp_location(), &array).unwrap();
        let decoded_timestamp = timestamp_nanos(secs.value(0), nanosecs.value(0));
        let number = array
            .column_by_name("number")
            .unwrap()
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();
        let values = array
            .column_by_name("values")
            .unwrap()
            .as_any()
            .downcast_ref::<ListArray>()
            .unwrap();
        let values = values.value(0);
        let values = values.as_any().downcast_ref::<Float32Array>().unwrap();
        let inner = array
            .column_by_name("inner")
            .unwrap()
            .as_any()
            .downcast_ref::<StructArray>()
            .unwrap();
        let inner_value = inner
            .column_by_name("value")
            .unwrap()
            .as_any()
            .downcast_ref::<UInt16Array>()
            .unwrap();

        assert_eq!(decoded_timestamp, Some(2_500_000_000));
        assert_eq!(number.value(0), 7);
        assert_eq!(values.values(), &[1.5, 2.5]);
        assert_eq!(inner_value.value(0), 9);
    }

    /// Checks that arrays of nested messages decode into a list of structs, and that fixed-size
    /// arrays (e.g. `float64[9]` covariances) decode without a length prefix — CDR omits it,
    /// because the length is already known from the schema.
    #[test]
    fn decodes_covariance_block_and_array_of_messages() {
        let schema = MessageSchema::parse(
            "test/Message",
            r#"
geometry_msgs/Point[] points
float64[9] orientation_covariance

================================================================================
MSG: geometry_msgs/Point
float64 x
float64 y
float64 z
"#,
        )
        .unwrap();
        let plan = Arc::new(MessageDecodePlan::from_schema(&schema).unwrap());

        let mut data = vec![0x00, 0x01, 0x00, 0x00]; // CDR LE header
        data.extend_from_slice(&2_u32.to_le_bytes()); // `points` length
        data.extend_from_slice(&[0, 0, 0, 0]); // padding to the f64 alignment
        for value in 1..=6 {
            data.extend_from_slice(&f64::from(value).to_le_bytes()); // points[0..2].x/y/z
        }
        for value in 1..=9 {
            data.extend_from_slice(&f64::from(value).to_le_bytes()); // covariance, unprefixed
        }

        let array = decode_one(&plan, &data);

        let points = array
            .column_by_name("points")
            .unwrap()
            .as_any()
            .downcast_ref::<ListArray>()
            .unwrap()
            .value(0);
        let points = points.as_any().downcast_ref::<StructArray>().unwrap();
        let axis = |name: &str| {
            points
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<Float64Array>()
                .unwrap()
                .values()
                .to_vec()
        };

        assert_eq!(axis("x"), [1.0, 4.0]);
        assert_eq!(axis("y"), [2.0, 5.0]);
        assert_eq!(axis("z"), [3.0, 6.0]);

        let covariance = array
            .column_by_name("orientation_covariance")
            .unwrap()
            .as_any()
            .downcast_ref::<ListArray>()
            .unwrap()
            .value(0);
        let covariance = covariance.as_any().downcast_ref::<Float64Array>().unwrap();

        assert_eq!(
            covariance.values(),
            &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]
        );
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

    /// Checks that a decode failing on the very first field still leaves every builder at a row
    /// boundary, so the row can be cancelled and the next message decoded.
    #[test]
    fn decode_failure_on_the_first_field_is_recoverable() {
        let schema = MessageSchema::parse("test/Message", "int32 first\nint32 second\n").unwrap();
        let plan = Arc::new(MessageDecodePlan::from_schema(&schema).unwrap());
        let mut decoder = CdrArrowDecoder::new(Arc::clone(&plan), 2);

        // A bare encapsulation header: the body is empty, so `first` cannot be read.
        assert!(decoder.decode_message(&[0x00, 0x01, 0x00, 0x00]).is_err());

        decoder
            .decode_message(
                &[
                    &[0x00, 0x01, 0x00, 0x00][..],
                    &7_i32.to_le_bytes(),
                    &8_i32.to_le_bytes(),
                ]
                .concat(),
            )
            .expect("decoding must continue after a cancelled row");

        let messages = decoder.finish();
        let array = messages
            .values()
            .as_any()
            .downcast_ref::<StructArray>()
            .unwrap();
        let first = array
            .column_by_name("first")
            .unwrap()
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();

        assert_eq!(messages.len(), 1);
        assert_eq!(first.values(), &[7]);
    }

    /// Checks that a decode failing part-way through an array of messages is recoverable.
    ///
    /// Cancelling that row closes the half-written element with a null, which lands in the list's
    /// values array before `finish` drops the row. `ListArray` rejects such an element under a
    /// non-nullable item field, so this is what keeps arrays of messages nullable.
    #[test]
    fn decode_failure_inside_an_array_of_messages_is_recoverable() {
        let schema = MessageSchema::parse(
            "test/Message",
            r#"
test/Inner[] items

================================================================================
MSG: test/Inner
int32 a
int32 b
"#,
        )
        .unwrap();
        let plan = Arc::new(MessageDecodePlan::from_schema(&schema).unwrap());
        let mut decoder = CdrArrowDecoder::new(Arc::clone(&plan), 2);

        // Two elements promised, but the second one is truncated after `a`, leaving the element
        // struct mid-row.
        assert!(
            decoder
                .decode_message(
                    &[
                        &[0x00, 0x01, 0x00, 0x00][..],
                        &2_u32.to_le_bytes(),
                        &1_i32.to_le_bytes(),
                        &2_i32.to_le_bytes(),
                        &3_i32.to_le_bytes(),
                    ]
                    .concat(),
                )
                .is_err()
        );

        decoder
            .decode_message(
                &[
                    &[0x00, 0x01, 0x00, 0x00][..],
                    &1_u32.to_le_bytes(),
                    &7_i32.to_le_bytes(),
                    &8_i32.to_le_bytes(),
                ]
                .concat(),
            )
            .expect("decoding must continue after a cancelled row");

        // Panics if the null element ended up under a non-nullable list item field.
        let messages = decoder.finish();
        assert_eq!(messages.len(), 1);

        let items = messages
            .values()
            .as_any()
            .downcast_ref::<StructArray>()
            .unwrap()
            .column_by_name("items")
            .unwrap()
            .as_any()
            .downcast_ref::<arrow::array::ListArray>()
            .unwrap()
            .clone();
        assert_eq!(items.len(), 1);

        let inner = items
            .values()
            .as_any()
            .downcast_ref::<StructArray>()
            .unwrap();
        assert_eq!(inner.null_count(), 0, "the cancelled element must be gone");
        for (name, expected) in [("a", 7_i32), ("b", 8)] {
            let column = inner
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<Int32Array>()
                .unwrap()
                .clone();
            assert_eq!(column.values(), &[expected], "unexpected values for {name}");
        }
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
            .as_any()
            .downcast_ref::<StructArray>()
            .expect("messages should be structs");
        let first = messages
            .column_by_name("first")
            .unwrap()
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();
        assert_eq!(first.values(), &[0, 2, 4, 6, 8]);
    }

    /// Checks that an empty sequence writes only its length, leaving the alignment of the next
    /// field untouched — CDR aligns a payload, and an empty sequence has none.
    #[test]
    fn empty_numeric_sequence_does_not_align_the_next_field() {
        let schema =
            MessageSchema::parse("test/Message", "float64[] empty\nfloat64[] values\n").unwrap();
        let plan = Arc::new(MessageDecodePlan::from_schema(&schema).unwrap());

        let array = decode_one(
            &plan,
            &[
                0x00, 0x01, 0x00, 0x00, // CDR LE header
                0, 0, 0, 0, // empty length
                1, 0, 0, 0, // values length
                0, 0, 0, 0, 0, 0, 0xF8, 0x3F, // values[0] = 1.5
            ],
        );
        let values = array
            .column_by_name("values")
            .unwrap()
            .as_any()
            .downcast_ref::<ListArray>()
            .unwrap()
            .value(0);
        let values = values.as_any().downcast_ref::<Float64Array>().unwrap();
        assert_eq!(values.values(), &[1.5]);
    }

    #[test]
    fn header_stamp_has_exclusive_precedence() {
        let schema = MessageSchema::parse(
            "test/Message",
            r#"
test/Header header
builtin_interfaces/Time stamp

================================================================================
MSG: test/Header
builtin_interfaces/Time stamp

================================================================================
MSG: builtin_interfaces/Time
int32 sec
uint32 nanosec
"#,
        )
        .unwrap();
        let plan = Arc::new(MessageDecodePlan::from_schema(&schema).unwrap());

        let array = decode_one(
            &plan,
            &[
                0x00, 0x01, 0x00, 0x00, // CDR LE header
                2, 0, 0, 0, // header.stamp.sec
                0x00, 0x65, 0xCD, 0x1D, // header.stamp.nanosec
                9, 0, 0, 0, // top-level stamp.sec
                0, 0, 0, 0, // top-level stamp.nanosec
            ],
        );
        let (secs, nanosecs) = timestamp_columns(plan.timestamp_location(), &array).unwrap();
        let decoded_timestamp = timestamp_nanos(secs.value(0), nanosecs.value(0));

        assert_eq!(decoded_timestamp, Some(2_500_000_000));
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

    #[test]
    fn top_level_stamp_is_ignored_when_header_has_no_stamp() {
        let schema = MessageSchema::parse(
            "test/Message",
            r#"
test/Header header
builtin_interfaces/Time stamp

================================================================================
MSG: test/Header
int32 value

================================================================================
MSG: builtin_interfaces/Time
int32 sec
uint32 nanosec
"#,
        )
        .unwrap();
        let plan = MessageDecodePlan::from_schema(&schema).unwrap();
        assert_matches!(plan.timestamp_location(), TimestampLocation::None);
    }
}
