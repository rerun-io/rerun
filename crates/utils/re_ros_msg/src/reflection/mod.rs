//! Decode CDR messages into Arrow arrays using a schema only known at runtime.
//!
//! ROS 2 message layouts are not known at compile time when reading a recording: the `.msg`
//! definition arrives alongside the data. [`MessageSchema`](crate::MessageSchema) parses that
//! text, [`MessageDecodePlan`] resolves it into a flat, immutable set of decoding instructions,
//! and [`CdrArrowDecoder`] walks the CDR stream straight into Arrow builders — with no
//! intermediate value tree.
//!
//! ```no_run
//! use re_ros_msg::{MessageSchema, reflection::{CdrArrowDecoder, MessageDecodePlan}};
//! # fn example(name: &str, definition: &str, messages: &[Vec<u8>]) -> anyhow::Result<()> {
//! let schema = MessageSchema::parse(name, definition)?;
//! let plan = std::sync::Arc::new(MessageDecodePlan::from_schema(&schema)?);
//!
//! let mut decoder = CdrArrowDecoder::new(plan, messages.len());
//! for message in messages {
//!     decoder.decode_message(message).ok();
//! }
//! let column = decoder.finish();
//! # Ok(())
//! # }
//! ```

mod cdr_to_arrow;
mod decode_plan;
mod timestamp;

pub use self::cdr_to_arrow::{CdrArrowDecoder, CdrDecodeError, ReflectionBuilderError};
pub use self::decode_plan::{FieldLayout, MessageDecodePlan, MessageLayout, ValueLayout};
pub use self::timestamp::TimestampError;
